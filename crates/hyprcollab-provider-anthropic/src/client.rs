//! Anthropic client implementing the `LlmProvider` trait.

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use reqwest::StatusCode;

use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::LlmProvider;
use hyprcollab_core::types::*;

use crate::streaming;
use crate::types::*;

/// Default Anthropic API base URL.
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com/v1";

/// Default model for chat completions.
pub const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";

/// Default Anthropic API version.
pub const DEFAULT_API_VERSION: &str = "2023-06-01";

/// Client for the Anthropic Messages API.
#[derive(Debug, Clone)]
pub struct AnthropicClient {
    /// API key for authentication.
    pub api_key: String,
    /// Base URL (default: `https://api.anthropic.com/v1`).
    pub base_url: String,
    /// HTTP client.
    pub http: reqwest::Client,
    /// Default model to use when not specified in the request.
    pub model: String,
    /// Anthropic API version header.
    pub api_version: String,
}

impl AnthropicClient {
    /// Create a new client with the given API key and default settings.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            http: reqwest::Client::new(),
            model: DEFAULT_MODEL.to_string(),
            api_version: DEFAULT_API_VERSION.to_string(),
        }
    }

    /// Set a custom base URL.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Set the default model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Resolve the model: use the request's model if non-empty, else fall back to the client default.
    fn resolve_model(&self, request_model: &str) -> String {
        if request_model.is_empty() {
            self.model.clone()
        } else {
            request_model.to_string()
        }
    }

    /// Hardcoded list of known Anthropic models.
    fn hardcoded_models() -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "claude-sonnet-4-20250514".to_string(),
                name: "Claude Sonnet 4".to_string(),
                provider: "anthropic".to_string(),
                context_length: 200_000,
                supports_streaming: true,
                supports_tools: true,
                supports_vision: true,
            },
            ModelInfo {
                id: "claude-opus-4-20250514".to_string(),
                name: "Claude Opus 4".to_string(),
                provider: "anthropic".to_string(),
                context_length: 200_000,
                supports_streaming: true,
                supports_tools: true,
                supports_vision: true,
            },
            ModelInfo {
                id: "claude-3-7-sonnet-20250219".to_string(),
                name: "Claude 3.7 Sonnet".to_string(),
                provider: "anthropic".to_string(),
                context_length: 200_000,
                supports_streaming: true,
                supports_tools: true,
                supports_vision: true,
            },
            ModelInfo {
                id: "claude-3-5-haiku-20241022".to_string(),
                name: "Claude 3.5 Haiku".to_string(),
                provider: "anthropic".to_string(),
                context_length: 200_000,
                supports_streaming: true,
                supports_tools: true,
                supports_vision: true,
            },
        ]
    }
}

#[async_trait]
impl LlmProvider for AnthropicClient {
    async fn chat_completion(&self, request: ChatRequest) -> Result<ChatResponse> {
        let model = self.resolve_model(&request.model);
        let mut anthropic_req = AnthropicRequest::from_core(&request);
        anthropic_req.model = model;
        anthropic_req.stream = None; // Ensure non-streaming

        let url = format!("{}/messages", self.base_url);

        let resp = self
            .http
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", &self.api_version)
            .header("Content-Type", "application/json")
            .json(&anthropic_req)
            .send()
            .await
            .map_err(|e| CoreError::Llm(format!("Anthropic request failed: {e}")))?;

        let status = resp.status();
        if status != StatusCode::OK {
            let body = resp.text().await.unwrap_or_default();
            return Err(CoreError::Llm(format!(
                "Anthropic API error ({}): {}",
                status, body
            )));
        }

        let anthropic_resp: AnthropicResponse = resp
            .json()
            .await
            .map_err(|e| CoreError::Llm(format!("Failed to parse Anthropic response: {e}")))?;

        let chat_id = request
            .messages
            .first()
            .map(|m| m.chat_id)
            .unwrap_or_default();

        Ok(anthropic_resp.into_core(chat_id))
    }

    fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<TokenChunk>> + Send>> {
        let model = self.resolve_model(&request.model);
        let mut anthropic_req = AnthropicRequest::from_core(&request);
        anthropic_req.model = model;
        anthropic_req.stream = Some(true);

        let url = format!("{}/messages", self.base_url);
        let api_key = self.api_key.clone();
        let api_version = self.api_version.clone();
        let http = self.http.clone();

        Box::pin(streaming::create_stream(
            http,
            url,
            api_key,
            api_version,
            anthropic_req,
        ))
    }

