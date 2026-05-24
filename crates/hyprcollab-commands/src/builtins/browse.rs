//! `/browse` command — open browser to URL.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::SlashCommand;

pub struct BrowseCommand;

#[async_trait]
impl SlashCommand for BrowseCommand {
    fn name(&self) -> &str { "/browse" }

    fn description(&self) -> &str {
        "Open a URL in the embedded browser or navigate to it"
    }

    fn parse_args(&self, raw: &str) -> Result<serde_json::Value> {
        if raw.trim().is_empty() {
            return Err(CoreError::Config("Usage: /browse <url>".into()));
        }
        Ok(serde_json::json!({"url": raw.trim()}))
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let url = args["url"].as_str().unwrap_or("");
        if url.is_empty() {
            return Err(CoreError::Config("No URL specified".into()));
        }
        Ok(format!("🌐 Opening browser: {url}\n📸 Screenshot captured (placeholder)."))
    }
}
