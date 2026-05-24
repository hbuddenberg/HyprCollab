//! `/design` command — load design mode.

use async_trait::async_trait;
use hyprcollab_core::errors::Result;
use hyprcollab_core::traits::SlashCommand;
use hyprcollab_core::types::CommandContext;

pub struct DesignCommand;

#[async_trait]
impl SlashCommand for DesignCommand {
    fn name(&self) -> &str { "/design" }

    fn description(&self) -> &str {
        "Load a design file and switch to design review mode"
    }

    async fn execute(&self, args: serde_json::Value, _ctx: &mut CommandContext) -> Result<String> {
        let raw = args["raw"].as_str().unwrap_or("").trim().to_string();

        if raw.is_empty() {
            Ok("Design mode: provide a .md file to review.\nUsage: /design path/to/DESIGN.md".into())
        } else {
            Ok(format!("Design mode activated. Reviewing: {raw}"))
        }
    }
}
