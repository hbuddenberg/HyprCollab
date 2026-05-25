use std::collections::HashMap;

use async_trait::async_trait;

use crate::types::{ImageError, ImageProviderType, ImageRequest, ImageResponse, ImageSize, Result};

#[async_trait]
pub trait ImageProvider: Send + Sync {
    fn name(&self) -> &str;
    fn provider_type(&self) -> ImageProviderType;
    async fn generate(&self, request: ImageRequest) -> Result<ImageResponse>;
    fn supported_sizes(&self) -> &[ImageSize];
    fn default_model(&self) -> &str;
}

pub struct ImageRouter {
    providers: HashMap<String, Box<dyn ImageProvider>>,
    default: Option<String>,
}

impl ImageRouter {
    pub fn new() -> Self {
        Self { providers: HashMap::new(), default: None }
    }

    pub fn add<P: ImageProvider + 'static>(&mut self, provider: P) {
        self.providers.insert(provider.name().to_string(), Box::new(provider));
    }

    /// Returns true if the named provider exists and the default was set.
    pub fn set_default(&mut self, name: &str) -> bool {
        if self.providers.contains_key(name) {
            self.default = Some(name.to_string());
            true
        } else {
            false
        }
    }

    pub fn has_provider(&self, name: &str) -> bool {
        self.providers.contains_key(name)
    }

    pub fn provider_names(&self) -> Vec<&str> {
        self.providers.keys().map(String::as_str).collect()
    }

    pub async fn generate(&self, request: ImageRequest) -> Result<ImageResponse> {
        let name = self.default.as_deref().ok_or(ImageError::NoDefaultProvider)?;
        self.generate_with(name, request).await
    }

    pub async fn generate_with(&self, name: &str, request: ImageRequest) -> Result<ImageResponse> {
        let provider = self
            .providers
            .get(name)
            .ok_or_else(|| ImageError::ProviderNotFound(name.to_string()))?;
        provider.generate(request).await
    }
}

impl Default for ImageRouter {
    fn default() -> Self {
        Self::new()
    }
}
