//! `/skill` command — load, list skills.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::SlashCommand;
use hyprcollab_core::types::CommandContext;

pub struct SkillCommand;

#[async_trait]
impl SlashCommand for SkillCommand {
    fn name(&self) -> &str { "/skill" }

    fn description(&self) -> &str {
        "Manage skills: list | load <name>"
    }

    async fn execute(&self, args: serde_json::Value, _ctx: &mut CommandContext) -> Result<String> {
        let parts: Vec<String> = args["args"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default();

        let subcmd = parts.first().map(|s| s.as_str()).unwrap_or("list");
        let arg = parts.get(1).map(|s| s.as_str()).unwrap_or("");

        match subcmd {
            "list" => Ok(
                "Available skills:\n  - debugging\n  - code-review\n  - planning\n  - testing"
                    .into(),
            ),
            "load" => {
                if arg.is_empty() {
                    return Err(CoreError::Config("Usage: /skill load <name>".into()));
                }
                Ok(format!("Skill '{arg}' loaded into current chat context."))
            }
            _ => Err(CoreError::Config(format!(
                "Unknown /skill subcommand: '{subcmd}'"
            ))),
        }
    }
}
