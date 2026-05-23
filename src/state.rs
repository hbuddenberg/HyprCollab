use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::ipc::*;

/// Daemon state — holds all app data in memory
#[derive(Debug, Clone)]
pub struct State {
    pub folders: Vec<Folder>,
    pub active_folder_idx: Option<usize>,
    pub active_chat_idx: Option<usize>,
    pub agents: Vec<AgentDef>,
    pub active_agent_idx: usize,
    pub models: Vec<ModelDef>,
    pub active_model_idx: usize,
    pub sidebar_visible: bool,
    pub working: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub chats: Vec<Chat>,
    pub git_branch: Option<String>,
    pub workdir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chat {
    pub id: String,
    pub title: String,
    pub messages: Vec<Message>,
    pub agent: String,
    pub model: String,
    pub tokens_used: u64,
    pub created_at: i64,
    #[serde(default)]
    pub system_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub timestamp: i64,
    pub tokens: u32,
}

#[derive(Debug, Clone)]
pub struct AgentDef {
    pub name: String,
    pub available: bool,
    pub icon: String,
}

#[derive(Debug, Clone)]
pub struct ModelDef {
    pub name: String,
    pub provider: String,
    pub available: bool,
}

impl State {
    pub fn new() -> Self {
        let now = now_secs();

        Self {
            folders: vec![
                Folder {
                    id: Uuid::new_v4().to_string(),
                    name: "Uncategorized".into(),
                    icon: "\u{f128}".into(),
                    git_branch: None,
                    workdir: None,
                    chats: vec![
                        Chat {
                            id: Uuid::new_v4().to_string(),
                            title: "Quick question".into(),
                            messages: vec![],
                            agent: "openai".into(),
                            model: "gpt-4o".into(),
                            tokens_used: 0,
                            created_at: now - 600,
                            system_prompt: None,
                        },
                    ],
                },
                Folder {
                    id: Uuid::new_v4().to_string(),
                    name: "Dev".into(),
                    icon: "\u{f121}".into(),
                    git_branch: Some("feature/api-fix".into()),
                    workdir: Some("~/developments/api-project".into()),
                    chats: vec![
                        Chat {
                            id: Uuid::new_v4().to_string(),
                            title: "Fix API bug".into(),
                            messages: vec![
                                Message {
                                    role: "user".into(),
                                    content: "El endpoint /api/users retorna 500".into(),
                                    timestamp: now - 3600,
                                    tokens: 24,
                                },
                                Message {
                                    role: "assistant".into(),
                                    content: "Vamos a debuggear. Necesito revisar los logs del servidor para identificar la causa del error 500.".into(),
                                    timestamp: now - 3590,
                                    tokens: 87,
                                },
                            ],
                            agent: "openai".into(),
                            model: "gpt-4o".into(),
                            tokens_used: 111,
                            created_at: now - 7200,
                            system_prompt: None,
                        },
                        Chat {
                            id: Uuid::new_v4().to_string(),
                            title: "Refactor database".into(),
                            messages: vec![],
                            agent: "openai".into(),
                            model: "gpt-4o".into(),
                            tokens_used: 0,
                            created_at: now - 1800,
                            system_prompt: None,
                        },
                        Chat {
                            id: Uuid::new_v4().to_string(),
                            title: "Deploy script".into(),
                            messages: vec![],
                            agent: "claude-code".into(),
                            model: "claude-sonnet-4".into(),
                            tokens_used: 0,
                            created_at: now - 900,
                            system_prompt: None,
                        },
                    ],
                },
                Folder {
                    id: Uuid::new_v4().to_string(),
                    name: "System".into(),
                    icon: "\u{f108}".into(),
                    git_branch: None,
                    workdir: None,
                    chats: vec![
                        Chat {
                            id: Uuid::new_v4().to_string(),
                            title: "Config Hyprland".into(),
                            messages: vec![],
                            agent: "opencode".into(),
                            model: "gemini-2.5-pro".into(),
                            tokens_used: 0,
                            created_at: now - 5400,
                            system_prompt: None,
                        },
                    ],
                },
                Folder {
                    id: Uuid::new_v4().to_string(),
                    name: "Research".into(),
                    icon: "\u{f002}".into(),
                    git_branch: Some("main".into()),
                    workdir: Some("~/Documents/research".into()),
                    chats: vec![
                        Chat {
                            id: Uuid::new_v4().to_string(),
                            title: "RAG implementation".into(),
                            messages: vec![],
                            agent: "openai".into(),
                            model: "gpt-4o".into(),
                            tokens_used: 0,
                            created_at: now - 10800,
                            system_prompt: None,
                        },
                    ],
                },
            ],
            active_folder_idx: Some(0),
            active_chat_idx: Some(0),
            agents: Self::default_agents(),
            active_agent_idx: 0,
            models: Self::default_models(),
            active_model_idx: 0,
            sidebar_visible: true,
            working: false,
        }
    }

