//! Error types for hyprcollab-artifacts.

#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Json(#[source] serde_json::Error),

    #[error("Task join error: {0}")]
    Internal(String),

    #[error("Artifact not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, ArtifactError>;
