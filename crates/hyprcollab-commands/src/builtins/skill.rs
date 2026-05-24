//! `/skill` command — load, list skills.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::SlashCommand;

pub struct SkillCommand;

#[async_trait]
impl SlashCommand for SkillCommand {
    fn name(&self) -> &str { "/skill" }

    fn description(&self) -> &str {
        "Manage skills: list, load <name>"
    }

    fn parse_args(&self, raw: &str) -> Result<serde_json::Value> {
        let parts: Vec<&str> = raw.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(serde_json::json!({"subcommand": "list"}));
        }
        Ok(serde_json::json!({"subcommand": parts[0], "arg": parts.get(1).unwrap_or(&"").to_string()}))
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let subcmd = args["subcommand"].as_str().unwrap_or("list");
        match subcmd {
            "list" => Ok("Available skills:\n  - debugging\n  - code-review\n  - planning\n  - testing".into()),
            "load" => {
                let name = args["arg"].as_str().unwrap_or("");
                if name.is_empty() {
                    return Err(CoreError::Config("Usage: /skill load <name>".into()));
                }
                Ok(format!("Skill '{name}' loaded into current chat context."))
            }
            _ => Err(CoreError::Config(format!("Unknown /skill subcommand: {subcmd}"))),
        }
    }
}
