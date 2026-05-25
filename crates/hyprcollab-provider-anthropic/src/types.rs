//! Anthropic-specific request/response types and conversions to/from core types.

use serde::{Deserialize, Serialize};

use hyprcollab_core::traits::ToolDefinition;
use hyprcollab_core::types::*;

// ── Messages ────────────────────────────────────────────────────────

/// Content block within an Anthropic message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

/// An Anthropic API message (role + content).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: serde_json::Value,
}

impl AnthropicMessage {
    /// Convert a core `Message` into an Anthropic message.
    /// System messages are handled separately (extracted at request level).
    pub fn from_core(msg: &Message) -> Option<Self> {
        let role = match msg.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            // System messages are extracted separately — skip them here
            MessageRole::System => return None,
            // Anthropic uses tool_result content blocks inside user messages
            MessageRole::Tool => "user",
        };

        // If the message has tool calls, emit content blocks
        if !msg.tool_calls.is_empty() {
            let mut blocks: Vec<serde_json::Value> = vec![];
            // Include any text content as well
            if !msg.content.is_empty() {
                blocks.push(serde_json::json!({
                    "type": "text",
                    "text": msg.content
                }));
            }
            for tc in &msg.tool_calls {
                blocks.push(serde_json::json!({
                    "type": "tool_use",
                    "id": tc.id,
                    "name": tc.name,
                    "input": tc.arguments
                }));
            }
            return Some(Self {
                role: role.to_string(),
                content: serde_json::Value::Array(blocks),
            });
        }

        // Tool-role messages get sent as tool_result content blocks inside a user message
        if msg.role == MessageRole::Tool {
            let tool_use_id = msg
                .metadata
                .get("tool_call_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            return Some(Self {
                role: "user".to_string(),
                content: serde_json::json!([{
                    "type": "tool_result",
                    "tool_use_id": tool_use_id,
                    "content": msg.content
                }]),
            });
        }

        // Simple text message
        Some(Self {
            role: role.to_string(),
            content: serde_json::Value::String(msg.content.clone()),
        })
    }
}

// ── Tool definition (Anthropic format) ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicTool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
}

impl AnthropicTool {
    pub fn from_core(td: &ToolDefinition) -> Self {
        Self {
            name: td.name.clone(),
            description: Some(td.description.clone()),
            input_schema: td.parameters.clone(),
        }
    }
}

// ── Request ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicRequest {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    pub messages: Vec<AnthropicMessage>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<AnthropicTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

impl AnthropicRequest {
    /// Build an Anthropic request from a core `ChatRequest`.
    /// Extracts system messages into the top-level `system` field.
    pub fn from_core(request: &ChatRequest) -> Self {
        let mut system_parts: Vec<String> = vec![];
        let mut messages: Vec<AnthropicMessage> = vec![];

        for msg in &request.messages {
            if msg.role == MessageRole::System {
                system_parts.push(msg.content.clone());
            } else if let Some(am) = AnthropicMessage::from_core(msg) {
                messages.push(am);
            }
        }

        let system = if system_parts.is_empty() {
            None
        } else {
            Some(system_parts.join("\n"))
        };

        let tools: Vec<AnthropicTool> = request
            .tools
            .iter()
            .map(AnthropicTool::from_core)
            .collect();

        Self {
            model: request.model.clone(),
            system,
            messages,
            max_tokens: request.max_tokens.unwrap_or(4096),
            temperature: request.temperature,
            tools,
            stream: if request.stream { Some(true) } else { None },
        }
    }
}

// ── Response ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub resp_type: Option<String>,
    pub role: Option<String>,
    pub content: Vec<AnthropicContentBlock>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
    pub usage: AnthropicUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicUsage {
    #[serde(default)]
    pub input_tokens: u32,
    pub output_tokens: u32,
}

fn parse_stop_reason(s: &Option<String>) -> FinishReason {
    match s.as_deref() {
        Some("end_turn") | Some("stop") => FinishReason::Stop,
        Some("tool_use") => FinishReason::ToolCalls,
        Some("max_tokens") => FinishReason::Length,
        _ => FinishReason::Stop,
    }
}

