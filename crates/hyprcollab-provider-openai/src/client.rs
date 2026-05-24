//! OpenAI client implementing the `LlmProvider` trait.

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use reqwest::StatusCode;

use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::LlmProvider;
use hyprcollab_core::types::*;

use crate::embeddings;
use crate::streaming;
use crate::types::*;

/// Default OpenAI API base URL.
pub const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

/// Default model for chat completions.
pub const DEFAULT_MODEL: &str = "gpt-4o";

/// Client for the OpenAI (and compatible) API.
#[derive(Debug, Clone)]
pub struct OpenAiClient {
    /// API key for authentication.
    pub api_key: String,
    /// Base URL (default: `https://api.openai.com/v1`).
    pub base_url: String,
    /// HTTP client.
    pub http: reqwest::Client,
    /// Default model to use when not specified in the request.
    pub model: String,
}

impl OpenAiClient {
    /// Create a new client with the given API key and default settings.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            http: reqwest::Client::new(),
            model: DEFAULT_MODEL.to_string(),
        }
    }

    /// Set a custom base URL (e.g. for DeepSeek or other compatible APIs).
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Set the default model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Build the authorization header value.
    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_key)
    }

    /// Resolve the model: use the request's model if non-empty, else fall back to the client default.
    fn resolve_model(&self, request_model: &str) -> String {
        if request_model.is_empty() {
            self.model.clone()
        } else {
            request_model.to_string()
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAiClient {
    async fn chat_completion(&self, request: ChatRequest) -> Result<ChatResponse> {
        let model = self.resolve_model(&request.model);
        let mut openai_req = OpenAiRequest::from_core(&request);
        openai_req.model = model;
        openai_req.stream = None; // Ensure non-streaming

        let url = format!("{}/chat/completions", self.base_url);

        let resp = self
            .http
            .post(&url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/json")
            .json(&openai_req)
            .send()
            .await
            .map_err(|e| CoreError::Llm(format!("HTTP request failed: {e}")))?;

        let status = resp.status();
        if status != StatusCode::OK {
            let body = resp.text().await.unwrap_or_default();
            return Err(CoreError::Llm(format!(
                "OpenAI API error ({}): {}",
                status, body
            )));
        }

        let openai_resp: OpenAiResponse = resp
            .json()
            .await
            .map_err(|e| CoreError::Llm(format!("Failed to parse response: {e}")))?;

        let chat_id = request
            .messages
            .first()
            .map(|m| m.chat_id)
            .unwrap_or_default();

        Ok(openai_resp.into_core(chat_id))
    }

    fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<TokenChunk>> + Send>> {
        let model = self.resolve_model(&request.model);
        let mut openai_req = OpenAiRequest::from_core(&request);
        openai_req.model = model;
        openai_req.stream = Some(true);

        let url = format!("{}/chat/completions", self.base_url);
        let auth = self.auth_header();
        let http = self.http.clone();

        Box::pin(streaming::create_stream(http, url, auth, openai_req))
    }

    async fn embeddings(&self, input: &str) -> Result<Vec<f32>> {
        embeddings::get_embeddings(&self.http, &self.base_url, &self.api_key, input).await
    }

    async fn models(&self) -> Result<Vec<ModelInfo>> {
        let url = format!("{}/models", self.base_url);

        let resp = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| CoreError::Llm(format!("HTTP request failed: {e}")))?;

        let status = resp.status();
        if status != StatusCode::OK {
            let body = resp.text().await.unwrap_or_default();
            return Err(CoreError::Llm(format!(
                "OpenAI API error ({}): {}",
                status, body
            )));
        }

        let models_resp: OpenAiModelsResponse = resp
            .json()
            .await
            .map_err(|e| CoreError::Llm(format!("Failed to parse models response: {e}")))?;

        let models = models_resp
            .data
            .into_iter()
            .map(|m| {
                let id = m.id;
                let supports_tools = id.contains("gpt-4") || id.contains("gpt-3.5-turbo");
                let supports_vision = id.contains("vision") || id.contains("gpt-4o");
                ModelInfo {
                    id: id.clone(),
                    name: id,
                    provider: "openai".to_string(),
                    context_length: 128_000,
                    supports_streaming: true,
                    supports_tools,
                    supports_vision,
                }
            })
            .collect();

        Ok(models)
    }

    fn name(&self) -> &str {
        "openai"
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
    fn test_client_construction_default() {
        let client = OpenAiClient::new("sk-test123");
        assert_eq!(client.api_key, "sk-test123");
        assert_eq!(client.base_url, DEFAULT_BASE_URL);
        assert_eq!(client.model, DEFAULT_MODEL);
        assert_eq!(client.name(), "openai");
        assert!(client.supports_streaming());
        assert!(client.supports_tools());
    }

    #[test]
    fn test_client_construction_custom_base_url() {
        let client = OpenAiClient::new("sk-test")
            .with_base_url("https://api.deepseek.com/v1")
            .with_model("deepseek-chat");

        assert_eq!(client.base_url, "https://api.deepseek.com/v1");
        assert_eq!(client.model, "deepseek-chat");
    }

    #[tokio::test]
    async fn test_error_handling_invalid_api_key() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("POST", "/chat/completions")
            .with_status(401)
            .with_body(r#"{"error":{"message":"Incorrect API key provided","type":"invalid_request_error"}}"#)
            .create_async()
            .await;

        let client = OpenAiClient::new("sk-invalid")
            .with_base_url(&server.url());

        let request = ChatRequest {
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
}
