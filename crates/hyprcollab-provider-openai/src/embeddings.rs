//! Embeddings endpoint for the OpenAI API.

use reqwest::StatusCode;

use hyprcollab_core::errors::{CoreError, Result};

use crate::types::{OpenAiEmbeddingRequest, OpenAiEmbeddingResponse};

/// Default embedding model.
const DEFAULT_EMBEDDING_MODEL: &str = "text-embedding-3-small";

/// Get embeddings for the given input text.
pub async fn get_embeddings(
    http: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    input: &str,
) -> Result<Vec<f32>> {
    let url = format!("{}/embeddings", base_url);

    let body = OpenAiEmbeddingRequest {
        model: DEFAULT_EMBEDDING_MODEL.to_string(),
        input: input.to_string(),
    };

    let resp = http
        .post(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| CoreError::Llm(format!("Embedding request failed: {e}")))?;

    let status = resp.status();
    if status != StatusCode::OK {
        let body = resp.text().await.unwrap_or_default();
        return Err(CoreError::Llm(format!(
            "OpenAI embedding error ({}): {}",
            status, body
        )));
    }

    let embedding_resp: OpenAiEmbeddingResponse = resp
        .json()
        .await
        .map_err(|e| CoreError::Llm(format!("Failed to parse embedding response: {e}")))?;

    embedding_resp
        .data
        .into_iter()
        .next()
        .map(|d| d.embedding)
        .ok_or_else(|| CoreError::Llm("No embedding data returned".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_embeddings_success() {
        let mut server = mockito::Server::new_async().await;

        let response_body = r#"{
            "object": "list",
            "data": [{"object": "embedding", "index": 0, "embedding": [0.1, -0.2, 0.3]}],
            "model": "text-embedding-3-small",
            "usage": {"prompt_tokens": 2, "completion_tokens": 0, "total_tokens": 2}
        }"#;

        let mock = server
            .mock("POST", "/embeddings")
            .with_status(200)
            .with_body(response_body)
            .create_async()
            .await;

        let http = reqwest::Client::new();
        let result = get_embeddings(&http, &server.url(), "sk-test", "Hello world").await;

        assert!(result.is_ok());
        let embedding = result.unwrap();
        assert_eq!(embedding, vec![0.1, -0.2, 0.3]);

        mock.assert_async().await;
    }
}
