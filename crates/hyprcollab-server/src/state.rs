use std::sync::Arc;
use tokio::sync::Mutex;

use hyprcollab_artifacts::ArtifactStore;

/// Shared state for the HyprCollab server.
#[derive(Clone, Debug)]
pub struct AppState {
    /// Crate/server version string.
    pub version: String,
    /// Pending tool-approval records (async-safe mutex).
    pub approvals: Arc<Mutex<Vec<String>>>,
    /// Artifact storage backend.
    pub artifacts: ArtifactStore,
}

impl AppState {
    pub fn new(artifacts: ArtifactStore) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            approvals: Arc::new(Mutex::new(Vec::new())),
            artifacts,
        }
    }
}
