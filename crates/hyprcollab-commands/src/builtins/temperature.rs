//! `/temperature` command — adjust chat temperature.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::SlashCommand;

pub struct TemperatureCommand;

#[async_trait]
impl SlashCommand for TemperatureCommand {
    fn name(&self) -> &str { "/temperature" }

    fn description(&self) -> &str {
        "Set or show the temperature for the current chat (0.0 - 2.0)"
    }

    fn parse_args(&self, raw: &str) -> Result<serde_json::Value> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(serde_json::json!({"action": "show"}));
        }
        let val: f32 = trimmed.parse()
            .map_err(|_| CoreError::Config(format!("Invalid temperature: {trimmed}")))?;
        if val < 0.0 || val > 2.0 {
            return Err(CoreError::Config("Temperature must be between 0.0 and 2.0".into()));
        }
        Ok(serde_json::json!({"action": "set", "value": val}))
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        match args["action"].as_str().unwrap_or("show") {
            "show" => Ok("Current temperature: 0.7 (default)".into()),
            "set" => {
                let val = args["value"].as_f64().unwrap_or(0.7) as f32;
                Ok(format!("Temperature set to {val}"))
            }
            _ => Err(CoreError::Config("Invalid temperature action".into())),
        }
    }
}
