use serde::{Deserialize, Serialize};

// ── IPC Protocol: JSON lines over Unix socket ──────────────────────────
//
// Client → Daemon:  Request (JSON + \n)
// Daemon → Client:  Response (JSON + \n)

/// Request from GUI to daemon
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Request {
    /// Get full state snapshot
    GetState,

    /// Send a user message in active chat
    SendMessage { content: String },

    /// Select a chat by folder+chat index
    SelectChat { folder: usize, chat: usize },

    /// Create a new chat in active folder
    NewChat { title: String },

    /// Create a new folder
    NewFolder { name: String },

    /// Toggle sidebar visibility
    ToggleSidebar,

    /// Collapse/expand sidebar
    SetSidebar { visible: bool },

    /// Change active agent
    SetAgent { name: String },

    /// Change active model
    SetModel { name: String },

    /// Set system prompt for the active chat
    SetSystemPrompt { content: String },

    /// Ping (health check)
    Ping,
}

/// Response from daemon to GUI
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Response {
    /// Full state snapshot
    State { data: StateSnapshot },

    /// Simple acknowledgement
    Ack { msg: String },

    /// Error response
    Error { msg: String },

    /// Pong
    Pong,

    /// Streaming token chunk (partial LLM response — sent repeatedly before final State)
    Token { text: String },
}

/// Serializable state snapshot sent to GUI
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StateSnapshot {
    pub folders: Vec<FolderInfo>,
    pub active_folder: Option<usize>,
    pub active_chat: Option<usize>,
    pub agents: Vec<AgentInfo>,
    pub active_agent: String,
    pub models: Vec<ModelInfo>,
    pub active_model: String,
    pub sidebar_visible: bool,
    pub active_messages: Vec<MessageInfo>,
    pub active_chat_title: String,
    pub active_chat_tokens: u64,
    pub active_git_branch: Option<String>,
    pub working: bool,
    pub system_prompt: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatInfo {
    pub title: String,
    pub last_active: i64, // unix timestamp
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FolderInfo {
    pub name: String,
    pub icon: String,
    pub chats: Vec<ChatInfo>,
    pub git_branch: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentInfo {
    pub name: String,
    pub available: bool,
    pub icon: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub provider: String,
    pub available: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessageInfo {
    pub role: String,
    pub content: String,
    pub timestamp: i64,
    pub tokens: u32,
}

/// Encode a response as JSON line
pub fn encode<T: Serialize>(msg: &T) -> String {
    serde_json::to_string(msg).unwrap_or_default() + "\n"
}

/// Decode a request from JSON line
pub fn decode_request(line: &str) -> Option<Request> {
    serde_json::from_str(line.trim()).ok()
}

/// Decode a response from JSON line
pub fn decode_response(line: &str) -> Option<Response> {
    serde_json::from_str(line.trim()).ok()
}
