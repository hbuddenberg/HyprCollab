use serde::{Deserialize, Serialize};

// ── ID newtypes ──────────────────────────────────────────────────────

macro_rules! id_newtype {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub uuid::Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(uuid::Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<uuid::Uuid> for $name {
            fn from(id: uuid::Uuid) -> Self {
                Self(id)
            }
        }

        impl From<$name> for uuid::Uuid {
            fn from(id: $name) -> Self {
                id.0
            }
        }
    };
}

id_newtype!(ChatId);
id_newtype!(MessageId);
id_newtype!(PersonaId);
id_newtype!(AgentRoleId);
id_newtype!(WorkspaceId);

// ── Enums ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Assistant => write!(f, "assistant"),
            Self::System => write!(f, "system"),
            Self::Tool => write!(f, "tool"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    ToolCalls,
    Length,
    ContentFilter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    Auto,
    Normal,
    Strict,
}

impl std::fmt::Display for ApprovalMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Normal => write!(f, "normal"),
            Self::Strict => write!(f, "strict"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    Code,
    Markdown,
    Html,
    Image,
    Mermaid,
    File,
}

// ── Structs ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub artifact_type: ArtifactType,
    pub title: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub chat_id: ChatId,
    pub role: MessageRole,
    pub content: String,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    /// Parent message ID for conversation branching (None = root message).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<MessageId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub tools: Vec<crate::traits::ToolDefinition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persona: Option<PersonaId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_role: Option<AgentRoleId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub message: Message,
    pub usage: TokenUsage,
    pub model: String,
    pub finish_reason: FinishReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenChunk {
    pub delta: String,
    pub finish_reason: Option<FinishReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
    /// Assembled tool calls, populated only when `finish_reason == ToolCalls`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
}

/// Shared execution context passed to every slash command.
///
/// Holds mutable per-session settings so commands like `/temperature` and
/// `/approval` can persist their changes across the current session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandContext {
    pub session_id: String,
    pub workspace_id: Option<WorkspaceId>,
    pub model: String,
    pub temperature: Option<f32>,
    pub approval_mode: ApprovalMode,
}

impl Default for CommandContext {
    fn default() -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            workspace_id: None,
            model: "openai/gpt-4o".to_string(),
            temperature: None,
            approval_mode: ApprovalMode::Normal,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub context_length: u32,
    pub supports_streaming: bool,
    pub supports_tools: bool,
    pub supports_vision: bool,
}

/// Structured SSE event types for the streaming chat API (S10).
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub enum SseEvent {
    Token { content: String },
    ToolCall { name: String, #[schema(value_type = Object)] args: serde_json::Value, id: String },
    ToolResult { id: String, output: String, duration_ms: u64 },
    Thinking { content: String },
    ApprovalRequest { tool: String, #[schema(value_type = Object)] args: serde_json::Value, id: String },
    Done { usage: TokenUsage },
}

#[cfg(test)]
mod sse_event_tests {
    use super::*;

    #[test]
    fn sse_event_token_round_trips() {
        let evt = SseEvent::Token { content: "hello".into() };
        let json = serde_json::to_string(&evt).unwrap();
        let back: SseEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, SseEvent::Token { .. }));
    }

    #[test]
    fn sse_event_tool_call_round_trips() {
        let evt = SseEvent::ToolCall {
            name: "shell".into(),
            args: serde_json::json!({"cmd": "ls"}),
            id: "call_123".into(),
        };
        let json = serde_json::to_string(&evt).unwrap();
        let back: SseEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, SseEvent::ToolCall { .. }));
    }

    #[test]
    fn sse_event_done_round_trips() {
        let evt = SseEvent::Done {
            usage: TokenUsage { prompt_tokens: 10, completion_tokens: 50, total_tokens: 60 },
        };
        let json = serde_json::to_string(&evt).unwrap();
        let back: SseEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, SseEvent::Done { .. }));
    }

    #[test]
    fn sse_event_all_variants_serialize() {
        let variants: Vec<SseEvent> = vec![
            SseEvent::Token { content: "tok".into() },
            SseEvent::ToolCall { name: "t".into(), args: serde_json::Value::Null, id: "i".into() },
            SseEvent::ToolResult { id: "i".into(), output: "out".into(), duration_ms: 100 },
            SseEvent::Thinking { content: "hmm".into() },
            SseEvent::ApprovalRequest {
                tool: "shell".into(),
                args: serde_json::json!({"cmd": "rm -rf /"}),
                id: "req_456".into(),
            },
            SseEvent::Done {
                usage: TokenUsage { prompt_tokens: 1, completion_tokens: 2, total_tokens: 3 },
            },
        ];
        for v in &variants {
            assert!(serde_json::to_string(v).is_ok());
        }
    }
}
