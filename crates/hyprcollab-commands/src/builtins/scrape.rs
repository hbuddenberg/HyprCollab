//! `/scrape` command — scrape the text content of a URL.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::SlashCommand;
use hyprcollab_core::types::CommandContext;

pub struct ScrapeCommand;

#[async_trait]
impl SlashCommand for ScrapeCommand {
    fn name(&self) -> &str {
        "/scrape"
    }

    fn description(&self) -> &str {
        "Scrape text content from a URL: /scrape <url>"
    }

    async fn execute(&self, args: serde_json::Value, _ctx: &mut CommandContext) -> Result<String> {
        let url = args["raw"].as_str().unwrap_or("").trim().to_string();

        if url.is_empty() {
            return Err(CoreError::Config("Usage: /scrape <url>".into()));
        }

        Ok(format!("Scraping: {url}"))
    }
}
