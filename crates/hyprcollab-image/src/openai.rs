use async_trait::async_trait;
use base64::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::{
    provider::ImageProvider,
    types::{
        GeneratedImage, ImageError, ImageProviderType, ImageQuality, ImageRequest, ImageResponse,
        ImageSize, ImageStyle, Result,
    },
};

const DEFAULT_BASE_URL: &str = "https://api.openai.com";
const DEFAULT_MODEL: &str = "dall-e-3";

static SUPPORTED_SIZES: &[ImageSize] = &[
    ImageSize::Square1024,
    ImageSize::Landscape1792x1024,
    ImageSize::Portrait1024x1792,
];

pub struct OpenAiProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl OpenAiProvider {
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
struct OpenAiRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    n: u32,
    size: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    quality: Option<&'a str>,
    response_format: &'a str,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    created: i64,
    data: Vec<OpenAiImage>,
}

#[derive(Deserialize)]
struct OpenAiImage {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    b64_json: Option<String>,
    #[serde(default)]
    revised_prompt: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiErrorBody {
    error: OpenAiErrorDetail,
}

#[derive(Deserialize)]
struct OpenAiErrorDetail {
    message: String,
}

#[async_trait]
impl ImageProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn provider_type(&self) -> ImageProviderType {
        ImageProviderType::OpenAi
    }

    fn supported_sizes(&self) -> &[ImageSize] {
        SUPPORTED_SIZES
    }

    fn default_model(&self) -> &str {
        DEFAULT_MODEL
    }

    async fn generate(&self, request: ImageRequest) -> Result<ImageResponse> {
        let model = request.model.as_deref().unwrap_or(DEFAULT_MODEL);
        let size_str = request.size.to_openai_str();
        let style_str = request.style.as_ref().map(ImageStyle::to_openai_str);
        let quality_str = request.quality.as_ref().map(ImageQuality::to_openai_str);
        let (w, h) = request.size.dimensions();

        let body = OpenAiRequest {
            model,
            prompt: &request.prompt,
            n: request.n,
            size: size_str,
            style: style_str,
            quality: quality_str,
            response_format: "url",
        };

        debug!("openai generate: model={model}");
        let response = self
            .client
            .post(format!("{}/v1/images/generations", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            let msg = serde_json::from_str::<OpenAiErrorBody>(&text)
                .map(|e| e.error.message)
                .unwrap_or(text);
            return Err(ImageError::ApiError { status: status.as_u16(), message: msg });
        }

        let api_resp: OpenAiResponse = response.json().await?;

        let images = api_resp
            .data
            .into_iter()
            .map(|img| GeneratedImage {
                url: img.url,
                data: img
                    .b64_json
                    .and_then(|b| BASE64_STANDARD.decode(b.as_bytes()).ok()),
                revised_prompt: img.revised_prompt,
                width: w,
                height: h,
                seed: None,
            })
            .collect();

        use chrono::TimeZone as _;
        let created = chrono::Utc
            .timestamp_opt(api_resp.created, 0)
            .single()
            .unwrap_or_else(chrono::Utc::now);

        Ok(ImageResponse {
            images,
            model: model.to_string(),
            provider: "openai".to_string(),
            created,
        })
    }
}
