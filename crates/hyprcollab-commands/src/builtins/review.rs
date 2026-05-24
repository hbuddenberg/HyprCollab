//! `/review` command — load code review mode.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::SlashCommand;

pub struct ReviewCommand;

#[async_trait]
impl SlashCommand for ReviewCommand {
    fn name(&self) -> &str { "/review" }

    fn description(&self) -> &str {
        "Start code review mode for a file or diff"
    }

    fn parse_args(&self, raw: &str) -> Result<serde_json::Value> {
        if raw.trim().is_empty() {
            return Ok(serde_json::json!({"action": "info"}));
        }
        Ok(serde_json::json!({"action": "review", "target": raw.trim()}))
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        match args["action"].as_str().unwrap_or("info") {
            "info" => Ok("Review mode: provide a file path or diff.\nUsage: /review path/to/file.rs".into()),
            "review" => {
                let target = args["target"].as_str().unwrap_or("");
                Ok(format!("🔍 Code review mode activated. Target: {target}"))
            }
            _ => Err(CoreError::Config("Invalid review action".into())),
        }
    }
}
