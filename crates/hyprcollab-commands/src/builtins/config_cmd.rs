//! `/config` command — view or edit configuration.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::SlashCommand;
use hyprcollab_core::types::CommandContext;

pub struct ConfigCommand;

#[async_trait]
impl SlashCommand for ConfigCommand {
    fn name(&self) -> &str { "/config" }

    fn description(&self) -> &str {
        "View or edit configuration: list | get <key> | set <key> <value>"
    }

    async fn execute(&self, args: serde_json::Value, ctx: &mut CommandContext) -> Result<String> {
        let parts: Vec<String> = args["args"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default();

        let subcmd = parts.first().map(|s| s.as_str()).unwrap_or("list");

        match subcmd {
            "list" => Ok(format!(
                "Configuration:\n  model:         {}\n  temperature:   {}\n  approval_mode: {}\n  session_id:    {}",
                ctx.model,
                ctx.temperature.map(|t| t.to_string()).unwrap_or_else(|| "default".into()),
                ctx.approval_mode,
                ctx.session_id,
            )),

            "get" => {
                let key = parts.get(1).map(|s| s.as_str()).unwrap_or("");
                match key {
                    "model" => Ok(format!("model = {}", ctx.model)),
                    "temperature" => Ok(format!(
                        "temperature = {}",
                        ctx.temperature.map(|t| t.to_string()).unwrap_or_else(|| "default".into())
                    )),
                    "approval_mode" => Ok(format!("approval_mode = {}", ctx.approval_mode)),
                    "" => Err(CoreError::Config("Usage: /config get <key>".into())),
                    k => Err(CoreError::Config(format!("Unknown config key: '{k}'"))),
                }
            }

            "set" => {
                if parts.len() < 3 {
                    return Err(CoreError::Config(
                        "Usage: /config set <key> <value>".into(),
                    ));
                }
                let key = parts[1].as_str();
                let value = parts[2..].join(" ");

                match key {
                    "model" => {
                        ctx.model = value.clone();
                        Ok(format!("Set model = {value}"))
                    }
                    "temperature" => {
                        let val: f32 = value.parse().map_err(|_| {
                            CoreError::Config(format!("Invalid temperature: '{value}'"))
                        })?;
                        if !(0.0..=2.0).contains(&val) {
                            return Err(CoreError::Config(
                                "Temperature must be between 0.0 and 2.0".into(),
                            ));
                        }
                        ctx.temperature = Some(val);
                        Ok(format!("Set temperature = {val}"))
                    }
                    "approval_mode" => {
                        ctx.approval_mode = match value.as_str() {
                            "auto" => hyprcollab_core::types::ApprovalMode::Auto,
                            "normal" => hyprcollab_core::types::ApprovalMode::Normal,
                            "strict" => hyprcollab_core::types::ApprovalMode::Strict,
                            m => {
                                return Err(CoreError::Config(format!(
                                    "Invalid approval_mode: '{m}'"
                                )))
                            }
                        };
                        Ok(format!("Set approval_mode = {value}"))
                    }
                    k => Err(CoreError::Config(format!("Unknown config key: '{k}'"))),
                }
            }

            _ => Err(CoreError::Config(format!(
                "Unknown /config subcommand: '{subcmd}'"
            ))),
        }
    }
}
