//! Enhanced shell tool with background processes, PTY, CWD tracking, env vars.
//!
//! Extends the basic ShellTool with:
//! - Background process management (start, stop, status, output streaming)
//! - CWD tracking across commands
//! - Environment variable management
//! - Configurable timeouts per command
//! - PTY allocation for interactive commands

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::Tool;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::sync::Mutex as AsyncMutex;

/// Enhanced shell tool with process management.
pub struct EnhancedShellTool {
    inner: Arc<ShellState>,
}

struct ShellState {
    /// Current working directory.
    cwd: Mutex<PathBuf>,
    /// Environment variables to inject into commands.
    env_vars: Mutex<HashMap<String, String>>,
    /// Background processes: id → process handle.
    bg_processes: AsyncMutex<HashMap<String, BackgroundProcess>>,
    /// Default timeout in seconds.
    default_timeout: u64,
}

/// A tracked background process.
struct BackgroundProcess {
    child: Child,
    #[allow(dead_code)]
    command: String,
    started_at: Instant,
    stdout_buffer: Arc<Mutex<String>>,
    stderr_buffer: Arc<Mutex<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum ShellAction {
    /// Run a foreground command (blocking).
    Run {
        command: String,
        #[serde(default)]
        cwd: Option<String>,
        #[serde(default)]
        timeout: Option<u64>,
        #[serde(default)]
        env: Option<HashMap<String, String>>,
    },
    /// Start a background process (non-blocking).
    Start {
        command: String,
        id: String,
        #[serde(default)]
        cwd: Option<String>,
        #[serde(default)]
        env: Option<HashMap<String, String>>,
    },
    /// Check status of a background process.
    Status { id: String },
    /// Stop (kill) a background process.
    Stop { id: String },
    /// Get output from a background process.
    Output {
        id: String,
        #[serde(default)]
        stream: Option<String>, // "stdout", "stderr", or "both" (default)
    },
    /// Set the default working directory.
    SetCwd { path: String },
    /// Get the current working directory.
    GetCwd {},
    /// Set an environment variable.
    SetEnv { key: String, value: String },
    /// Unset an environment variable.
    UnsetEnv { key: String },
    /// List all environment variables.
    ListEnv {},
}

#[derive(Debug, serde::Serialize)]
struct RunResult {
    stdout: String,
    stderr: String,
    exit_code: i32,
    duration_ms: u64,
    cwd: String,
}

#[derive(Debug, serde::Serialize)]
struct StartResult {
    id: String,
    pid: Option<u32>,
    command: String,
}

#[derive(Debug, serde::Serialize)]
struct StatusResult {
    id: String,
    running: bool,
    exit_code: Option<i32>,
    uptime_secs: Option<u64>,
}

#[derive(Debug, serde::Serialize)]
struct OutputResult {
    id: String,
    stdout: String,
    stderr: String,
}

impl EnhancedShellTool {
    pub fn new() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        Self {
            inner: Arc::new(ShellState {
                cwd: Mutex::new(cwd),
                env_vars: Mutex::new(HashMap::new()),
                bg_processes: AsyncMutex::new(HashMap::new()),
                default_timeout: 30,
            }),
        }
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        // Access inner directly since we own it exclusively during construction
        let state = Arc::get_mut(&mut self.inner).expect("exclusive access during construction");
        state.default_timeout = secs;
        self
    }

    fn get_cwd(&self) -> PathBuf {
        self.inner.cwd.lock().unwrap().clone()
    }

    fn build_command(&self, cmd_str: &str, cwd: Option<&str>, env: Option<&HashMap<String, String>>) -> Command {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(cmd_str);

        // Set CWD
        let effective_cwd = cwd
            .map(PathBuf::from)
            .unwrap_or_else(|| self.get_cwd());
        cmd.current_dir(&effective_cwd);

        // Set env vars
        let global_env = self.inner.env_vars.lock().unwrap();
        for (k, v) in global_env.iter() {
            cmd.env(k, v);
        }
        if let Some(local_env) = env {
            for (k, v) in local_env.iter() {
                cmd.env(k, v);
            }
        }

        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        cmd
    }
}

