//! ArtifactStore — SQLite metadata + filesystem content storage.

use std::path::PathBuf;

use rusqlite::params;

use crate::error::{ArtifactError, Result};
use crate::types::{Artifact, ArtifactId, ArtifactMeta};

/// Storage backend for artifacts.
///
/// `db_path` holds SQLite metadata. Text content is also written to
/// `artifacts_dir/{id}.{ext}` so it can be served as static files.
///
/// All database access uses `tokio::task::spawn_blocking` so the store is
/// safe to use from async handlers even though `rusqlite::Connection` is
/// `!Send`.
#[derive(Debug, Clone)]
pub struct ArtifactStore {
    pub db_path: PathBuf,
    pub artifacts_dir: PathBuf,
}

impl ArtifactStore {
    /// Create an `ArtifactStore` rooted at `~/.local/share/hyprcollab/`.
    pub async fn new_default() -> Result<Self> {
        let base = default_base_dir();
        Self::new(base.join("artifacts.db"), base.join("artifacts")).await
    }

    /// Create an `ArtifactStore` with explicit paths (useful for tests).
    pub async fn new(db_path: PathBuf, artifacts_dir: PathBuf) -> Result<Self> {
        tokio::fs::create_dir_all(&artifacts_dir).await?;

        // Initialise the schema on a blocking thread.
        let db_path_clone = db_path.clone();
        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&db_path_clone)?;
            conn.execute_batch(SCHEMA)?;
            Ok::<_, ArtifactError>(())
        })
        .await
        .map_err(|e| ArtifactError::Internal(e.to_string()))??;

        Ok(Self { db_path, artifacts_dir })
    }

    /// Persist a new artifact and return its generated ID.
    pub async fn create(&self, chat_id: &str, artifact: Artifact) -> Result<ArtifactId> {
        let id = ArtifactId::new();
        let now = chrono::Utc::now().timestamp();

        // Write text content to filesystem if applicable.
        if let Some(text) = artifact.content_str() {
            let file_path = self
                .artifacts_dir
                .join(format!("{}.{}", id.0, artifact.extension()));
            tokio::fs::write(&file_path, text).await?;
        }

        let artifact_json = serde_json::to_string(&artifact)
            .map_err(ArtifactError::Json)?;
        let artifact_type = artifact.type_name().to_string();

        let db_path = self.db_path.clone();
        let id_str = id.0.clone();
        let chat_id = chat_id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&db_path)?;
            conn.execute(
                "INSERT INTO artifacts (id, chat_id, artifact_type, artifact_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id_str, chat_id, artifact_type, artifact_json, now, now],
            )?;
            Ok::<_, ArtifactError>(())
        })
        .await
        .map_err(|e| ArtifactError::Internal(e.to_string()))??;

        Ok(id)
    }

    /// Retrieve a full artifact by ID.
    pub async fn get(&self, id: &ArtifactId) -> Result<Option<Artifact>> {
        let db_path = self.db_path.clone();
        let id_str = id.0.clone();

        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&db_path)?;
            let mut stmt = conn.prepare(
                "SELECT artifact_json FROM artifacts WHERE id = ?1",
            )?;
            let mut rows = stmt.query(params![id_str])?;
            if let Some(row) = rows.next()? {
                let json: String = row.get(0)?;
                let artifact = serde_json::from_str(&json)
                    .map_err(ArtifactError::Json)?;
                Ok(Some(artifact))
            } else {
                Ok(None)
            }
        })
        .await
        .map_err(|e| ArtifactError::Internal(e.to_string()))?
    }

    /// Replace the artifact content for an existing ID.
    pub async fn update(&self, id: &ArtifactId, artifact: Artifact) -> Result<()> {
        let now = chrono::Utc::now().timestamp();

        // Overwrite filesystem content if applicable.
        if let Some(text) = artifact.content_str() {
            let file_path = self
                .artifacts_dir
                .join(format!("{}.{}", id.0, artifact.extension()));
            tokio::fs::write(&file_path, text).await?;
        }

        let artifact_json = serde_json::to_string(&artifact)
            .map_err(ArtifactError::Json)?;
        let artifact_type = artifact.type_name().to_string();

        let db_path = self.db_path.clone();
        let id_str = id.0.clone();

        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&db_path)?;
            let rows_affected = conn.execute(
                "UPDATE artifacts SET artifact_type = ?1, artifact_json = ?2, updated_at = ?3
                 WHERE id = ?4",
                params![artifact_type, artifact_json, now, id_str],
            )?;
            if rows_affected == 0 {
                Err(ArtifactError::NotFound(id_str))
            } else {
                Ok(())
            }
        })
        .await
        .map_err(|e| ArtifactError::Internal(e.to_string()))?
    }

    /// List lightweight metadata for all artifacts belonging to a chat.
    pub async fn list_for_chat(&self, chat_id: &str) -> Result<Vec<ArtifactMeta>> {
        let db_path = self.db_path.clone();
        let chat_id = chat_id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&db_path)?;
            let mut stmt = conn.prepare(
                "SELECT id, chat_id, artifact_type, created_at, updated_at
                 FROM artifacts WHERE chat_id = ?1
                 ORDER BY created_at ASC",
            )?;
            let metas: std::result::Result<Vec<ArtifactMeta>, _> = stmt
                .query_map(params![chat_id], |row| {
                    let created_ts: i64 = row.get(3)?;
                    let updated_ts: i64 = row.get(4)?;
                    Ok(ArtifactMeta {
                        id: ArtifactId(row.get(0)?),
                        chat_id: row.get(1)?,
                        artifact_type: row.get(2)?,
                        created_at: chrono::DateTime::from_timestamp(created_ts, 0)
                            .unwrap_or_default(),
                        updated_at: chrono::DateTime::from_timestamp(updated_ts, 0)
                            .unwrap_or_default(),
                    })
                })?
                .collect();
            Ok(metas?)
        })
        .await
        .map_err(|e| ArtifactError::Internal(e.to_string()))?
    }

    /// Delete an artifact. Returns `true` if the row existed, `false` otherwise.
    pub async fn delete(&self, id: &ArtifactId) -> Result<bool> {
        // Best-effort filesystem cleanup — ignore errors if the file is missing.
        for ext in &[
            "rs", "py", "js", "ts", "md", "html", "svg", "mmd", "jsx", "tex", "txt", "json",
        ] {
            let path = self.artifacts_dir.join(format!("{}.{}", id.0, ext));
            let _ = tokio::fs::remove_file(&path).await;
        }

        let db_path = self.db_path.clone();
        let id_str = id.0.clone();

        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&db_path)?;
            let rows = conn.execute(
                "DELETE FROM artifacts WHERE id = ?1",
                params![id_str],
            )?;
            Ok::<bool, ArtifactError>(rows > 0)
        })
        .await
        .map_err(|e| ArtifactError::Internal(e.to_string()))?
    }
}

fn default_base_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".local/share/hyprcollab")
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS artifacts (
    id            TEXT PRIMARY KEY,
    chat_id       TEXT NOT NULL,
    artifact_type TEXT NOT NULL,
    artifact_json TEXT NOT NULL,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_artifacts_chat_id ON artifacts (chat_id);
";