    /// Load state from disk. Returns Err if no data exists yet (caller falls back to new()).
    pub fn load() -> anyhow::Result<Self> {
        let folders = crate::storage::folders::load_all_folders()?;
        if folders.is_empty() {
            return Err(anyhow::anyhow!("no persisted data on disk"));
        }
        let active_chat = folders.first()
            .and_then(|f| if f.chats.is_empty() { None } else { Some(0) });
        Ok(Self {
            folders,
            active_folder_idx: Some(0),
            active_chat_idx: active_chat,
            agents: Self::default_agents(),
            active_agent_idx: 0,
            models: Self::default_models(),
            active_model_idx: 0,
            sidebar_visible: true,
            working: false,
        })
    }

    fn default_agents() -> Vec<AgentDef> {
        vec![
            AgentDef { name: "openai".into(), available: true, icon: "\u{f187}".into() },
            AgentDef { name: "claude-code".into(), available: true, icon: "\u{e63e}".into() },
            AgentDef { name: "opencode".into(), available: true, icon: "\u{f121}".into() },
        ]
    }

    fn default_models() -> Vec<ModelDef> {
        vec![
            ModelDef { name: "gpt-4o".into(), provider: "openai".into(), available: true },
            ModelDef { name: "gpt-4o-mini".into(), provider: "openai".into(), available: true },
            ModelDef { name: "claude-sonnet-4".into(), provider: "anthropic".into(), available: true },
            ModelDef { name: "gemini-2.5-pro".into(), provider: "google".into(), available: true },
            ModelDef { name: "deepseek-v4".into(), provider: "deepseek".into(), available: true },
        ]
    }

    /// Convert to IPC snapshot
    pub fn snapshot(&self) -> StateSnapshot {
        let active_messages = self.active_chat()
            .map(|c| c.messages.iter().map(|m| MessageInfo {
                role: m.role.clone(),
                content: m.content.clone(),
                timestamp: m.timestamp,
                tokens: m.tokens,
            }).collect())
            .unwrap_or_default();

        let active_chat_title = self.active_chat()
            .map(|c| c.title.clone())
            .unwrap_or_default();

        let active_chat_tokens = self.active_chat()
            .map(|c| c.tokens_used)
            .unwrap_or(0);

        let active_git_branch = self.active_folder()
            .and_then(|f| f.git_branch.clone());

        StateSnapshot {
            folders: self.folders.iter().map(|f| FolderInfo {
                name: f.name.clone(),
                icon: f.icon.clone(),
                chats: f.chats.iter().map(|c| ChatInfo {
                    title: c.title.clone(),
                    last_active: c.created_at,
                }).collect(),
                git_branch: f.git_branch.clone(),
            }).collect(),
            active_folder: self.active_folder_idx,
            active_chat: self.active_chat_idx,
            agents: self.agents.iter().map(|a| AgentInfo {
                name: a.name.clone(),
                available: a.available,
                icon: a.icon.clone(),
            }).collect(),
            active_agent: self.agents.get(self.active_agent_idx)
                .map(|a| a.name.clone())
                .unwrap_or_default(),
            models: self.models.iter().map(|m| ModelInfo {
                name: m.name.clone(),
                provider: m.provider.clone(),
                available: m.available,
            }).collect(),
            active_model: self.models.get(self.active_model_idx)
                .map(|m| m.name.clone())
                .unwrap_or_default(),
            sidebar_visible: self.sidebar_visible,
            active_messages,
            active_chat_title,
            active_chat_tokens,
            active_git_branch,
            working: self.working,
            system_prompt: self.active_chat().and_then(|c| c.system_prompt.clone()),
        }
    }

