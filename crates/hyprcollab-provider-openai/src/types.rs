//! OpenAI-specific request/response types and conversions to/from core types.

use serde::{Deserialize, Serialize};

use hyprcollab_core::types::*;
use hyprcollab_core::traits::ToolDefinition;

// ── Messages ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: OpenAiFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<OpenAiToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl OpenAiMessage {
    pub fn from_core(msg: &Message) -> Self {
        let role = match msg.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
            MessageRole::Tool => "tool",
        };

        let tool_calls: Vec<OpenAiToolCall> = msg
            .tool_calls
            .iter()
            .map(|tc| OpenAiToolCall {
                id: tc.id.clone(),
                call_type: "function".to_string(),
                function: OpenAiFunction {
                    name: tc.name.clone(),
                    arguments: tc.arguments.to_string(),
                },
            })
            .collect();

        // For tool-role messages, extract tool_call_id from metadata
        let tool_call_id = if msg.role == MessageRole::Tool {
            msg.metadata
                .get("tool_call_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };

        Self {
            role: role.to_string(),
            content: Some(msg.content.clone()),
            tool_calls,
            tool_call_id,
        }
    }
}

// ── Tool definition (OpenAI format) ────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OpenAiToolFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl OpenAiTool {
    pub fn from_core(td: &ToolDefinition) -> Self {
        Self {
            tool_type: "function".to_string(),
            function: OpenAiToolFunction {
                name: td.name.clone(),
                description: td.description.clone(),
                parameters: td.parameters.clone(),
            },
        }
    }
}

// ── Request ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiRequest {
    pub model: String,
    pub messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<OpenAiTool>,
}

impl OpenAiRequest {
    pub fn from_core(request: &ChatRequest) -> Self {
        let messages: Vec<OpenAiMessage> = request
            .messages
            .iter()
            .map(OpenAiMessage::from_core)
            .collect();

        let tools: Vec<OpenAiTool> = request
            .tools
            .iter()
            .map(OpenAiTool::from_core)
            .collect();

        Self {
            model: request.model.clone(),
            messages,
            stream: if request.stream { Some(true) } else { None },
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            tools,
        }
    }
}

// ── Response ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiResponse {
    pub id: String,
    pub object: Option<String>,
    pub created: Option<u64>,
    pub model: String,
    pub choices: Vec<OpenAiChoice>,
    pub usage: OpenAiUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiChoice {
    pub index: u32,
    pub message: OpenAiMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

fn parse_finish_reason(s: &Option<String>) -> FinishReason {
    match s.as_deref() {
        Some("stop") => FinishReason::Stop,
        Some("tool_calls") => FinishReason::ToolCalls,
        Some("length") => FinishReason::Length,
        Some("content_filter") => FinishReason::ContentFilter,
        _ => FinishReason::Stop,
    }
}

impl OpenAiResponse {
    pub fn into_core(self, chat_id: ChatId) -> ChatResponse {
        let choice = self
            .choices
            .into_iter()
            .next()
            .expect("OpenAI response must have at least one choice");

        let tool_calls: Vec<ToolCall> = choice
            .message
            .tool_calls
            .into_iter()
            .map(|tc| ToolCall {
                id: tc.id,
                name: tc.function.name,
                arguments: serde_json::from_str(&tc.function.arguments)
                    .unwrap_or(serde_json::Value::Null),
            })
            .collect();

        let content = choice.message.content.unwrap_or_default();

        let message = Message {
            id: MessageId::new(),
            chat_id,
            role: MessageRole::Assistant,
            content,
            tool_calls,
            artifacts: vec![],
            timestamp: chrono::Utc::now(),
            metadata: serde_json::Value::Null,
        };

        ChatResponse {
            id: self.id,
            message,
            usage: TokenUsage {
                prompt_tokens: self.usage.prompt_tokens,
                completion_tokens: self.usage.completion_tokens,
                total_tokens: self.usage.total_tokens,
            },
            model: self.model,
            finish_reason: parse_finish_reason(&choice.finish_reason),
        }
    }
}

// ── Stream chunk ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiStreamChunk {
    pub id: String,
    pub object: Option<String>,
    pub created: Option<u64>,
    pub model: String,
    pub choices: Vec<OpenAiStreamChoice>,
    #[serde(default)]
    pub usage: Option<OpenAiUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiStreamChoice {
    pub index: u32,
    pub delta: OpenAiDelta,
    pub finish_reason: Option<String>,
}

/// Incremental tool-call fragment streamed within a single SSE chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiStreamToolCallDelta {
    pub index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub call_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<OpenAiStreamFunctionDelta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiStreamFunctionDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<OpenAiStreamToolCallDelta>,
}

