//! ArtifactStore — SQLite metadata + filesystem content storage.

use std::path::PathBuf;

use deadpool_sqlite::{Config, Pool, Runtime};
use rusqlite::params;

use crate::error::{ArtifactError, Result};
use crate::types::{Artifact, ArtifactId, ArtifactMeta};

/// Storage backend for artifacts.
///
/// `db_path` holds SQLite metadata. Text content is also written to
/// `artifacts_dir/{id}.{ext}` so it can be served as static files.
///
/// All database access uses `deadpool_sqlite`'s `interact()` which dispatches
/// to a dedicated blocking thread, keeping every async caller unblocked.
#[derive(Clone)]
pub struct ArtifactStore {
    pool: Pool,
    pub artifacts_dir: PathBuf,
}

impl std::fmt::Debug for ArtifactStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArtifactStore")
            .field("artifacts_dir", &self.artifacts_dir)
            .finish_non_exhaustive()
    }
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

        let cfg = Config::new(db_path.to_string_lossy().to_string());
        let pool = cfg
            .create_pool(Runtime::Tokio1)
            .map_err(|e| ArtifactError::Internal(e.to_string()))?;

        let conn = pool
            .get()
            .await
            .map_err(|e| ArtifactError::Internal(e.to_string()))?;
        conn.interact(|c| c.execute_batch(SCHEMA))
            .await
            .map_err(|e| ArtifactError::Internal(e.to_string()))?
            .map_err(ArtifactError::Db)?;

        Ok(Self { pool, artifacts_dir })
    }

    /// Persist a new artifact and return its generated ID.
    pub async fn create(&self, chat_id: &str, artifact: Artifact) -> Result<ArtifactId> {
        let id = ArtifactId::new();
        let now = chrono::Utc::now().timestamp();
        let extension = artifact.extension().to_string();

        // Write text content to filesystem if applicable.
        if let Some(text) = artifact.content_str() {
            let file_path = self
                .artifacts_dir
                .join(format!("{}.{}", id.0, extension));
            tokio::fs::write(&file_path, text).await?;
        }

        let artifact_json = serde_json::to_string(&artifact).map_err(ArtifactError::Json)?;
        let artifact_type = artifact.type_name().to_string();

        let id_str = id.0.clone();
        let chat_id = chat_id.to_string();

        let conn = self
            .pool
            .get()
            .await
            .map_err(|e| ArtifactError::Internal(e.to_string()))?;
        conn.interact(move |c| {
            c.execute(
                "INSERT INTO artifacts \
                 (id, chat_id, artifact_type, artifact_json, extension, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![id_str, chat_id, artifact_type, artifact_json, extension, now, now],
            )
        })
        .await
        .map_err(|e| ArtifactError::Internal(e.to_string()))?
        .map_err(ArtifactError::Db)?;

        Ok(id)
    }

    /// Retrieve a full artifact by ID.
    pub async fn get(&self, id: &ArtifactId) -> Result<Option<Artifact>> {
        let id_str = id.0.clone();

        let conn = self
            .pool
            .get()
            .await
            .map_err(|e| ArtifactError::Internal(e.to_string()))?;
        let maybe_json: Option<String> = conn
            .interact(move |c| {
                let mut stmt = c.prepare("SELECT artifact_json FROM artifacts WHERE id = ?1")?;
                let mut rows = stmt.query(params![id_str])?;
                if let Some(row) = rows.next()? {
                    Ok(Some(row.get::<_, String>(0)?))
                } else {
                    Ok(None)
                }
            })
            .await
            .map_err(|e| ArtifactError::Internal(e.to_string()))?
            .map_err(ArtifactError::Db)?;

        maybe_json
            .map(|json| serde_json::from_str(&json).map_err(ArtifactError::Json))
            .transpose()
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

        let artifact_json = serde_json::to_string(&artifact).map_err(ArtifactError::Json)?;
        let artifact_type = artifact.type_name().to_string();
        let extension = artifact.extension().to_string();
        let id_str = id.0.clone();
        let id_for_err = id.0.clone();

        let conn = self
            .pool
            .get()
            .await
            .map_err(|e| ArtifactError::Internal(e.to_string()))?;
        let rows_affected = conn
            .interact(move |c| {
                c.execute(
                    "UPDATE artifacts \
                     SET artifact_type = ?1, artifact_json = ?2, extension = ?3, updated_at = ?4 \
                     WHERE id = ?5",
                    params![artifact_type, artifact_json, extension, now, id_str],
                )
            })
            .await
            .map_err(|e| ArtifactError::Internal(e.to_string()))?
            .map_err(ArtifactError::Db)?;

        if rows_affected == 0 {
            Err(ArtifactError::NotFound(id_for_err))
        } else {
            Ok(())
        }
    }

    /// List lightweight metadata for all artifacts belonging to a chat.
    pub async fn list_for_chat(&self, chat_id: &str) -> Result<Vec<ArtifactMeta>> {
        let chat_id = chat_id.to_string();

        let conn = self
            .pool
            .get()
            .await
            .map_err(|e| ArtifactError::Internal(e.to_string()))?;
        conn.interact(move |c| {
            let mut stmt = c.prepare(
                "SELECT id, chat_id, artifact_type, created_at, updated_at
                 FROM artifacts WHERE chat_id = ?1
                 ORDER BY created_at ASC",
            )?;
            stmt.query_map(params![chat_id], |row| {
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
            .collect::<std::result::Result<Vec<ArtifactMeta>, _>>()
        })
        .await
        .map_err(|e| ArtifactError::Internal(e.to_string()))?
        .map_err(ArtifactError::Db)
    }

    /// Delete an artifact. Returns `true` if the row existed, `false` otherwise.
    ///
    /// Looks up the stored extension from the DB before deleting, so no
    /// hardcoded extension list is needed.
    pub async fn delete(&self, id: &ArtifactId) -> Result<bool> {
        let id_str = id.0.clone();

        let conn = self
            .pool
            .get()
            .await
            .map_err(|e| ArtifactError::Internal(e.to_string()))?;
        let (extension, rows_deleted): (Option<String>, usize) = conn
            .interact(move |c| {
                let ext: Option<String> = c
                    .query_row(
                        "SELECT extension FROM artifacts WHERE id = ?1",
                        params![id_str.clone()],
                        |row| row.get(0),
                    )
                    .ok();
                let rows = c.execute("DELETE FROM artifacts WHERE id = ?1", params![id_str])?;
                Ok::<_, rusqlite::Error>((ext, rows))
            })
            .await
            .map_err(|e| ArtifactError::Internal(e.to_string()))?
            .map_err(ArtifactError::Db)?;

        // Best-effort filesystem cleanup using the stored extension.
        if let Some(ext) = extension {
            let path = self.artifacts_dir.join(format!("{}.{}", id.0, ext));
            let _ = tokio::fs::remove_file(&path).await;
        }

        Ok(rows_deleted > 0)
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
    extension     TEXT NOT NULL DEFAULT '',
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_artifacts_chat_id ON artifacts (chat_id);
";
