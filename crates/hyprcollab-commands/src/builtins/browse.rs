//! `/browse` command — open browser to URL.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::SlashCommand;
use hyprcollab_core::types::CommandContext;

pub struct BrowseCommand;

#[async_trait]
impl SlashCommand for BrowseCommand {
    fn name(&self) -> &str { "/browse" }

    fn description(&self) -> &str {
        "Open a URL in the embedded browser or navigate to it"
    }

    async fn execute(&self, args: serde_json::Value, _ctx: &mut CommandContext) -> Result<String> {
        let url = args["raw"].as_str().unwrap_or("").trim().to_string();

        if url.is_empty() {
            return Err(CoreError::Config("Usage: /browse <url>".into()));
        }

        Ok(format!("Opening browser: {url}"))
    }
}
