use std::sync::Arc;
use tokio::sync::Mutex;

use hyprcollab_artifacts::ArtifactStore;
use hyprcollab_memory::MemoryStore;
use hyprcollab_router::LlmRouter;

/// Shared state for the HyprCollab server.
#[derive(Clone)]
pub struct AppState {
    /// Crate/server version string.
    pub version: String,
    /// Pending tool-approval records (async-safe mutex).
    pub approvals: Arc<Mutex<Vec<String>>>,
    /// Artifact storage backend.
    pub artifacts: ArtifactStore,
    /// Multi-provider LLM router.
    pub router: Arc<LlmRouter>,
    /// SQLite-backed conversation and message store.
    pub memory: Arc<MemoryStore>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState").field("version", &self.version).finish_non_exhaustive()
    }
}

impl AppState {
    /// Convenience constructor for tests: empty router + in-memory SQLite.
    pub fn new(artifacts: ArtifactStore) -> Self {
        Self::new_full(
            artifacts,
            LlmRouter::new(),
            MemoryStore::open_in_memory().expect("in-memory SQLite"),
        )
    }

    /// Full constructor used by the CLI binary.
    pub fn new_full(artifacts: ArtifactStore, router: LlmRouter, memory: MemoryStore) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            approvals: Arc::new(Mutex::new(Vec::new())),
            artifacts,
            router: Arc::new(router),
            memory: Arc::new(memory),
        }
    }
}
