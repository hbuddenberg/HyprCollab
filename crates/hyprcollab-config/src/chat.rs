use serde::{Deserialize, Serialize};

use hyprcollab_core::ApprovalMode;

/// Per-chat runtime overrides. These are ephemeral and not persisted to disk
/// (they are supplied by the UI or API at chat-creation time).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChatConfig {
    /// Override the system prompt for this chat.
    #[serde(default)]
    pub system_prompt: Option<String>,

    /// Override the model for this chat.
    #[serde(default)]
    pub model: Option<String>,

    /// Override the approval mode for this chat.
    #[serde(default)]
    pub approval_mode: Option<ApprovalMode>,

    /// Override max turns for this chat.
    #[serde(default)]
    pub max_turns: Option<u32>,

    /// Override RAG enabled flag.
    #[serde(default)]
    pub rag_enabled: Option<bool>,

    /// Additional tools to enable for this chat (appended to resolved list).
    #[serde(default)]
    pub tools: Vec<String>,

    /// Additional MCP servers to connect for this chat (appended to resolved list).
    #[serde(default)]
    pub mcp_servers: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_config_default_is_empty() {
        let config = ChatConfig::default();
        assert!(config.system_prompt.is_none());
        assert!(config.model.is_none());
        assert!(config.approval_mode.is_none());
        assert!(config.max_turns.is_none());
        assert!(config.rag_enabled.is_none());
        assert!(config.tools.is_empty());
        assert!(config.mcp_servers.is_empty());
    }
}
