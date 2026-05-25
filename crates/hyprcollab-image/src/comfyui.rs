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

const DEFAULT_BASE_URL: &str = "http://localhost:8188";
const DEFAULT_MODEL: &str = "comfyui";

static SUPPORTED_SIZES: &[ImageSize] = &[
    ImageSize::Square1024,
    ImageSize::Landscape1792x1024,
    ImageSize::Portrait1024x1792,
];

pub struct ComfyUiProvider {
    client: reqwest::Client,
    base_url: String,
}

impl ComfyUiProvider {
    pub fn new() -> Self {
        Self::with_base_url(DEFAULT_BASE_URL)
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }
}

impl Default for ComfyUiProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize)]
struct ComfyPromptRequest {
    prompt: serde_json::Value,
}

#[derive(Deserialize)]
struct ComfyPromptResponse {
    prompt_id: String,
}

fn build_workflow(
    prompt: &str,
    width: u32,
    height: u32,
    steps: u32,
    cfg_scale: f32,
    seed: u64,
) -> serde_json::Value {
    serde_json::json!({
        "1": { "class_type": "CLIPTextEncode", "inputs": { "text": prompt, "clip": ["4", 1] } },
        "2": { "class_type": "CLIPTextEncode", "inputs": { "text": "", "clip": ["4", 1] } },
        "3": { "class_type": "KSampler", "inputs": {
            "seed": seed, "steps": steps, "cfg": cfg_scale,
            "sampler_name": "euler", "scheduler": "normal",
            "positive": ["1", 0], "negative": ["2", 0],
            "latent_image": ["5", 0], "model": ["4", 0],
            "denoise": 1.0
        }},
        "4": { "class_type": "CheckpointLoaderSimple",
               "inputs": { "ckpt_name": "sd_xl_base_1.0.safetensors" } },
        "5": { "class_type": "EmptyLatentImage",
               "inputs": { "width": width, "height": height, "batch_size": 1 } },
        "8": { "class_type": "VAEDecode",
               "inputs": { "samples": ["3", 0], "vae": ["4", 2] } },
        "9": { "class_type": "SaveImage",
               "inputs": { "images": ["8", 0], "filename_prefix": "HyprCollab" } }
    })
}

fn time_seed() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(42)
}

fn extract_images(
    history: &serde_json::Value,
    base_url: &str,
    width: u32,
    height: u32,
    seed: u64,
) -> Vec<GeneratedImage> {
    let Some(obj) = history.as_object() else { return Vec::new() };
    obj.values()
        .filter_map(|entry| entry.get("outputs")?.as_object())
        .flat_map(|outputs| outputs.values())
        .filter_map(|node| node.get("images")?.as_array())
        .flatten()
        .filter_map(|img| {
            let filename = img.get("filename")?.as_str()?;
            let subfolder = img.get("subfolder").and_then(|s| s.as_str()).unwrap_or("");
            let img_type = img.get("type").and_then(|t| t.as_str()).unwrap_or("output");
            let url = format!(
                "{}/view?filename={}&subfolder={}&type={}",
                base_url, filename, subfolder, img_type
            );
            Some(GeneratedImage {
                url: Some(url),
                data: None,
                revised_prompt: None,
                width,
                height,
                seed: Some(seed),
            })
        })
        .collect()
}

#[async_trait]
impl ImageProvider for ComfyUiProvider {
    fn name(&self) -> &str {
        "comfyui"
    }

    fn provider_type(&self) -> ImageProviderType {
        ImageProviderType::ComfyUi
    }

    fn supported_sizes(&self) -> &[ImageSize] {
        SUPPORTED_SIZES
    }

    fn default_model(&self) -> &str {
        DEFAULT_MODEL
    }

    async fn generate(&self, request: ImageRequest) -> Result<ImageResponse> {
        let (w, h) = request.size.dimensions();
        let steps = request.steps.unwrap_or(20);
        let cfg = request.cfg_scale.unwrap_or(7.0);
        let seed = request.seed.unwrap_or_else(time_seed);

        let workflow = build_workflow(&request.prompt, w, h, steps, cfg, seed);
        let body = ComfyPromptRequest { prompt: workflow };

        debug!("comfyui submit prompt");
        let resp = self
            .client
            .post(format!("{}/prompt", self.base_url))
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ImageError::ApiError { status: status.as_u16(), message: text });
        }

        let prompt_resp: ComfyPromptResponse = resp.json().await?;
        let prompt_id = prompt_resp.prompt_id;

        debug!("comfyui poll history: prompt_id={prompt_id}");
        let hist_resp = self
            .client
            .get(format!("{}/history/{}", self.base_url, prompt_id))
            .send()
            .await?;

        let status = hist_resp.status();
        if !status.is_success() {
            return Err(ImageError::ApiError {
                status: status.as_u16(),
                message: "failed to fetch comfyui history".to_string(),
            });
        }

        let history: serde_json::Value = hist_resp.json().await?;
        let images = extract_images(&history, &self.base_url, w, h, seed);

        Ok(ImageResponse {
            images,
            model: DEFAULT_MODEL.to_string(),
            provider: "comfyui".to_string(),
            created: chrono::Utc::now(),
        })
    }
}
