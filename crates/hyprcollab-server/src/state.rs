use std::sync::Arc;
use tokio::sync::Mutex;

use hyprcollab_artifacts::ArtifactStore;
use hyprcollab_browser::BrowserEngine;
use hyprcollab_image::ImageRouter;
use hyprcollab_memory::MemoryStore;
use hyprcollab_rag::RagPipeline;
use hyprcollab_router::LlmRouter;
use hyprcollab_themes::ThemeEngine;

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
    /// Optional RAG pipeline (None when not configured).
    pub rag: Option<Arc<RagPipeline>>,
    /// Optional browser engine (None when not configured).
    pub browser: Option<Arc<BrowserEngine>>,
    /// Optional image generation router (None when not configured).
    pub image_router: Option<Arc<ImageRouter>>,
    /// Theme engine (built-in + custom themes).
    pub themes: Arc<ThemeEngine>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState").field("version", &self.version).finish_non_exhaustive()
    }
}

impl AppState {
    /// Convenience constructor for tests: empty router + in-memory SQLite, no optional services.
    pub fn new(artifacts: ArtifactStore) -> Self {
        Self::new_full(
            artifacts,
            LlmRouter::new(),
            MemoryStore::open_in_memory().expect("in-memory SQLite"),
            None,
        )
    }

    /// Full constructor used by the CLI binary.
    pub fn new_full(
        artifacts: ArtifactStore,
        router: LlmRouter,
        memory: MemoryStore,
        rag: Option<RagPipeline>,
    ) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            approvals: Arc::new(Mutex::new(Vec::new())),
            artifacts,
            router: Arc::new(router),
            memory: Arc::new(memory),
            rag: rag.map(Arc::new),
            browser: None,
            image_router: None,
            themes: Arc::new(ThemeEngine::new()),
        }
    }

    /// Attach a browser engine to the state.
    pub fn with_browser(mut self, engine: BrowserEngine) -> Self {
        self.browser = Some(Arc::new(engine));
        self
    }

    /// Attach an image router to the state.
    pub fn with_image_router(mut self, router: ImageRouter) -> Self {
        self.image_router = Some(Arc::new(router));
        self
    }
}
