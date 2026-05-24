//! Embeddings are not supported by the Anthropic API.

use hyprcollab_core::errors::{CoreError, Result};

/// Anthropic does not provide an embeddings API.
pub async fn get_embeddings(_http: &reqwest::Client, _input: &str) -> Result<Vec<f32>> {
    Err(CoreError::Llm(
        "Anthropic does not provide an embeddings API".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_embeddings_not_supported() {
        let http = reqwest::Client::new();
        let result = get_embeddings(&http, "test").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Anthropic does not provide an embeddings API"));
    }
}
