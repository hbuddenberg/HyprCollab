use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::{
    provider::ImageProvider,
    types::{
        GeneratedImage, ImageError, ImageProviderType, ImageRequest, ImageResponse, ImageSize,
        Result,
    },
};

const DEFAULT_BASE_URL: &str = "https://fal.run";
const DEFAULT_MODEL: &str = "fal-ai/flux/schnell";

static SUPPORTED_SIZES: &[ImageSize] = &[
    ImageSize::Square1024,
    ImageSize::Landscape1792x1024,
    ImageSize::Portrait1024x1792,
];

pub struct FalProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl FalProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_base_url(api_key, DEFAULT_BASE_URL)
    }

    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }
}

#[derive(Serialize)]
struct FalRequest {
    prompt: String,
    image_size: serde_json::Value,
    num_images: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<u64>,
}

#[derive(Deserialize)]
struct FalResponse {
    images: Vec<FalImage>,
    #[serde(default)]
    seed: Option<u64>,
}

#[derive(Deserialize)]
struct FalImage {
    url: String,
    width: u32,
    height: u32,
}

#[derive(Deserialize)]
struct FalError {
    #[serde(default)]
    detail: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[async_trait]
impl ImageProvider for FalProvider {
    fn name(&self) -> &str {
        "fal"
    }

    fn provider_type(&self) -> ImageProviderType {
        ImageProviderType::Fal
    }

    fn supported_sizes(&self) -> &[ImageSize] {
        SUPPORTED_SIZES
    }

    fn default_model(&self) -> &str {
        DEFAULT_MODEL
    }

    async fn generate(&self, request: ImageRequest) -> Result<ImageResponse> {
        let model = request.model.as_deref().unwrap_or(DEFAULT_MODEL);
        let endpoint = format!("{}/{}", self.base_url, model);

        let body = FalRequest {
            prompt: request.prompt.clone(),
            image_size: request.size.to_fal_size(),
            num_images: request.n,
            seed: request.seed,
        };

        debug!("fal generate: model={model}");
        let response = self
            .client
            .post(&endpoint)
            .header("Authorization", format!("Key {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            let msg = serde_json::from_str::<FalError>(&text)
                .map(|e| e.detail.or(e.error).unwrap_or_else(|| text.clone()))
                .unwrap_or(text);
            return Err(ImageError::ApiError { status: status.as_u16(), message: msg });
        }

        let api_resp: FalResponse = response.json().await?;
        let seed = api_resp.seed;

        let images = api_resp
            .images
            .into_iter()
            .map(|img| GeneratedImage {
                url: Some(img.url),
                data: None,
                revised_prompt: None,
                width: img.width,
                height: img.height,
                seed,
            })
            .collect();

        Ok(ImageResponse {
            images,
            model: model.to_string(),
            provider: "fal".to_string(),
            created: chrono::Utc::now(),
        })
    }
}