    async fn embeddings(&self, _input: &str) -> Result<Vec<f32>> {
        Err(CoreError::Llm(
            "Anthropic does not provide an embeddings API".to_string(),
        ))
    }

    async fn models(&self) -> Result<Vec<ModelInfo>> {
        Ok(Self::hardcoded_models())
    }

    fn name(&self) -> &str {
        "anthropic"
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn supports_tools(&self) -> bool {
        true
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_construction_defaults() {
        let client = AnthropicClient::new("sk-ant-test123");
        assert_eq!(client.api_key, "sk-ant-test123");
        assert_eq!(client.base_url, DEFAULT_BASE_URL);
        assert_eq!(client.model, DEFAULT_MODEL);
        assert_eq!(client.api_version, DEFAULT_API_VERSION);
        assert_eq!(client.name(), "anthropic");
        assert!(client.supports_streaming());
        assert!(client.supports_tools());
    }

    #[test]
    fn test_client_construction_custom_base_url() {
        let client = AnthropicClient::new("sk-ant-test")
            .with_base_url("https://custom-proxy.example.com/v1")
            .with_model("claude-opus-4-20250514");

        assert_eq!(client.base_url, "https://custom-proxy.example.com/v1");
        assert_eq!(client.model, "claude-opus-4-20250514");
    }

    #[tokio::test]
    async fn test_error_handling_401() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("POST", "/messages")
            .match_header("x-api-key", "sk-ant-invalid")
            .match_header("anthropic-version", "2023-06-01")
            .with_status(401)
            .with_body(r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#)
            .create_async()
            .await;

        let client = AnthropicClient::new("sk-ant-invalid")
            .with_base_url(&server.url());

        let request = ChatRequest {
            model: "claude-sonnet-4-20250514".to_string(),
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
            tools: vec![],
            temperature: None,
            max_tokens: None,
            stream: false,
            persona: None,
            agent_role: None,
        };

        let result = client.chat_completion(request).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("401"));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_hardcoded_model_list() {
        let client = AnthropicClient::new("sk-ant-test");
        let models = client.models().await.unwrap();
        assert!(!models.is_empty());
        assert_eq!(models.len(), 4);

        // Check first model
        let first = &models[0];
        assert_eq!(first.id, "claude-sonnet-4-20250514");
        assert_eq!(first.provider, "anthropic");
        assert!(first.supports_streaming);
        assert!(first.supports_tools);

        // All models should be from anthropic
        for m in &models {
            assert_eq!(m.provider, "anthropic");
        }
    }

    #[tokio::test]
    async fn test_embeddings_returns_error() {
        let client = AnthropicClient::new("sk-ant-test");
        let result = client.embeddings("test input").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Anthropic does not provide an embeddings API"));
    }

    #[tokio::test]
    async fn test_chat_completion_sends_correct_headers() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("POST", "/messages")
            .match_header("x-api-key", "sk-ant-test123")
            .match_header("anthropic-version", "2023-06-01")
            .match_header("Content-Type", "application/json")
            .with_status(200)
            .with_body(r#"{
                "id": "msg_test",
                "type": "message",
                "role": "assistant",
                "content": [{"type": "text", "text": "Hi!"}],
                "model": "claude-sonnet-4-20250514",
                "stop_reason": "end_turn",
                "usage": {"input_tokens": 8, "output_tokens": 2}
            }"#)
            .create_async()
            .await;

        let client = AnthropicClient::new("sk-ant-test123")
            .with_base_url(&server.url());

        let request = ChatRequest {
            model: "claude-sonnet-4-20250514".to_string(),
            messages: vec![Message {
                id: MessageId::new(),
                chat_id: ChatId::new(),
                role: MessageRole::User,
                content: "Hey".to_string(),
                tool_calls: vec![],
                artifacts: vec![],
                timestamp: chrono::Utc::now(),
                metadata: serde_json::Value::Null,
            }],
            tools: vec![],
            temperature: None,
            max_tokens: None,
            stream: false,
            persona: None,
            agent_role: None,
        };

        let result = client.chat_completion(request).await;
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.id, "msg_test");
        assert_eq!(resp.message.content, "Hi!");
        assert_eq!(resp.finish_reason, FinishReason::Stop);

        mock.assert_async().await;
    }
}
