//! Shell tool — execute system commands with output capture.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::Tool;
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Shell tool: executes a command and returns stdout + stderr.
pub struct ShellTool {
    /// Default working directory for commands.
    default_cwd: Option<String>,
    /// Maximum execution time in seconds.
    timeout_secs: u64,
}

/// Input parameters for the shell tool.
#[derive(Debug, Deserialize)]
struct ShellParams {
    /// The command to execute.
    command: String,
    /// Optional working directory.
    #[serde(default)]
    cwd: Option<String>,
}

/// Output structure for the shell tool.
#[derive(Debug, Serialize, Deserialize)]
pub struct ShellOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}

impl ShellTool {
    pub fn new() -> Self {
        Self {
            default_cwd: None,
            timeout_secs: 30,
        }
    }

    pub fn with_cwd(mut self, cwd: impl Into<String>) -> Self {
        self.default_cwd = Some(cwd.into());
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

impl Default for ShellTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        "Execute a system command and return stdout, stderr, and exit code."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to execute"
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory (optional)"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let params: ShellParams = serde_json::from_value(args)
            .map_err(|e| CoreError::Tool(format!("Invalid shell params: {e}")))?;

        let cwd = params.cwd.as_deref().or(self.default_cwd.as_deref());

        let start = Instant::now();

        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(&params.command);

        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(self.timeout_secs),
            cmd.output(),
        )
        .await
        .map_err(|_| {
            CoreError::Tool(format!(
                "Command timed out after {}s: {}",
                self.timeout_secs, params.command
            ))
        })?
        .map_err(|e| CoreError::Tool(format!("Failed to execute command: {e}")))?;

        let duration_ms = start.elapsed().as_millis() as u64;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        let result = ShellOutput {
            stdout,
            stderr,
            exit_code,
            duration_ms,
        };

        Ok(serde_json::to_string(&result)
            .unwrap_or_else(|_| "Failed to serialize shell output".to_string()))
    }
}
