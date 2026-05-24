//! `/review` command — load code review mode.

use async_trait::async_trait;
use hyprcollab_core::errors::Result;
use hyprcollab_core::traits::SlashCommand;
use hyprcollab_core::types::CommandContext;

pub struct ReviewCommand;

#[async_trait]
impl SlashCommand for ReviewCommand {
    fn name(&self) -> &str { "/review" }

    fn description(&self) -> &str {
        "Start code review mode for a file or diff"
    }

    async fn execute(&self, args: serde_json::Value, _ctx: &mut CommandContext) -> Result<String> {
        let target = args["raw"].as_str().unwrap_or("").trim().to_string();

        if target.is_empty() {
            Ok("Review mode: provide a file path or diff.\nUsage: /review path/to/file.rs".into())
        } else {
            Ok(format!("Code review mode activated. Target: {target}"))
        }
    }
}
