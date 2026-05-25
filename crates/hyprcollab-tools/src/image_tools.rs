use std::sync::Arc;

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::Tool;
use hyprcollab_image::{ImageRequest, ImageRouter, ImageSize};

// ── ImageGenerateTool ─────────────────────────────────────────────────────────

pub struct ImageGenerateTool {
    router: Arc<ImageRouter>,
}

impl ImageGenerateTool {
    pub fn new(router: Arc<ImageRouter>) -> Self {
        Self { router }
    }
}

#[async_trait]
impl Tool for ImageGenerateTool {
    fn name(&self) -> &str {
        "image_generate"
    }

    fn description(&self) -> &str {
        "Generate an image from a text prompt using the configured image provider."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string", "description": "Text description of the image to generate" },
                "provider": { "type": "string", "description": "Provider name to use (optional, uses default)" },
                "size": {
                    "type": "string",
                    "description": "Image size: 'square' (1024x1024), 'landscape' (1792x1024), 'portrait' (1024x1792)",
                    "enum": ["square", "landscape", "portrait"],
                    "default": "square"
                },
                "n": { "type": "integer", "description": "Number of images to generate (default: 1)", "default": 1 }
            },
            "required": ["prompt"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let prompt = args["prompt"]
            .as_str()
            .ok_or_else(|| CoreError::Tool("Missing 'prompt' parameter".into()))?;

        let size = match args["size"].as_str().unwrap_or("square") {
            "landscape" => ImageSize::Landscape1792x1024,
            "portrait" => ImageSize::Portrait1024x1792,
            _ => ImageSize::Square1024,
        };

        let n = args["n"].as_u64().unwrap_or(1) as u32;

        let mut req = ImageRequest::new(prompt);
        req.size = size;
        req.n = n;

        let resp = if let Some(provider) = args["provider"].as_str() {
            self.router
                .generate_with(provider, req)
                .await
                .map_err(|e| CoreError::Tool(format!("image_generate: {e}")))?
        } else {
            self.router
                .generate(req)
                .await
                .map_err(|e| CoreError::Tool(format!("image_generate: {e}")))?
        };

        let images: Vec<serde_json::Value> = resp
            .images
            .iter()
            .map(|img| {
                serde_json::json!({
                    "url": img.url,
                    "width": img.width,
                    "height": img.height,
                    "revised_prompt": img.revised_prompt,
                    "seed": img.seed,
                })
            })
            .collect();

        serde_json::to_string(&serde_json::json!({
            "images": images,
            "model": resp.model,
            "provider": resp.provider,
        }))
        .map_err(|e| CoreError::Tool(e.to_string()))
    }
}

// ── ImageListProvidersTool ────────────────────────────────────────────────────

pub struct ImageListProvidersTool {
    router: Arc<ImageRouter>,
}

impl ImageListProvidersTool {
    pub fn new(router: Arc<ImageRouter>) -> Self {
        Self { router }
    }
}

#[async_trait]
impl Tool for ImageListProvidersTool {
    fn name(&self) -> &str {
        "image_list_providers"
    }

    fn description(&self) -> &str {
        "List all configured image generation providers."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _args: serde_json::Value) -> Result<String> {
        let names = self.router.provider_names();
        serde_json::to_string(&serde_json::json!({ "providers": names }))
            .map_err(|e| CoreError::Tool(e.to_string()))
    }
}
