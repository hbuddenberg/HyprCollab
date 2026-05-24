//! `/temperature` command — adjust chat temperature.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::SlashCommand;
use hyprcollab_core::types::CommandContext;

pub struct TemperatureCommand;

#[async_trait]
impl SlashCommand for TemperatureCommand {
    fn name(&self) -> &str { "/temperature" }

    fn description(&self) -> &str {
        "Set or show the temperature for the current chat (0.0 - 2.0)"
    }

    async fn execute(&self, args: serde_json::Value, ctx: &mut CommandContext) -> Result<String> {
        let raw = args["raw"].as_str().unwrap_or("").trim().to_string();

        if raw.is_empty() {
            let current = ctx.temperature.unwrap_or(0.7);
            return Ok(format!("Current temperature: {current}"));
        }

        let val: f32 = raw
            .parse()
            .map_err(|_| CoreError::Config(format!("Invalid temperature: '{raw}'")))?;

        if !(0.0..=2.0).contains(&val) {
            return Err(CoreError::Config(
                "Temperature must be between 0.0 and 2.0".into(),
            ));
        }

        ctx.temperature = Some(val);
        Ok(format!("Temperature set to {val}"))
    }
}
