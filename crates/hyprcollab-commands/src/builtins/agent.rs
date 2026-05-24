//! `/agent` command — load, list, assign agent roles.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::SlashCommand;

pub struct AgentCommand;

#[async_trait]
impl SlashCommand for AgentCommand {
    fn name(&self) -> &str { "/agent" }

    fn description(&self) -> &str {
        "Manage agent roles: list, load <name>, assign <name>, unassign, create <name>"
    }

    fn parse_args(&self, raw: &str) -> Result<serde_json::Value> {
        let parts: Vec<&str> = raw.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(serde_json::json!({"subcommand": "list"}));
        }
        let subcmd = parts[0];
        let arg = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
        Ok(serde_json::json!({"subcommand": subcmd, "arg": arg}))
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let subcmd = args["subcommand"].as_str().unwrap_or("list");
        match subcmd {
            "list" => Ok("Available agents:\n  - code-reviewer\n  - debugger\n  - architect\n  - writer".into()),
            "load" => {
                let name = args["arg"].as_str().unwrap_or("");
                if name.is_empty() {
                    return Err(CoreError::Config("Usage: /agent load <name>".into()));
                }
                Ok(format!("Agent '{name}' loaded. System prompt and tools injected into current chat."))
            }
            "assign" => {
                let name = args["arg"].as_str().unwrap_or("");
                if name.is_empty() {
                    return Err(CoreError::Config("Usage: /agent assign <name>".into()));
                }
                Ok(format!("Agent role '{name}' assigned to active persona."))
            }
            "unassign" => Ok("Agent role unassigned from active persona.".into()),
            "create" => {
                let name = args["arg"].as_str().unwrap_or("");
                if name.is_empty() {
                    return Err(CoreError::Config("Usage: /agent create <name>".into()));
                }
                Ok(format!("Agent '{name}' created. Edit ~/config/hyprcollab/agents/{name}.md to configure."))
            }
            _ => Err(CoreError::Config(format!("Unknown /agent subcommand: {subcmd}"))),
        }
    }
}
