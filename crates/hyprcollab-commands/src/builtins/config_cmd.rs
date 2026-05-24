//! `/config` command — view or edit configuration.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::SlashCommand;

pub struct ConfigCommand;

#[async_trait]
impl SlashCommand for ConfigCommand {
    fn name(&self) -> &str { "/config" }

    fn description(&self) -> &str {
        "View or edit configuration: get <key>, set <key> <value>, list"
    }

    fn parse_args(&self, raw: &str) -> Result<serde_json::Value> {
        let parts: Vec<&str> = raw.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(serde_json::json!({"subcommand": "list"}));
        }
        match parts[0] {
            "get" => Ok(serde_json::json!({"subcommand": "get", "key": parts.get(1).unwrap_or(&"")})),
            "set" => {
                if parts.len() < 3 {
                    return Err(CoreError::Config("Usage: /config set <key> <value>".into()));
                }
                let value = parts[2..].join(" ");
                Ok(serde_json::json!({"subcommand": "set", "key": parts[1], "value": value}))
            }
            "list" => Ok(serde_json::json!({"subcommand": "list"})),
            _ => Err(CoreError::Config(format!("Unknown /config subcommand: {}", parts[0]))),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let subcmd = args["subcommand"].as_str().unwrap_or("list");
        match subcmd {
            "list" => Ok("Configuration:\n  model: openai/gpt-4o\n  temperature: 0.7\n  approval_mode: normal\n  max_turns: 10".into()),
            "get" => {
                let key = args["key"].as_str().unwrap_or("");
                Ok(format!("{key}: (not implemented - placeholder)"))
            }
            "set" => {
                let key = args["key"].as_str().unwrap_or("");
                let value = args["value"].as_str().unwrap_or("");
                Ok(format!("Set {key} = {value} (placeholder — will persist to config)"))
            }
            _ => Err(CoreError::Config(format!("Unknown config subcommand: {subcmd}"))),
        }
    }
}
