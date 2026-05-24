//! `/help` command — list available commands.

use async_trait::async_trait;
use hyprcollab_core::errors::Result;
use hyprcollab_core::traits::SlashCommand;
use hyprcollab_core::types::CommandContext;

pub struct HelpCommand;

#[async_trait]
impl SlashCommand for HelpCommand {
    fn name(&self) -> &str { "/help" }

    fn description(&self) -> &str {
        "Show available slash commands and their descriptions"
    }

    async fn execute(&self, args: serde_json::Value, _ctx: &mut CommandContext) -> Result<String> {
        let query = args["raw"].as_str().unwrap_or("").trim().to_string();

        let commands = [
            ("/agent",       "Manage agent roles: list, load, assign, create"),
            ("/approval",    "Set approval mode: strict, normal, auto"),
            ("/browse",      "Open URL in embedded browser"),
            ("/config",      "View or edit configuration"),
            ("/design",      "Load design review mode"),
            ("/help",        "Show this help message"),
            ("/review",      "Start code review mode"),
            ("/run",         "Execute command with approval check"),
            ("/skill",       "Manage skills: list, load"),
            ("/temperature", "Set chat temperature (0.0–2.0)"),
        ];

        if query.is_empty() {
            let mut lines = vec!["Available commands:\n".to_string()];
            for (name, desc) in &commands {
                lines.push(format!("  {name:<15} {desc}"));
            }
            lines.push("\nType /help <command> for details.".to_string());
            Ok(lines.join("\n"))
        } else {
            for (name, desc) in &commands {
                if name.contains(&query) || desc.to_lowercase().contains(&query.to_lowercase()) {
                    return Ok(format!("{name} — {desc}"));
                }
            }
            Ok(format!("No command matching '{query}' found."))
        }
    }
}
