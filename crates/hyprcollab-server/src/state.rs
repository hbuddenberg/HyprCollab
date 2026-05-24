use std::sync::{Arc, Mutex};

/// Shared state for the HyprCollab server.
#[derive(Clone, Debug)]
pub struct AppState {
    /// Crate/Server version.
    pub version: String,
    /// Thread-safe tracking of tool approvals for demonstration and testing purposes.
    pub approvals: Arc<Mutex<Vec<String>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            approvals: Arc::new(Mutex::new(Vec::new())),
        }
    }
}
