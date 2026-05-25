use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImageError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("provider: {0}")]
    Provider(String),
    #[error("provider not found: '{0}'")]
    ProviderNotFound(String),
    #[error("no default provider configured")]
    NoDefaultProvider,
    #[error("decode: {0}")]
    Decode(String),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("api error {status}: {message}")]
    ApiError { status: u16, message: String },
}

pub type Result<T> = std::result::Result<T, ImageError>;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageSize {
    #[default]
    Square1024,
    Landscape1792x1024,
    Portrait1024x1792,
    Custom { width: u32, height: u32 },
}

impl ImageSize {
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            Self::Square1024 => (1024, 1024),
            Self::Landscape1792x1024 => (1792, 1024),
            Self::Portrait1024x1792 => (1024, 1792),
            Self::Custom { width, height } => (*width, *height),
        }
    }

    pub fn to_openai_str(&self) -> &str {
        match self {
            Self::Square1024 => "1024x1024",
            Self::Landscape1792x1024 => "1792x1024",
            Self::Portrait1024x1792 => "1024x1792",
            Self::Custom { .. } => "1024x1024",
        }
    }

    pub fn to_fal_size(&self) -> serde_json::Value {
        let (w, h) = self.dimensions();
        serde_json::json!({ "width": w, "height": h })
    }
}


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageStyle {
    Vivid,
    Natural,
}

impl ImageStyle {
    pub fn to_openai_str(&self) -> &str {
        match self {
            Self::Vivid => "vivid",
            Self::Natural => "natural",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageQuality {
    Standard,
    Hd,
}

impl ImageQuality {
    pub fn to_openai_str(&self) -> &str {
        match self {
            Self::Standard => "standard",
            Self::Hd => "hd",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImageProviderType {
    OpenAi,
    Fal,
    ComfyUi,
    Automatic1111,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRequest {
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub size: ImageSize,
    pub model: Option<String>,
    pub n: u32,
    pub style: Option<ImageStyle>,
    pub quality: Option<ImageQuality>,
    pub seed: Option<u64>,
    pub steps: Option<u32>,
    pub cfg_scale: Option<f32>,
}

impl ImageRequest {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            negative_prompt: None,
            size: ImageSize::default(),
            model: None,
            n: 1,
            style: None,
            quality: None,
            seed: None,
            steps: None,
            cfg_scale: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedImage {
    pub url: Option<String>,
    pub data: Option<Vec<u8>>,
    pub revised_prompt: Option<String>,
    pub width: u32,
    pub height: u32,
    pub seed: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageResponse {
    pub images: Vec<GeneratedImage>,
    pub model: String,
    pub provider: String,
    pub created: chrono::DateTime<chrono::Utc>,
}