    pub fn active_chat(&self) -> Option<&Chat> {
        let fi = self.active_folder_idx?;
        let ci = self.active_chat_idx?;
        self.folders.get(fi).and_then(|f| f.chats.get(ci))
    }

    pub fn active_chat_mut(&mut self) -> Option<&mut Chat> {
        let fi = self.active_folder_idx?;
        let ci = self.active_chat_idx?;
        self.folders.get_mut(fi).and_then(|f| f.chats.get_mut(ci))
    }

    pub fn active_folder(&self) -> Option<&Folder> {
        self.active_folder_idx.and_then(|i| self.folders.get(i))
    }

    pub fn active_folder_id(&self) -> Option<String> {
        self.active_folder().map(|f| f.id.clone())
    }

    pub fn active_chat_id(&self) -> Option<String> {
        self.active_chat().map(|c| c.id.clone())
    }

    /// Handle an IPC request, mutating state
    pub fn handle(&mut self, req: Request) -> Response {
        match req {
            Request::GetState => Response::State { data: self.snapshot() },
            Request::Ping => Response::Pong,

            Request::SendMessage { content } => {
                let model_name = self.models.get(self.active_model_idx)
                    .map(|m| m.name.clone())
                    .unwrap_or_default();

                // Capture IDs before mutable borrow
                let folder_id = self.active_folder_id();
                let chat_id = self.active_chat_id();

                if let Some(chat) = self.active_chat_mut() {
                    let now = now_secs();
                    let user_tokens = (content.len() / 4) as u32;
                    let user_msg = Message {
                        role: "user".into(),
                        content: content.clone(),
                        timestamp: now,
                        tokens: user_tokens,
                    };
                    chat.messages.push(user_msg.clone());
                    chat.tokens_used += user_tokens as u64;

                    // Persist user message (best-effort — errors are logged, not propagated)
                    if let (Some(ref fid), Some(ref cid)) = (&folder_id, &chat_id) {
                        if let Err(e) = crate::storage::messages::append_message(fid, cid, &user_msg) {
                            tracing::warn!("persist user msg: {}", e);
                        }
                    }

                    // Mock assistant response — replaced by real LLM call in Fase B
                    let resp_content = format!(
                        "Recibido: \"{}\". Procesando con {}...",
                        &content[..content.len().min(40)],
                        model_name
                    );
                    let resp_tokens = (resp_content.len() / 4) as u32;
                    let assistant_msg = Message {
                        role: "assistant".into(),
                        content: resp_content,
                        timestamp: now_secs(),
                        tokens: resp_tokens,
                    };
                    chat.messages.push(assistant_msg.clone());
                    chat.tokens_used += resp_tokens as u64;

                    // Persist assistant message (best-effort)
                    if let (Some(ref fid), Some(ref cid)) = (&folder_id, &chat_id) {
                        if let Err(e) = crate::storage::messages::append_message(fid, cid, &assistant_msg) {
                            tracing::warn!("persist assistant msg: {}", e);
                        }
                    }

                    Response::State { data: self.snapshot() }
                } else {
                    Response::Error { msg: "No active chat".into() }
                }
            }

            Request::SelectChat { folder, chat } => {
                if folder < self.folders.len() {
                    if chat < self.folders[folder].chats.len() {
                        self.active_folder_idx = Some(folder);
                        self.active_chat_idx = Some(chat);
                        Response::State { data: self.snapshot() }
                    } else {
                        Response::Error { msg: "Invalid chat index".into() }
                    }
                } else {
                    Response::Error { msg: "Invalid folder index".into() }
                }
            }

            Request::NewChat { title } => {
                let agent = self.agents.get(self.active_agent_idx)
                    .map(|a| a.name.clone())
                    .unwrap_or_default();
                let model = self.models.get(self.active_model_idx)
                    .map(|m| m.name.clone())
                    .unwrap_or_default();

                if let Some(fi) = self.active_folder_idx {
                    // Capture folder_id before mutable borrow
                    let folder_id = self.folders.get(fi).map(|f| f.id.clone());

                    if let Some(folder) = self.folders.get_mut(fi) {
                        let new_chat = Chat {
                            id: Uuid::new_v4().to_string(),
                            title: title.clone(),
                            messages: vec![],
                            agent,
                            model,
                            tokens_used: 0,
                            created_at: now_secs(),
                            system_prompt: None,
                        };

                        // Persist before push (save_chat_metadata takes &Chat, no move)
                        if let Some(ref fid) = folder_id {
                            if let Err(e) = crate::storage::folders::save_chat_metadata(fid, &new_chat) {
                                tracing::warn!("persist chat: {}", e);
                            }
                        }

                        let idx = folder.chats.len();
                        folder.chats.push(new_chat);
                        self.active_chat_idx = Some(idx);
                        Response::State { data: self.snapshot() }
                    } else {
                        Response::Error { msg: "Invalid folder".into() }
                    }
                } else {
                    Response::Error { msg: "No active folder".into() }
                }
            }

            Request::NewFolder { name } => {
                let new_folder = Folder {
                    id: Uuid::new_v4().to_string(),
                    name: name.clone(),
                    icon: "\u{f07b}".into(),
                    chats: vec![],
                    git_branch: None,
                    workdir: None,
                };
                self.folders.push(new_folder);
                let idx = self.folders.len() - 1;
                self.active_folder_idx = Some(idx);
                self.active_chat_idx = None;

                // Persist folder metadata (best-effort)
                if let Some(folder) = self.folders.last() {
                    if let Err(e) = crate::storage::folders::save_folder_metadata(folder) {
                        tracing::warn!("persist folder: {}", e);
                    }
                }

                Response::State { data: self.snapshot() }
            }

            Request::ToggleSidebar => {
                self.sidebar_visible = !self.sidebar_visible;
                Response::State { data: self.snapshot() }
            }

            Request::SetSidebar { visible } => {
                self.sidebar_visible = visible;
                Response::State { data: self.snapshot() }
            }

            Request::SetAgent { name } => {
                for (i, a) in self.agents.iter().enumerate() {
                    if a.name.eq_ignore_ascii_case(&name) {
                        self.active_agent_idx = i;
                        return Response::Ack { msg: format!("Agent: {}", name) };
                    }
                }
                Response::Error { msg: format!("Unknown agent: {}", name) }
            }

            Request::SetModel { name } => {
                for (i, m) in self.models.iter().enumerate() {
                    if m.name.eq_ignore_ascii_case(&name) {
                        self.active_model_idx = i;
                        return Response::Ack { msg: format!("Model: {}", name) };
                    }
                }
                Response::Error { msg: format!("Unknown model: {}", name) }
            }

            Request::SetSystemPrompt { content } => {
                let folder_id = self.active_folder_id();
                let chat_id = self.active_chat_id();
                if let Some(chat) = self.active_chat_mut() {
                    chat.system_prompt = if content.trim().is_empty() {
                        None
                    } else {
                        Some(content)
                    };
                    if let (Some(ref fid), Some(ref cid)) = (&folder_id, &chat_id) {
                        if let Err(e) = crate::storage::folders::save_chat_metadata(fid, chat) {
                            tracing::warn!("persist system_prompt: {}", e);
                            let _ = cid; // suppress unused warning
                        }
                    }
                    Response::State { data: self.snapshot() }
                } else {
                    Response::Error { msg: "No active chat".into() }
                }
            }
        }
    }
}

pub(crate) fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
