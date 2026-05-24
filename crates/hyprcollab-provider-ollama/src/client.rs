use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use reqwest::StatusCode;

use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::LlmProvider;
use hyprcollab_core::types::*;

/// Default Ollama API base URL.
pub const DEFAULT_BASE_URL: &str = "http://localhost:11434";

/// Client for the Ollama API (OpenAI-compatible endpoints).
#[derive(Debug, Clone)]
pub struct OllamaClient {
    pub base_url: String,
    pub http: reqwest::Client,
    pub model: String,
}

impl OllamaClient {
    pub fn new() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            http: reqwest::Client::new(),
            model: "llama3".to_string(),
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    fn resolve_model(&self, request_model: &str) -> String {
        if request_model.is_empty() {
            self.model.clone()
        } else {
            request_model.to_string()
        }
    }
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmProvider for OllamaClient {
    async fn chat_completion(&self, request: ChatRequest) -> Result<ChatResponse> {
        let model = self.resolve_model(&request.model);
        let url = format!("{}/v1/chat/completions", self.base_url);

        let body = serde_json::json!({
            "model": model,
            "messages": request.messages.iter().map(|m| serde_json::json!({
                "role": m.role.to_string(),
                "content": m.content,
            })).collect::<Vec<_>>(),
            "stream": false,
            "temperature": request.temperature,
            "max_tokens": request.max_tokens,
        });

        let resp = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| CoreError::Llm(format!("Ollama request failed: {e}")))?;

        let status = resp.status();
        if status != StatusCode::OK {
            let body = resp.text().await.unwrap_or_default();
            return Err(CoreError::Llm(format!("Ollama API error ({}): {}", status, body)));
        }

        let raw: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CoreError::Llm(format!("Failed to parse Ollama response: {e}")))?;

        let chat_id = request.messages.first().map(|m| m.chat_id).unwrap_or_default();

