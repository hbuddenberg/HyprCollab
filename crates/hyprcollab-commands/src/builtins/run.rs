//! `/run` command — execute a command with approval check.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::SlashCommand;

pub struct RunCommand;

#[async_trait]
impl SlashCommand for RunCommand {
    fn name(&self) -> &str { "/run" }

    fn description(&self) -> &str {
        "Execute a shell command with approval check"
    }

    fn parse_args(&self, raw: &str) -> Result<serde_json::Value> {
        if raw.trim().is_empty() {
            return Err(CoreError::Config("Usage: /run <command>".into()));
        }
        Ok(serde_json::json!({"command": raw.trim()}))
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let cmd = args["command"].as_str().unwrap_or("");
        if cmd.is_empty() {
            return Err(CoreError::Config("No command specified".into()));
        }
        // In a real implementation, this would go through the approval engine.
        Ok(format!("⏳ Approval requested for: {cmd}\n✅ Approved and executed (placeholder)."))
    }
}
