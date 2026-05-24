use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use hyprcollab_core::ApprovalMode;

use crate::{AgentConfig, ChatConfig, FolderConfig, GlobalConfig};

/// The fully resolved (flattened) configuration after merging all four layers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedConfig {
    // ── General ──────────────────────────────────────────────────────
    pub app_name: String,
    pub version: String,
    pub language: String,
    pub theme: String,
    pub data_dir: String,

    // ── Model ────────────────────────────────────────────────────────
    pub default_model: String,

    // ── Providers ────────────────────────────────────────────────────
    pub providers: HashMap<String, crate::global::ProviderConfig>,

    // ── Approval ─────────────────────────────────────────────────────
    pub approval_mode: ApprovalMode,

    // ── UI ───────────────────────────────────────────────────────────
    pub sidebar_width: u32,
    pub show_status_bar: bool,
    pub font_size: u32,

    // ── RAG ──────────────────────────────────────────────────────────
    pub rag_enabled: bool,
    pub rag_paths: Vec<String>,
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub top_k: usize,

    // ── Agent ────────────────────────────────────────────────────────
    pub system_prompt: String,
    pub tools: Vec<String>,
    pub mcp_servers: Vec<String>,
    pub max_turns: Option<u32>,

    // ── Project ──────────────────────────────────────────────────────
    pub project_name: Option<String>,
}

/// Builder that merges the four config layers (global → folder → agent → chat).
#[derive(Debug, Clone)]
pub struct ConfigResolver {
    global: GlobalConfig,
    folder: Option<FolderConfig>,
    agent: Option<AgentConfig>,
    chat: Option<ChatConfig>,
}

impl ConfigResolver {
    /// Create a new resolver starting from the given global config.
    pub fn new(global: GlobalConfig) -> Self {
        Self {
            global,
            folder: None,
            agent: None,
            chat: None,
        }
    }

    /// Create a resolver with the default global config.
    pub fn with_defaults() -> Self {
        Self::new(GlobalConfig::default())
    }

    /// Load the global config and return a new resolver.
    pub fn load() -> Result<Self> {
        let global = GlobalConfig::load()?;
        Ok(Self::new(global))
    }

    /// Apply folder-level config (layer 2).
    pub fn with_folder(mut self, folder: FolderConfig) -> Self {
        self.folder = Some(folder);
        self
    }

    /// Apply agent-level config (layer 3).
    pub fn with_agent(mut self, agent: AgentConfig) -> Self {
        self.agent = Some(agent);
        self
    }

    /// Apply chat-level config (layer 4).
    pub fn with_chat(mut self, chat: ChatConfig) -> Self {
        self.chat = Some(chat);
        self
    }

