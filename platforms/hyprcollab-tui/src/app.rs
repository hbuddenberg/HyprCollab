use crate::artifact_pane::ArtifactPane;
use crate::chat_pane::ChatPane;
use crate::input::{AppMode, FocusedPane, InputHandler};

/// Top-level TUI application state.
pub struct App {
    session_id: String,
    pub mode: AppMode,
    pub pane: FocusedPane,
    pub input: InputHandler,
    pub chat: ChatPane,
    pub artifacts: ArtifactPane,
}

impl App {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            mode: AppMode::Normal,
            pane: FocusedPane::Chat,
            input: InputHandler::new(),
            chat: ChatPane::new(),
            artifacts: ArtifactPane::new(),
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn mode(&self) -> AppMode {
        self.mode
    }
}
