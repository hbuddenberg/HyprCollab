//! `/design` command — load design mode.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::SlashCommand;

pub struct DesignCommand;

#[async_trait]
impl SlashCommand for DesignCommand {
    fn name(&self) -> &str { "/design" }

    fn description(&self) -> &str {
        "Load a design file and switch to design review mode"
    }

    fn parse_args(&self, raw: &str) -> Result<serde_json::Value> {
        if raw.trim().is_empty() {
            return Ok(serde_json::json!({"action": "info"}));
        }
        Ok(serde_json::json!({"action": "load", "file": raw.trim()}))
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        match args["action"].as_str().unwrap_or("info") {
            "info" => Ok("Design mode: provide a .md file to review.\nUsage: /design path/to/DESIGN.md".into()),
            "load" => {
                let file = args["file"].as_str().unwrap_or("");
                Ok(format!("🎨 Design mode activated. Reviewing: {file}"))
            }
            _ => Err(CoreError::Config("Invalid design action".into())),
        }
    }
}
