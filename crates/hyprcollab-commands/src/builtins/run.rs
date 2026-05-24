//! `/run` command — execute a shell command via the approval check.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::SlashCommand;
use hyprcollab_core::types::{ApprovalMode, CommandContext};

pub struct RunCommand;

#[async_trait]
impl SlashCommand for RunCommand {
    fn name(&self) -> &str { "/run" }

    fn description(&self) -> &str {
        "Execute a shell command (blocked in Strict mode unless explicitly approved)"
    }

    async fn execute(&self, args: serde_json::Value, ctx: &mut CommandContext) -> Result<String> {
        let cmd = args["raw"].as_str().unwrap_or("").trim().to_string();

        if cmd.is_empty() {
            return Err(CoreError::Config("Usage: /run <command>".into()));
        }

        if ctx.approval_mode == ApprovalMode::Strict {
            return Err(CoreError::Config(format!(
                "Command '{cmd}' requires explicit approval in Strict mode"
            )));
        }

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .output()
            .await
            .map_err(|e| CoreError::Tool(format!("Failed to spawn command: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let code = output.status.code().unwrap_or(-1);

        let mut result = format!("$ {cmd}\n");
        if !stdout.is_empty() {
            result.push_str(&stdout);
        }
        if !stderr.is_empty() {
            result.push_str(&format!("[stderr] {stderr}"));
        }
        if code != 0 {
            result.push_str(&format!("[exit {code}]"));
        }

        Ok(result.trim_end().to_string())
    }
}
