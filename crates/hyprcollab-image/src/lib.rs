pub mod a1111;
pub mod comfyui;
pub mod fal;
pub mod openai;
pub mod provider;
pub mod types;

#[cfg(test)]
mod tests;

pub use a1111::A1111Provider;
pub use comfyui::ComfyUiProvider;
pub use fal::FalProvider;
pub use openai::OpenAiProvider;
pub use provider::{ImageProvider, ImageRouter};
pub use types::{
    GeneratedImage, ImageError, ImageProviderType, ImageQuality, ImageRequest, ImageResponse,
    ImageSize, ImageStyle, Result,
};