        let message = Message {
            id: MessageId::new(),
            chat_id,
            role: MessageRole::Assistant,
            content: raw["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            tool_calls: vec![],
            artifacts: vec![],
            timestamp: chrono::Utc::now(),
            metadata: serde_json::Value::Null,
        };

        let usage = TokenUsage {
            prompt_tokens: raw["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: raw["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: raw["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
        };

        let finish_reason_str = raw["choices"][0]["finish_reason"].as_str().unwrap_or("stop");
        let finish_reason = match finish_reason_str {
            "stop" => FinishReason::Stop,
            "tool_calls" => FinishReason::ToolCalls,
            "length" => FinishReason::Length,
            _ => FinishReason::Stop,
        };

        Ok(ChatResponse {
            id: raw["id"].as_str().unwrap_or("ollama").to_string(),
            message,
            usage,
            model: raw["model"].as_str().unwrap_or(&model).to_string(),
            finish_reason,
        })
    }

    fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<TokenChunk>> + Send>> {
        let model = self.resolve_model(&request.model);
        let url = format!("{}/v1/chat/completions", self.base_url);
        let http = self.http.clone();

        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .map(|m| serde_json::json!({"role": m.role.to_string(), "content": m.content}))
            .collect();

        let body = serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": true,
            "temperature": request.temperature,
        });

        let (tx, rx) = tokio::sync::mpsc::channel::<Result<TokenChunk>>(256);

        tokio::spawn(async move {
            use eventsource_stream::Eventsource;
            use futures::StreamExt;

            let resp = match http
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(Err(CoreError::Llm(format!("Ollama stream failed: {e}")))).await;
                    return;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let _ = tx.send(Err(CoreError::Llm(format!("Ollama stream error ({}): {}", status, body)))).await;
                return;
            }

            let mut stream = resp.bytes_stream().eventsource();
            while let Some(event) = stream.next().await {
                match event {
                    Ok(sse) => {
                        let data = sse.data.trim();
                        if data == "[DONE]" {
                            break;
                        }
                        if data.is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<serde_json::Value>(data) {
                            Ok(chunk) => {
                                let delta = chunk["choices"][0]["delta"]["content"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string();
                                let finish = chunk["choices"][0]["finish_reason"].as_str();
                                let finish_reason = match finish {
                                    Some("stop") => Some(FinishReason::Stop),
                                    Some("length") => Some(FinishReason::Length),
                                    Some(_) => Some(FinishReason::Stop),
                                    None => None,
                                };
                                if tx.send(Ok(TokenChunk { delta, finish_reason, usage: None, tool_calls: vec![] })).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(Err(CoreError::Llm(format!("Parse error: {e}")))).await;
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(CoreError::Llm(format!("SSE error: {e}")))).await;
                        break;
                    }
                }
            }
        });

        Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx))
    }

    async fn embeddings(&self, input: &str) -> Result<Vec<f32>> {
        let url = format!("{}/api/embeddings", self.base_url);
        let body = serde_json::json!({
            "model": self.model,
            "prompt": input,
        });

        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| CoreError::Llm(format!("Ollama embeddings failed: {e}")))?;

        if !resp.status().is_success() {
            return Err(CoreError::Llm(format!("Ollama embeddings error ({})", resp.status())));
        }

        let raw: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CoreError::Llm(format!("Failed to parse embeddings: {e}")))?;

        raw["embedding"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
            .ok_or_else(|| CoreError::Llm("No embedding in response".to_string()))
    }

    async fn models(&self) -> Result<Vec<ModelInfo>> {
        let url = format!("{}/api/tags", self.base_url);

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| CoreError::Llm(format!("Ollama models request failed: {e}")))?;

        if !resp.status().is_success() {
            return Err(CoreError::Llm(format!("Ollama models error ({})", resp.status())));
        }

        let raw: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CoreError::Llm(format!("Failed to parse models: {e}")))?;

        let models = raw["models"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|m| {
                        let id = m["name"].as_str().unwrap_or("unknown").to_string();
                        ModelInfo {
                            id: id.clone(),
                            name: id,
                            provider: "ollama".to_string(),
                            context_length: 8192,
                            supports_streaming: true,
                            supports_tools: false,
                            supports_vision: false,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(models)
    }

    fn name(&self) -> &str {
        "ollama"
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn supports_tools(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_construction_default() {
        let client = OllamaClient::new();
        assert_eq!(client.base_url, DEFAULT_BASE_URL);
        assert_eq!(client.model, "llama3");
        assert_eq!(client.name(), "ollama");
        assert!(client.supports_streaming());
        assert!(!client.supports_tools());
    }

    #[test]
    fn test_client_construction_custom() {
        let client = OllamaClient::new()
            .with_base_url("http://192.168.1.5:11434")
            .with_model("codellama");
        assert_eq!(client.base_url, "http://192.168.1.5:11434");
        assert_eq!(client.model, "codellama");
    }

    #[test]
    fn test_default_impl() {
        let client = OllamaClient::default();
        assert_eq!(client.base_url, DEFAULT_BASE_URL);
    }

    #[test]
    fn test_resolve_model_fallback() {
        let client = OllamaClient::new().with_model("mistral");
        assert_eq!(client.resolve_model(""), "mistral");
        assert_eq!(client.resolve_model("llama3"), "llama3");
    }

    #[tokio::test]
    async fn test_error_handling() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(500)
            .with_body(r#"{"error":"model not found"}"#)
            .create_async()
            .await;

        let client = OllamaClient::new().with_base_url(&server.url());
        let request = ChatRequest {
            model: "nonexistent".to_string(),
            messages: vec![Message {
                id: MessageId::new(),
                chat_id: ChatId::new(),
                role: MessageRole::User,
                content: "test".to_string(),
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
    }

    #[tokio::test]
    async fn test_models_endpoint() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("GET", "/api/tags")
            .with_status(200)
            .with_body(r#"{"models":[{"name":"llama3:latest","size":4661224676},{"name":"mistral:latest","size":4132121376}]}"#)
            .create_async()
            .await;

        let client = OllamaClient::new().with_base_url(&server.url());
        let models = client.models().await.unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "llama3:latest");
        assert_eq!(models[1].provider, "ollama");
    }

    #[tokio::test]
    async fn test_chat_completion_success() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_body(r#"{"id":"ollama-123","model":"llama3","choices":[{"index":0,"message":{"role":"assistant","content":"Hello from Ollama!"},"finish_reason":"stop"}],"usage":{"prompt_tokens":5,"completion_tokens":4,"total_tokens":9}}"#)
            .create_async()
            .await;

        let client = OllamaClient::new().with_base_url(&server.url());
        let request = ChatRequest {
            model: "llama3".to_string(),
            messages: vec![Message {
                id: MessageId::new(),
                chat_id: ChatId::new(),
                role: MessageRole::User,
                content: "Hi".to_string(),
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

        let response = client.chat_completion(request).await.unwrap();
        assert_eq!(response.message.content, "Hello from Ollama!");
        assert_eq!(response.usage.total_tokens, 9);
        assert_eq!(response.finish_reason, FinishReason::Stop);
    }

    #[tokio::test]
    async fn test_embeddings_endpoint() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/api/embeddings")
            .with_status(200)
            .with_body(r#"{"model":"llama3","embedding":[0.1,0.2,0.3]}"#)
            .create_async()
            .await;

        let client = OllamaClient::new().with_base_url(&server.url());
        let emb = client.embeddings("hello").await.unwrap();
        assert_eq!(emb.len(), 3);
        assert!((emb[0] - 0.1).abs() < f32::EPSILON);
    }
}