    /// Resolve all layers into a single flattened config.
    ///
    /// Priority: global < folder < agent < chat (higher wins).
    pub fn resolve(self) -> ResolvedConfig {
        let g = self.global;
        let f = self.folder.as_ref();
        let a = self.agent.as_ref();
        let c = self.chat.as_ref();

        // ── General (only from global) ────────────────────────────────
        let app_name = g.general.app_name.clone();
        let version = g.general.version.clone();
        let language = g.general.language.clone();
        let theme = g.general.theme.clone();
        let data_dir = g.general.data_dir.clone();

        // ── Default model ─────────────────────────────────────────────
        let default_model = c
            .and_then(|c| c.model.clone())
            .or_else(|| a.and_then(|a| a.model.clone()))
            .or_else(|| f.and_then(|f| f.default_model.clone()))
            .unwrap_or_else(|| g.default_model.clone());

        // ── Providers (from global only) ──────────────────────────────
        let providers = g.providers.clone();

        // ── Approval mode ─────────────────────────────────────────────
        let approval_mode = c
            .and_then(|c| c.approval_mode)
            .or_else(|| a.and_then(|a| a.approval_mode))
            .or_else(|| f.and_then(|f| f.approval_mode))
            .unwrap_or(g.approval_mode);

        // ── UI (from global only) ─────────────────────────────────────
        let sidebar_width = g.ui.sidebar_width;
        let show_status_bar = g.ui.show_status_bar;
        let font_size = g.ui.font_size;

        // ── RAG ───────────────────────────────────────────────────────
        let rag_enabled = c
            .and_then(|c| c.rag_enabled)
            .or_else(|| f.map(|f| f.rag_enabled))
            .unwrap_or(false);

        let rag_paths = f.map(|f| f.rag_paths.clone()).unwrap_or_default();

        let chunk_size = g.rag.chunk_size;
        let chunk_overlap = g.rag.chunk_overlap;
        let top_k = g.rag.top_k;

        // ── System prompt ─────────────────────────────────────────────
        let system_prompt = c
            .and_then(|c| c.system_prompt.clone())
            .or_else(|| a.map(|a| a.system_prompt.clone()))
            .or_else(|| f.and_then(|f| f.system_prompt.clone()))
            .unwrap_or_default();

        // ── Tools ─────────────────────────────────────────────────────
        let mut tools: Vec<String> = a.map(|a| a.tools.clone()).unwrap_or_default();
        if let Some(chat) = c {
            tools.extend(chat.tools.iter().cloned());
        }

        // ── MCP servers ───────────────────────────────────────────────
        let mut mcp_servers: Vec<String> = a.map(|a| a.mcp_servers.clone()).unwrap_or_default();
        if let Some(chat) = c {
            mcp_servers.extend(chat.mcp_servers.iter().cloned());
        }

        // ── Max turns ─────────────────────────────────────────────────
        let max_turns = c
            .and_then(|c| c.max_turns)
            .or_else(|| a.and_then(|a| a.max_turns));

        // ── Project name ──────────────────────────────────────────────
        let project_name = f.map(|f| f.project_name.clone()).filter(|n| !n.is_empty());

        ResolvedConfig {
            app_name,
            version,
            language,
            theme,
            data_dir,
            default_model,
            providers,
            approval_mode,
            sidebar_width,
            show_status_bar,
            font_size,
            rag_enabled,
            rag_paths,
            chunk_size,
            chunk_overlap,
            top_k,
            system_prompt,
            tools,
            mcp_servers,
            max_turns,
            project_name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_with_defaults_only() {
        let resolver = ConfigResolver::with_defaults();
        let resolved = resolver.resolve();
        assert_eq!(resolved.default_model, "anthropic/claude-sonnet-4");
        assert_eq!(resolved.approval_mode, ApprovalMode::Normal);
        assert!(resolved.system_prompt.is_empty());
    }

    #[test]
    fn folder_overrides_model() {
        let resolver = ConfigResolver::with_defaults().with_folder(FolderConfig {
            project_name: "my-project".into(),
            system_prompt: None,
            rag_enabled: true,
            rag_paths: vec!["./docs".into()],
            default_model: Some("openai/gpt-4o".into()),
            approval_mode: None,
        });
        let resolved = resolver.resolve();
        assert_eq!(resolved.default_model, "openai/gpt-4o");
        assert_eq!(resolved.project_name.as_deref(), Some("my-project"));
        assert!(resolved.rag_enabled);
    }

    #[test]
    fn agent_overrides_folder() {
        let mut agent = AgentConfig::default_test();
        agent.name = "coder".into();
        agent.model = Some("ollama/codellama".into());
        agent.approval_mode = Some(ApprovalMode::Auto);
        agent.system_prompt = "You are a coding assistant.".into();

        let resolver = ConfigResolver::with_defaults()
            .with_folder(FolderConfig {
                project_name: "proj".into(),
                system_prompt: None,
                rag_enabled: false,
                rag_paths: vec![],
                default_model: Some("openai/gpt-4o".into()),
                approval_mode: Some(ApprovalMode::Strict),
            })
            .with_agent(agent);

        let resolved = resolver.resolve();
        // Agent overrides folder
        assert_eq!(resolved.default_model, "ollama/codellama");
        assert_eq!(resolved.approval_mode, ApprovalMode::Auto);
        assert_eq!(resolved.system_prompt, "You are a coding assistant.");
        // But project_name still comes from folder
        assert_eq!(resolved.project_name.as_deref(), Some("proj"));
    }

    #[test]
    fn chat_overrides_everything() {
        let mut agent = AgentConfig::default_test();
        agent.name = "coder".into();
        agent.model = Some("agent-model".into());
        agent.system_prompt = "agent prompt".into();

        let chat = ChatConfig {
            model: Some("chat-model".into()),
            system_prompt: Some("chat prompt".into()),
            approval_mode: Some(ApprovalMode::Strict),
            max_turns: Some(10),
            ..Default::default()
        };

        let resolver = ConfigResolver::with_defaults()
            .with_agent(agent)
            .with_chat(chat);

        let resolved = resolver.resolve();
        assert_eq!(resolved.default_model, "chat-model");
        assert_eq!(resolved.system_prompt, "chat prompt");
        assert_eq!(resolved.approval_mode, ApprovalMode::Strict);
        assert_eq!(resolved.max_turns, Some(10));
    }
}

/// Helper for tests — a minimal default AgentConfig.
impl AgentConfig {
    #[cfg(test)]
    fn default_test() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            system_prompt: String::new(),
            model: None,
            tools: vec![],
            mcp_servers: vec![],
            approval_mode: None,
            max_turns: None,
        }
    }
}
