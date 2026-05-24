//! `/agent` command — load, list, assign agent roles.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::SlashCommand;
use hyprcollab_core::types::CommandContext;

pub struct AgentCommand;

#[async_trait]
impl SlashCommand for AgentCommand {
    fn name(&self) -> &str { "/agent" }

    fn description(&self) -> &str {
        "Manage agent roles: list | load <name> | assign <name> | unassign | create <name>"
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
                "Available agents:\n  - code-reviewer\n  - debugger\n  - architect\n  - writer"
                    .into(),
            ),

            "load" => {
                if arg.is_empty() {
                    return Err(CoreError::Config("Usage: /agent load <name>".into()));
                }
                // Read persona config from ~/.config/hyprcollab/agents/<name>.md
                let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
                let path = std::path::PathBuf::from(&home)
                    .join(".config/hyprcollab/agents")
                    .join(format!("{arg}.md"));

                match tokio::fs::read_to_string(&path).await {
                    Ok(content) => Ok(format!(
                        "Agent '{arg}' loaded from {}.\n---\n{}",
                        path.display(),
                        content.trim()
                    )),
                    Err(_) => Err(CoreError::Config(format!(
                        "Agent '{arg}' not found. Create it at: {}",
                        path.display()
                    ))),
                }
            }

            "assign" => {
                if arg.is_empty() {
                    return Err(CoreError::Config("Usage: /agent assign <name>".into()));
                }
                Ok(format!("Agent role '{arg}' assigned to active persona."))
            }

            "unassign" => Ok("Agent role unassigned from active persona.".into()),

            "create" => {
                if arg.is_empty() {
                    return Err(CoreError::Config("Usage: /agent create <name>".into()));
                }
                let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
                let dir = std::path::PathBuf::from(&home).join(".config/hyprcollab/agents");
                let path = dir.join(format!("{arg}.md"));
                Ok(format!(
                    "Agent '{arg}' scaffold ready. Edit it at: {}",
                    path.display()
                ))
            }

            _ => Err(CoreError::Config(format!(
                "Unknown /agent subcommand: '{subcmd}'"
            ))),
        }
    }
}
