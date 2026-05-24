//! `/approval` command — change approval mode.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::SlashCommand;
use hyprcollab_core::types::{ApprovalMode, CommandContext};

pub struct ApprovalCommand;

#[async_trait]
impl SlashCommand for ApprovalCommand {
    fn name(&self) -> &str { "/approval" }

    fn description(&self) -> &str {
        "Set approval mode: strict | normal | auto"
    }

    async fn execute(&self, args: serde_json::Value, ctx: &mut CommandContext) -> Result<String> {
        let raw = args["raw"].as_str().unwrap_or("").trim().to_lowercase();

        if raw.is_empty() {
            return Ok(format!("Current approval mode: {}", ctx.approval_mode));
        }

        let mode = match raw.as_str() {
            "auto" => ApprovalMode::Auto,
            "normal" => ApprovalMode::Normal,
            "strict" => ApprovalMode::Strict,
            m => {
                return Err(CoreError::Config(format!(
                    "Invalid approval mode: '{m}'. Use: strict, normal, auto"
                )))
            }
        };

        ctx.approval_mode = mode;
        Ok(format!("Approval mode set to {mode}"))
    }
}
