use anyhow::anyhow;
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct Embedder {
    client: Client,
    base_url: String,
    api_key: String,
    pub model: String,
}

impl Embedder {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    pub async fn embed_one(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let mut batch = self.embed_batch(&[text]).await?;
        batch.pop().ok_or_else(|| anyhow!("empty embedding response"))
    }

    pub async fn embed_batch(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        #[derive(Serialize)]
        struct Req<'a> {
            model: &'a str,
            input: Vec<&'a str>,
        }

        #[derive(Deserialize)]
        struct Resp {
            data: Vec<EmbedData>,
        }

        #[derive(Deserialize)]
        struct EmbedData {
            embedding: Vec<f32>,
            index: usize,
        }

        let req = Req { model: &self.model, input: texts.to_vec() };

        let resp = self
            .client
            .post(format!("{}/embeddings", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&req)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("embeddings API {}: {}", status, body));
        }

        let mut parsed: Resp = resp.json().await?;
        // Sort by index to maintain original order
        parsed.data.sort_by_key(|d| d.index);
        Ok(parsed.data.into_iter().map(|d| d.embedding).collect())
    }
}

/// Cosine similarity between two equal-length vectors. Returns 0.0 on zero vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}
