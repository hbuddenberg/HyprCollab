use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::Result;
use crate::RagError;

/// Embedding provider abstraction.
#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    fn dimension(&self) -> usize;
}

/// OpenAI-compatible embeddings client.
pub struct OpenAiEmbedder {
    http: reqwest::Client,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    dim: usize,
}

impl OpenAiEmbedder {
    pub fn new(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: api_key.into(),
            base_url: base_url.into(),
            model: "text-embedding-3-small".to_string(),
            dim: 1536,
        }
    }

    pub fn with_model(mut self, model: impl Into<String>, dim: usize) -> Self {
        self.model = model.into();
        self.dim = dim;
        self
    }
}

#[derive(Serialize)]
struct EmbedRequest<'a> {
    model: &'a str,
    input: &'a str,
}

#[derive(Deserialize)]
struct EmbedResponse {
    data: Vec<EmbedData>,
}

#[derive(Deserialize)]
struct EmbedData {
    embedding: Vec<f32>,
}

#[async_trait]
impl Embedder for OpenAiEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/embeddings", self.base_url.trim_end_matches('/'));
        let body = EmbedRequest { model: &self.model, input: text };

        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let msg = resp.text().await.unwrap_or_default();
            return Err(RagError::Embed(format!("HTTP {status}: {msg}")));
        }

        let parsed: EmbedResponse = resp.json().await?;
        parsed
            .data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .ok_or_else(|| RagError::Embed("empty data array in response".into()))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut out = Vec::with_capacity(texts.len());
        for t in texts {
            out.push(self.embed(t).await?);
        }
        Ok(out)
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

// ── test helpers ──────────────────────────────────────────────────────────────

#[cfg(test)]
pub struct MockEmbedder {
    pub dim: usize,
    pub fail: bool,
}

#[cfg(test)]
#[async_trait]
impl Embedder for MockEmbedder {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        if self.fail {
            return Err(RagError::Embed("mock failure".into()));
        }
        Ok(vec![0.1f32; self.dim])
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        texts.iter().map(|t| self.embed_sync(t)).collect()
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

#[cfg(test)]
impl MockEmbedder {
    fn embed_sync(&self, _text: &str) -> Result<Vec<f32>> {
        if self.fail {
            return Err(RagError::Embed("mock failure".into()));
        }
        Ok(vec![0.1f32; self.dim])
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_embed_response(embedding: Vec<f32>) -> String {
        let vals: Vec<String> = embedding.iter().map(|v| v.to_string()).collect();
        format!(
            r#"{{"object":"list","data":[{{"object":"embedding","index":0,"embedding":[{}]}}],"model":"text-embedding-3-small","usage":{{"prompt_tokens":1,"total_tokens":1}}}}"#,
            vals.join(",")
        )
    }

    #[tokio::test]
    async fn embedder_embed_success() {
        let mut server = mockito::Server::new_async().await;
        let body = make_embed_response(vec![0.1, 0.2, 0.3]);
        let _mock = server
            .mock("POST", "/embeddings")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&body)
            .create_async()
            .await;

        let emb = OpenAiEmbedder::new("sk-test", server.url());
        let result = emb.embed("hello").await.unwrap();
        assert_eq!(result, vec![0.1, 0.2, 0.3]);
    }

    #[tokio::test]
    async fn embedder_embed_http_401_returns_err() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/embeddings")
            .with_status(401)
            .with_body("Unauthorized")
            .create_async()
            .await;

        let emb = OpenAiEmbedder::new("bad-key", server.url());
        assert!(emb.embed("hello").await.is_err());
    }

    #[tokio::test]
    async fn embedder_embed_http_500_returns_err() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/embeddings")
            .with_status(500)
            .with_body("Server Error")
            .create_async()
            .await;

        let emb = OpenAiEmbedder::new("sk-test", server.url());
        assert!(emb.embed("hello").await.is_err());
    }

    #[tokio::test]
    async fn embedder_embed_batch_success() {
        let mut server = mockito::Server::new_async().await;
        let body = make_embed_response(vec![0.5, 0.6]);
        let _mock = server
            .mock("POST", "/embeddings")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&body)
            .expect(2)
            .create_async()
            .await;

        let emb = OpenAiEmbedder::new("sk-test", server.url());
        let texts = vec!["hello".to_string(), "world".to_string()];
        let result = emb.embed_batch(&texts).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], vec![0.5, 0.6]);
    }

    #[tokio::test]
    async fn embedder_embed_batch_empty_returns_empty() {
        let server = mockito::Server::new_async().await;
        let emb = OpenAiEmbedder::new("sk-test", server.url());
        let result = emb.embed_batch(&[]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn embedder_dimension_default_1536() {
        let server = mockito::Server::new_async().await;
        let emb = OpenAiEmbedder::new("sk-test", server.url());
        assert_eq!(emb.dimension(), 1536);
    }

    #[tokio::test]
    async fn embedder_custom_base_url_is_called() {
        let mut server = mockito::Server::new_async().await;
        let body = make_embed_response(vec![1.0]);
        let mock = server
            .mock("POST", "/embeddings")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&body)
            .create_async()
            .await;

        let emb = OpenAiEmbedder::new("sk-test", server.url());
        emb.embed("test").await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn embedder_with_custom_model_and_dimension() {
        let server = mockito::Server::new_async().await;
        let emb = OpenAiEmbedder::new("sk-test", server.url())
            .with_model("text-embedding-ada-002", 1536);
        assert_eq!(emb.model, "text-embedding-ada-002");
        assert_eq!(emb.dimension(), 1536);
    }

    #[tokio::test]
    async fn mock_embedder_returns_deterministic_vector() {
        let emb = MockEmbedder { dim: 4, fail: false };
        let v = emb.embed("anything").await.unwrap();
        assert_eq!(v, vec![0.1, 0.1, 0.1, 0.1]);
    }
}