impl AnthropicResponse {
    pub fn into_core(self, chat_id: ChatId) -> ChatResponse {
        let mut text_content = String::new();
        let mut tool_calls: Vec<ToolCall> = vec![];

        for block in &self.content {
            match block {
                AnthropicContentBlock::Text { text } => {
                    text_content.push_str(text);
                }
                AnthropicContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: input.clone(),
                    });
                }
                _ => {}
            }
        }

        let message = Message {
            id: MessageId::new(),
            chat_id,
            role: MessageRole::Assistant,
            content: text_content,
            tool_calls,
            artifacts: vec![],
            timestamp: chrono::Utc::now(),
            metadata: serde_json::Value::Null,
            parent_id: None,
        };

        ChatResponse {
            id: self.id,
            message,
            usage: TokenUsage {
                prompt_tokens: self.usage.input_tokens,
                completion_tokens: self.usage.output_tokens,
                total_tokens: self.usage.input_tokens + self.usage.output_tokens,
            },
            model: self.model,
            finish_reason: parse_stop_reason(&self.stop_reason),
        }
    }
}

// ── Stream events ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicStreamMessageStart {
    #[serde(rename = "type")]
    pub event_type: Option<String>,
    pub message: Option<AnthropicResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicStreamContentBlockStart {
    pub index: Option<u32>,
    #[serde(rename = "content_block")]
    pub content_block: Option<AnthropicContentBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicStreamContentBlockDelta {
    pub index: Option<u32>,
    pub delta: AnthropicDelta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicDelta {
    #[serde(rename = "type")]
    pub delta_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicStreamMessageDelta {
    pub delta: AnthropicMessageDeltaInfo,
    pub usage: Option<AnthropicUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessageDeltaInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
}

/// Top-level streaming event envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    // The actual payload depends on event_type; we parse the raw value as needed.
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_message_serialization_string_content() {
        let msg = AnthropicMessage {
            role: "user".to_string(),
            content: serde_json::Value::String("Hello".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"Hello\""));
    }

    #[test]
    fn test_anthropic_message_serialization_array_content() {
        let msg = AnthropicMessage {
            role: "assistant".to_string(),
            content: serde_json::json!([{"type": "text", "text": "Hi there"}]),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"assistant\""));
        assert!(json.contains("text"));
    }

    #[test]
    fn test_anthropic_request_system_extraction() {
        let core_req = ChatRequest {
            model: "claude-sonnet-4-20250514".to_string(),
            messages: vec![
                Message {
                    id: MessageId::new(),
                    chat_id: ChatId::new(),
                    role: MessageRole::System,
                    content: "You are helpful.".to_string(),
                    tool_calls: vec![],
                    artifacts: vec![],
                    timestamp: chrono::Utc::now(),
                    metadata: serde_json::Value::Null,
                    parent_id: None,
                },
                Message {
                    id: MessageId::new(),
                    chat_id: ChatId::new(),
                    role: MessageRole::User,
                    content: "Hello".to_string(),
                    tool_calls: vec![],
                    artifacts: vec![],
                    timestamp: chrono::Utc::now(),
                    metadata: serde_json::Value::Null,
                    parent_id: None,
                },
            ],
            tools: vec![],
            temperature: None,
            max_tokens: Some(1024),
            stream: false,
            persona: None,
            agent_role: None,
        };

        let anthropic_req = AnthropicRequest::from_core(&core_req);
        assert_eq!(anthropic_req.system, Some("You are helpful.".to_string()));
        assert_eq!(anthropic_req.messages.len(), 1);
        assert_eq!(anthropic_req.messages[0].role, "user");
        assert_eq!(anthropic_req.max_tokens, 1024);
    }

    #[test]
    fn test_anthropic_response_deserialization() {
        let json = r#"{
            "id": "msg_123",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "text", "text": "Hello!"}],
            "model": "claude-sonnet-4-20250514",
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": {"input_tokens": 10, "output_tokens": 5}
        }"#;
        let resp: AnthropicResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, "msg_123");
        assert_eq!(resp.content.len(), 1);
        assert_eq!(resp.stop_reason, Some("end_turn".to_string()));
        assert_eq!(resp.usage.input_tokens, 10);
        assert_eq!(resp.usage.output_tokens, 5);
    }

    #[test]
    fn test_stream_content_block_delta_text() {
        let json = r#"{
            "type": "content_block_delta",
            "index": 0,
            "delta": {"type": "text_delta", "text": "Hello"}
        }"#;
        let delta_event: AnthropicStreamContentBlockDelta =
            serde_json::from_str(json).unwrap();
        assert_eq!(delta_event.delta.text, Some("Hello".to_string()));
    }

    #[test]
    fn test_stream_message_delta_stop_reason() {
        let json = r#"{
            "type": "message_delta",
            "delta": {"stop_reason": "end_turn"},
            "usage": {"output_tokens": 15}
        }"#;
        let msg_delta: AnthropicStreamMessageDelta = serde_json::from_str(json).unwrap();
        assert_eq!(msg_delta.delta.stop_reason, Some("end_turn".to_string()));
        assert!(msg_delta.usage.is_some());
        let usage = msg_delta.usage.unwrap();
        assert_eq!(usage.output_tokens, 15);
    }

    #[test]
    fn test_conversion_chat_request_to_anthropic_request() {
        let core_req = ChatRequest {
            model: "claude-sonnet-4-20250514".to_string(),
            messages: vec![
                Message {
                    id: MessageId::new(),
                    chat_id: ChatId::new(),
                    role: MessageRole::System,
                    content: "Be concise.".to_string(),
                    tool_calls: vec![],
                    artifacts: vec![],
                    timestamp: chrono::Utc::now(),
                    metadata: serde_json::Value::Null,
                    parent_id: None,
                },
                Message {
                    id: MessageId::new(),
                    chat_id: ChatId::new(),
                    role: MessageRole::User,
                    content: "What is 2+2?".to_string(),
                    tool_calls: vec![],
                    artifacts: vec![],
                    timestamp: chrono::Utc::now(),
                    metadata: serde_json::Value::Null,
                    parent_id: None,
                },
            ],
            tools: vec![ToolDefinition {
                name: "calculator".to_string(),
                description: "Do math".to_string(),
                parameters: serde_json::json!({"type": "object"}),
            }],
            temperature: Some(0.5),
            max_tokens: Some(200),
            stream: false,
            persona: None,
            agent_role: None,
        };

        let anthropic_req = AnthropicRequest::from_core(&core_req);
        assert_eq!(anthropic_req.model, "claude-sonnet-4-20250514");
        assert_eq!(anthropic_req.system, Some("Be concise.".to_string()));
        assert_eq!(anthropic_req.messages.len(), 1);
        assert_eq!(anthropic_req.messages[0].role, "user");
        assert_eq!(anthropic_req.tools.len(), 1);
        assert_eq!(anthropic_req.tools[0].name, "calculator");
        assert_eq!(anthropic_req.temperature, Some(0.5));
        assert_eq!(anthropic_req.max_tokens, 200);
    }

    #[test]
    fn test_conversion_anthropic_response_to_chat_response() {
        let anthropic_resp = AnthropicResponse {
            id: "msg_abc".to_string(),
            resp_type: Some("message".to_string()),
            role: Some("assistant".to_string()),
            content: vec![AnthropicContentBlock::Text {
                text: "World".to_string(),
            }],
            model: "claude-sonnet-4-20250514".to_string(),
            stop_reason: Some("end_turn".to_string()),
            stop_sequence: None,
            usage: AnthropicUsage {
                input_tokens: 5,
                output_tokens: 3,
            },
        };

        let chat_id = ChatId::new();
        let core_resp = anthropic_resp.into_core(chat_id);
        assert_eq!(core_resp.id, "msg_abc");
        assert_eq!(core_resp.message.content, "World");
        assert_eq!(core_resp.message.role, MessageRole::Assistant);
        assert_eq!(core_resp.usage.prompt_tokens, 5);
        assert_eq!(core_resp.usage.completion_tokens, 3);
        assert_eq!(core_resp.usage.total_tokens, 8);
        assert_eq!(core_resp.finish_reason, FinishReason::Stop);
    }

    #[test]
    fn test_tool_definition_conversion() {
        let td = ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get current weather".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                }
            }),
        };

        let anthropic_tool = AnthropicTool::from_core(&td);
        assert_eq!(anthropic_tool.name, "get_weather");
        assert_eq!(anthropic_tool.description, Some("Get current weather".to_string()));
        assert!(anthropic_tool.input_schema["properties"]["location"].is_object());
    }

    #[test]
    fn test_anthropic_response_with_tool_use() {
        let json = r#"{
            "id": "msg_tool",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "Let me check that."},
                {"type": "tool_use", "id": "toolu_123", "name": "search", "input": {"query": "rust"}}
            ],
            "model": "claude-sonnet-4-20250514",
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 20, "output_tokens": 10}
        }"#;
        let resp: AnthropicResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.content.len(), 2);

        let chat_id = ChatId::new();
        let core_resp = resp.into_core(chat_id);
        assert_eq!(core_resp.message.content, "Let me check that.");
        assert_eq!(core_resp.message.tool_calls.len(), 1);
        assert_eq!(core_resp.message.tool_calls[0].id, "toolu_123");
        assert_eq!(core_resp.message.tool_calls[0].name, "search");
        assert_eq!(core_resp.finish_reason, FinishReason::ToolCalls);
    }
}
