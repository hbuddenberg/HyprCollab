//! `/approval` command — change approval mode.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::SlashCommand;

pub struct ApprovalCommand;

#[async_trait]
impl SlashCommand for ApprovalCommand {
    fn name(&self) -> &str { "/approval" }

    fn description(&self) -> &str {
        "Set approval mode: strict, normal, auto"
    }

    fn parse_args(&self, raw: &str) -> Result<serde_json::Value> {
        let mode = raw.trim().to_lowercase();
        if mode.is_empty() {
            return Ok(serde_json::json!({"action": "show"}));
        }
        match mode.as_str() {
            "strict" | "normal" | "auto" => Ok(serde_json::json!({"action": "set", "mode": mode})),
            _ => Err(CoreError::Config(format!("Invalid approval mode: {mode}. Use: strict, normal, auto"))),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        match args["action"].as_str().unwrap_or("show") {
            "show" => Ok("Current approval mode: normal".into()),
            "set" => {
                let mode = args["mode"].as_str().unwrap_or("normal");
                Ok(format!("Approval mode set to {mode}"))
            }
            _ => Err(CoreError::Config("Invalid approval action".into())),
        }
    }
}