impl OpenAiStreamChunk {
    pub fn into_token_chunk(self) -> Option<TokenChunk> {
        let choice = self.choices.into_iter().next()?;

        let delta = choice.delta.content.unwrap_or_default();

        let usage = self.usage.map(|u| TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Some(TokenChunk {
            delta,
            finish_reason: if choice.finish_reason.is_some() {
                Some(parse_finish_reason(&choice.finish_reason))
            } else {
                None
            },
            usage,
            tool_calls: vec![],
        })
    }
}

// ── Models ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiModelsResponse {
    pub object: Option<String>,
    pub data: Vec<OpenAiModelData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiModelData {
    pub id: String,
    pub object: Option<String>,
    pub created: Option<u64>,
    pub owned_by: Option<String>,
}

// ── Embeddings ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiEmbeddingRequest {
    pub model: String,
    pub input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiEmbeddingResponse {
    pub object: Option<String>,
    pub data: Vec<OpenAiEmbeddingData>,
    pub model: String,
    pub usage: OpenAiUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiEmbeddingData {
    pub object: Option<String>,
    pub index: u32,
    pub embedding: Vec<f32>,
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_message_serialization() {
        let msg = OpenAiMessage {
            role: "user".to_string(),
            content: Some("Hello".to_string()),
            tool_calls: vec![],
            tool_call_id: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"Hello\""));
    }

    #[test]
    fn test_openai_request_serialization_with_tools() {
        let req = OpenAiRequest {
            model: "gpt-4".to_string(),
            messages: vec![OpenAiMessage {
                role: "user".to_string(),
                content: Some("test".to_string()),
                tool_calls: vec![],
                tool_call_id: None,
            }],
            stream: None,
            temperature: Some(0.7),
            max_tokens: Some(100),
            tools: vec![OpenAiTool {
                tool_type: "function".to_string(),
                function: OpenAiToolFunction {
                    name: "get_weather".to_string(),
                    description: "Get weather".to_string(),
                    parameters: serde_json::json!({"type": "object", "properties": {}}),
                },
            }],
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"tools\""));
        assert!(json.contains("\"get_weather\""));
    }

    #[test]
    fn test_response_deserialization() {
        let json = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1234567890,
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Hi there!"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        }"#;
        let resp: OpenAiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, "chatcmpl-123");
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(resp.choices[0].message.content.as_deref(), Some("Hi there!"));
    }

    #[test]
    fn test_stream_chunk_deserialization() {
        let json = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion.chunk",
            "created": 1234567890,
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "delta": {"content": "Hello"},
                "finish_reason": null
            }]
        }"#;
        let chunk: OpenAiStreamChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.choices[0].delta.content.as_deref(), Some("Hello"));
    }

    #[test]
    fn test_conversion_from_chat_request_to_openai_request() {
        let core_req = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![Message {
                id: MessageId::new(),
                chat_id: ChatId::new(),
                role: MessageRole::User,
                content: "Hello".to_string(),
                tool_calls: vec![],
                artifacts: vec![],
                timestamp: chrono::Utc::now(),
                metadata: serde_json::Value::Null,
            }],
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

        let openai_req = OpenAiRequest::from_core(&core_req);
        assert_eq!(openai_req.model, "gpt-4");
        assert_eq!(openai_req.messages.len(), 1);
        assert_eq!(openai_req.messages[0].role, "user");
        assert_eq!(openai_req.tools.len(), 1);
        assert_eq!(openai_req.tools[0].function.name, "calculator");
        assert_eq!(openai_req.temperature, Some(0.5));
        assert_eq!(openai_req.max_tokens, Some(200));
    }

    #[test]
    fn test_conversion_from_openai_response_to_chat_response() {
        let openai_resp = OpenAiResponse {
            id: "chatcmpl-abc".to_string(),
            object: Some("chat.completion".to_string()),
            created: Some(12345),
            model: "gpt-4".to_string(),
            choices: vec![OpenAiChoice {
                index: 0,
                message: OpenAiMessage {
                    role: "assistant".to_string(),
                    content: Some("World".to_string()),
                    tool_calls: vec![],
                    tool_call_id: None,
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: OpenAiUsage {
                prompt_tokens: 5,
                completion_tokens: 3,
                total_tokens: 8,
            },
        };

        let chat_id = ChatId::new();
        let core_resp = openai_resp.into_core(chat_id);
        assert_eq!(core_resp.id, "chatcmpl-abc");
        assert_eq!(core_resp.message.content, "World");
        assert_eq!(core_resp.message.role, MessageRole::Assistant);
        assert_eq!(core_resp.usage.total_tokens, 8);
        assert_eq!(core_resp.finish_reason, FinishReason::Stop);
    }

    #[test]
    fn test_embedding_request_serialization() {
        let req = OpenAiEmbeddingRequest {
            model: "text-embedding-3-small".to_string(),
            input: "Hello world".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("text-embedding-3-small"));
    }

    #[test]
    fn test_embedding_response_deserialization() {
        let json = r#"{
            "object": "list",
            "data": [{"object": "embedding", "index": 0, "embedding": [0.1, 0.2, 0.3]}],
            "model": "text-embedding-3-small",
            "usage": {"prompt_tokens": 2, "completion_tokens": 0, "total_tokens": 2}
        }"#;
        let resp: OpenAiEmbeddingResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 1);
        assert_eq!(resp.data[0].embedding, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn test_model_list_parsing() {
        let json = r#"{
            "object": "list",
            "data": [
                {"id": "gpt-4", "object": "model", "created": 123, "owned_by": "openai"},
                {"id": "gpt-3.5-turbo", "object": "model", "created": 456, "owned_by": "openai"}
            ]
        }"#;
        let resp: OpenAiModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 2);
        assert_eq!(resp.data[0].id, "gpt-4");
        assert_eq!(resp.data[1].id, "gpt-3.5-turbo");
    }

    #[test]
    fn test_stream_chunk_with_finish_reason() {
        let json = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion.chunk",
            "created": 1234567890,
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "delta": {"content": ""},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        }"#;
        let chunk: OpenAiStreamChunk = serde_json::from_str(json).unwrap();
        let token_chunk = chunk.into_token_chunk().unwrap();
        assert_eq!(token_chunk.finish_reason, Some(FinishReason::Stop));
        assert!(token_chunk.usage.is_some());
    }
}
