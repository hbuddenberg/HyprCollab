use async_trait::async_trait;
use base64::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::{
    provider::ImageProvider,
    types::{
        GeneratedImage, ImageError, ImageProviderType, ImageRequest, ImageResponse, ImageSize,
        Result,
    },
};

const DEFAULT_BASE_URL: &str = "http://localhost:7860";
const DEFAULT_MODEL: &str = "a1111";

static SUPPORTED_SIZES: &[ImageSize] = &[
    ImageSize::Square1024,
    ImageSize::Landscape1792x1024,
    ImageSize::Portrait1024x1792,
];

pub struct A1111Provider {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl A1111Provider {
    pub fn new() -> Self {
        Self::with_base_url(DEFAULT_BASE_URL)
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: None,
        }
    }

    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }
}

impl Default for A1111Provider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize)]
struct A1111Request<'a> {
    prompt: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    negative_prompt: Option<&'a str>,
    steps: u32,
    cfg_scale: f32,
    width: u32,
    height: u32,
    n_iter: u32,
    seed: i64,
}

#[derive(Deserialize)]
struct A1111Response {
    images: Vec<String>,
    #[serde(default)]
    info: Option<String>,
}

#[derive(Deserialize)]
struct A1111Info {
    #[serde(default)]
    all_seeds: Vec<i64>,
}

#[async_trait]
impl ImageProvider for A1111Provider {
    fn name(&self) -> &str {
        "a1111"
    }

    fn provider_type(&self) -> ImageProviderType {
        ImageProviderType::Automatic1111
    }

    fn supported_sizes(&self) -> &[ImageSize] {
        SUPPORTED_SIZES
    }

    fn default_model(&self) -> &str {
        DEFAULT_MODEL
    }

    async fn generate(&self, request: ImageRequest) -> Result<ImageResponse> {
        let (w, h) = request.size.dimensions();

        let body = A1111Request {
            prompt: &request.prompt,
            negative_prompt: request.negative_prompt.as_deref(),
            steps: request.steps.unwrap_or(20),
            cfg_scale: request.cfg_scale.unwrap_or(7.0),
            width: w,
            height: h,
            n_iter: request.n,
            seed: request.seed.map(|s| s as i64).unwrap_or(-1),
        };

        debug!("a1111 txt2img w={w} h={h}");
        let mut builder = self.client.post(format!("{}/sdapi/v1/txt2img", self.base_url));
        if let Some(key) = &self.api_key {
            builder = builder.header("Authorization", format!("Bearer {key}"));
        }
        let response = builder.json(&body).send().await?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(ImageError::ApiError { status: status.as_u16(), message: text });
        }

        let api_resp: A1111Response = response.json().await?;

        let seeds: Vec<u64> = api_resp
            .info
            .as_deref()
            .and_then(|s| serde_json::from_str::<A1111Info>(s).ok())
            .map(|info| {
                info.all_seeds
                    .into_iter()
                    .filter(|&s| s >= 0)
                    .map(|s| s as u64)
                    .collect()
            })
            .unwrap_or_default();

        let images = api_resp
            .images
            .into_iter()
            .enumerate()
            .map(|(i, b64)| GeneratedImage {
                url: None,
                data: BASE64_STANDARD.decode(b64.as_bytes()).ok(),
                revised_prompt: None,
                width: w,
                height: h,
                seed: seeds.get(i).copied(),
            })
            .collect();

        Ok(ImageResponse {
            images,
            model: DEFAULT_MODEL.to_string(),
            provider: "a1111".to_string(),
            created: chrono::Utc::now(),
        })
    }
}