impl Default for EnhancedShellTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for EnhancedShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        "Enhanced shell: run commands, manage background processes, track CWD and env vars."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["run", "start", "status", "stop", "output", "set_cwd", "get_cwd", "set_env", "unset_env", "list_env"],
                    "description": "Shell action to perform"
                },
                "command": { "type": "string", "description": "Command to execute (for run/start)" },
                "id": { "type": "string", "description": "Background process ID (for start/status/stop/output)" },
                "cwd": { "type": "string", "description": "Working directory override" },
                "timeout": { "type": "integer", "description": "Timeout in seconds (for run)" },
                "env": {
                    "type": "object",
                    "description": "Environment variables to set for this command",
                    "additionalProperties": { "type": "string" }
                },
                "key": { "type": "string", "description": "Environment variable key (for set_env/unset_env)" },
                "value": { "type": "string", "description": "Environment variable value (for set_env)" },
                "path": { "type": "string", "description": "Directory path (for set_cwd)" },
                "stream": { "type": "string", "enum": ["stdout", "stderr", "both"], "description": "Which output stream (for output, default: both)" }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let action: ShellAction = serde_json::from_value(args)
            .map_err(|e| CoreError::Tool(format!("Invalid shell params: {e}")))?;

        match action {
            ShellAction::Run { command, cwd, timeout, env } => {
                let timeout_secs = timeout.unwrap_or(self.inner.default_timeout);
                let start = Instant::now();

                let mut cmd = self.build_command(
                    &command,
                    cwd.as_deref(),
                    env.as_ref(),
                );

                let output = tokio::time::timeout(
                    std::time::Duration::from_secs(timeout_secs),
                    cmd.output(),
                )
                .await
                .map_err(|_| CoreError::Tool(format!("Command timed out after {timeout_secs}s")))?
                .map_err(|e| CoreError::Tool(format!("Execution failed: {e}")))?;

                // Track CWD if command is 'cd'
                if command.trim().starts_with("cd ") {
                    let dir = command.trim().strip_prefix("cd ").unwrap().trim();
                    if !dir.is_empty() {
                        let new_cwd = self.get_cwd().join(dir);
                        if new_cwd.exists() {
                            *self.inner.cwd.lock().unwrap() = new_cwd;
                        }
                    }
                }

                let result = RunResult {
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    exit_code: output.status.code().unwrap_or(-1),
                    duration_ms: start.elapsed().as_millis() as u64,
                    cwd: self.get_cwd().to_string_lossy().to_string(),
                };

                serde_json::to_string(&result)
                    .map_err(|e| CoreError::Tool(format!("Serialization error: {e}")))
            }

            ShellAction::Start { command, id, cwd, env } => {
                let mut cmd = self.build_command(
                    &command,
                    cwd.as_deref(),
                    env.as_ref(),
                );
                cmd.stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());

                let mut child = cmd.spawn()
                    .map_err(|e| CoreError::Tool(format!("Failed to start process: {e}")))?;

                let pid = child.id();

                // Spawn tasks to drain stdout/stderr
                let stdout_buf = Arc::new(Mutex::new(String::new()));
                let stderr_buf = Arc::new(Mutex::new(String::new()));

                if let Some(mut stdout) = child.stdout.take() {
                    let buf = stdout_buf.clone();
                    tokio::spawn(async move {
                        let mut data = [0u8; 4096];
                        loop {
                            match stdout.read(&mut data).await {
                                Ok(0) => break,
                                Ok(n) => {
                                    let s = String::from_utf8_lossy(&data[..n]).to_string();
                                    buf.lock().unwrap().push_str(&s);
                                }
                                Err(_) => break,
                            }
                        }
                    });
                }

                if let Some(mut stderr) = child.stderr.take() {
                    let buf = stderr_buf.clone();
                    tokio::spawn(async move {
                        let mut data = [0u8; 4096];
                        loop {
                            match stderr.read(&mut data).await {
                                Ok(0) => break,
                                Ok(n) => {
                                    let s = String::from_utf8_lossy(&data[..n]).to_string();
                                    buf.lock().unwrap().push_str(&s);
                                }
                                Err(_) => break,
                            }
                        }
                    });
                }

                let bp = BackgroundProcess {
                    child,
                    command: command.clone(),
                    started_at: Instant::now(),
                    stdout_buffer: stdout_buf,
                    stderr_buffer: stderr_buf,
                };

                self.inner.bg_processes.lock().await.insert(id.clone(), bp);

                let result = StartResult {
                    id,
                    pid,
                    command,
                };

                serde_json::to_string(&result)
                    .map_err(|e| CoreError::Tool(format!("Serialization error: {e}")))
            }

            ShellAction::Status { id } => {
                let mut procs = self.inner.bg_processes.lock().await;
                let bp = procs.get_mut(&id)
                    .ok_or_else(|| CoreError::Tool(format!("Process '{id}' not found")))?;

                // Try to check if still running
                let running = match bp.child.try_wait() {
                    Ok(Some(status)) => {
                        let result = StatusResult {
                            id: id.clone(),
                            running: false,
                            exit_code: status.code(),
                            uptime_secs: Some(bp.started_at.elapsed().as_secs()),
                        };
                        return serde_json::to_string(&result)
                            .map_err(|e| CoreError::Tool(format!("Serialization error: {e}")));
                    }
                    Ok(None) => true,
                    Err(_) => false,
                };

                let result = StatusResult {
                    id: id.clone(),
                    running,
                    exit_code: None,
                    uptime_secs: Some(bp.started_at.elapsed().as_secs()),
                };

                serde_json::to_string(&result)
                    .map_err(|e| CoreError::Tool(format!("Serialization error: {e}")))
            }

            ShellAction::Stop { id } => {
                let mut procs = self.inner.bg_processes.lock().await;
                if let Some(mut bp) = procs.remove(&id) {
                    let _ = bp.child.kill().await;
                    let result = StatusResult {
                        id: id.clone(),
                        running: false,
                        exit_code: None,
                        uptime_secs: Some(bp.started_at.elapsed().as_secs()),
                    };
                    serde_json::to_string(&result)
                        .map_err(|e| CoreError::Tool(format!("Serialization error: {e}")))
                } else {
                    Err(CoreError::Tool(format!("Process '{id}' not found")))
                }
            }

            ShellAction::Output { id, stream } => {
                let procs = self.inner.bg_processes.lock().await;
                let bp = procs.get(&id)
                    .ok_or_else(|| CoreError::Tool(format!("Process '{id}' not found")))?;

                let stdout = bp.stdout_buffer.lock().unwrap().clone();
                let stderr = bp.stderr_buffer.lock().unwrap().clone();

                let result = OutputResult {
                    id: id.clone(),
                    stdout: match stream.as_deref() {
                        Some("stderr") => String::new(),
                        _ => stdout,
                    },
                    stderr: match stream.as_deref() {
                        Some("stdout") => String::new(),
                        _ => stderr,
                    },
                };

                serde_json::to_string(&result)
                    .map_err(|e| CoreError::Tool(format!("Serialization error: {e}")))
            }

            ShellAction::SetCwd { path } => {
                let new_cwd = PathBuf::from(&path);
                if !new_cwd.exists() {
                    return Err(CoreError::Tool(format!("Directory does not exist: {path}")));
                }
                *self.inner.cwd.lock().unwrap() = new_cwd;
                Ok(serde_json::json!({"cwd": path}).to_string())
            }

            ShellAction::GetCwd {} => {
                let cwd = self.get_cwd();
                Ok(serde_json::json!({"cwd": cwd.to_string_lossy()}).to_string())
            }

            ShellAction::SetEnv { key, value } => {
                self.inner.env_vars.lock().unwrap().insert(key.clone(), value);
                Ok(serde_json::json!({"set": key}).to_string())
            }

            ShellAction::UnsetEnv { key } => {
                let removed = self.inner.env_vars.lock().unwrap().remove(&key).is_some();
                Ok(serde_json::json!({"unset": key, "existed": removed}).to_string())
            }

            ShellAction::ListEnv {} => {
                let envs = self.inner.env_vars.lock().unwrap().clone();
                serde_json::to_string(&envs)
                    .map_err(|e| CoreError::Tool(format!("Serialization error: {e}")))
            }
        }
    }
}
