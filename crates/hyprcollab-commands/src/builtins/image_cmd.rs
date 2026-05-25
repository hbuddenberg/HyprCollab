//! `/image` command — generate an image from a text prompt.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::SlashCommand;
use hyprcollab_core::types::CommandContext;

pub struct ImageCommand;

#[async_trait]
impl SlashCommand for ImageCommand {
    fn name(&self) -> &str {
        "/image"
    }

    fn description(&self) -> &str {
        "Generate an image from a text prompt: /image <prompt>"
    }

    async fn execute(&self, args: serde_json::Value, _ctx: &mut CommandContext) -> Result<String> {
        let prompt = args["raw"].as_str().unwrap_or("").trim().to_string();

        if prompt.is_empty() {
            return Err(CoreError::Config("Usage: /image <prompt>".into()));
        }

        Ok(format!("Generating image: {prompt}"))
    }
}
