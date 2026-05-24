//! File operations tool — read and write files.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::Tool;
use serde::Deserialize;

/// File operations tool: read and write files on the local filesystem.
pub struct FileOpsTool {
    /// Base directory for sandboxing (if set, all paths must be within this dir).
    sandbox_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum FileParams {
    Read { path: String },
    Write { path: String, content: String },
    List { path: String },
    Exists { path: String },
}

impl FileOpsTool {
    pub fn new() -> Self {
        Self { sandbox_dir: None }
    }

    pub fn with_sandbox(mut self, dir: impl Into<String>) -> Self {
        self.sandbox_dir = Some(dir.into());
        self
    }

    /// Validate path is within sandbox if configured.
    fn validate_path(&self, path: &str) -> Result<()> {
        if let Some(ref sandbox) = self.sandbox_dir {
            let canonical = std::path::Path::new(path)
                .canonicalize()
                .map_err(|e| CoreError::Tool(format!("Invalid path {path}: {e}")))?;
            let sandbox_path = std::path::Path::new(sandbox)
                .canonicalize()
                .map_err(|e| CoreError::Tool(format!("Invalid sandbox {sandbox}: {e}")))?;
            if !canonical.starts_with(&sandbox_path) {
                return Err(CoreError::Tool(format!(
                    "Path '{path}' is outside sandbox '{}'",
                    sandbox
                )));
            }
        }
        Ok(())
    }
}

impl Default for FileOpsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for FileOpsTool {
    fn name(&self) -> &str {
        "file_ops"
    }

    fn description(&self) -> &str {
        "Read, write, list, or check existence of files on the local filesystem."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["read", "write", "list", "exists"],
                    "description": "The file operation to perform"
                },
                "path": {
                    "type": "string",
                    "description": "File or directory path"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write (required for write action)"
                }
            },
            "required": ["action", "path"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let params: FileParams = serde_json::from_value(args)
            .map_err(|e| CoreError::Tool(format!("Invalid file_ops params: {e}")))?;

        match params {
            FileParams::Read { path } => {
                self.validate_path(&path)?;
                let content = tokio::fs::read_to_string(&path)
                    .await
                    .map_err(|e| CoreError::Tool(format!("Failed to read {path}: {e}")))?;
                Ok(content)
            }
            FileParams::Write { path, content } => {
                self.validate_path(&path)?;
                // Create parent directories if needed.
                if let Some(parent) = std::path::Path::new(&path).parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|e| CoreError::Tool(format!("Failed to create dirs: {e}")))?;
                }
                tokio::fs::write(&path, &content)
                    .await
                    .map_err(|e| CoreError::Tool(format!("Failed to write {path}: {e}")))?;
                Ok(format!("Successfully wrote {} bytes to {path}", content.len()))
            }
            FileParams::List { path } => {
                self.validate_path(&path)?;
                let mut entries = tokio::fs::read_dir(&path)
                    .await
                    .map_err(|e| CoreError::Tool(format!("Failed to list {path}: {e}")))?;
                let mut files = Vec::new();
                while let Some(entry) = entries
                    .next_entry()
                    .await
                    .map_err(|e| CoreError::Tool(format!("Error reading dir entry: {e}")))?
                {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let is_dir = entry
                        .file_type()
                        .await
                        .map(|ft| ft.is_dir())
                        .unwrap_or(false);
                    files.push(if is_dir { format!("{name}/") } else { name });
                }
                files.sort();
                Ok(files.join("\n"))
            }
            FileParams::Exists { path } => {
                let exists = tokio::fs::metadata(&path).await.is_ok();
                Ok(serde_json::json!({"exists": exists}).to_string())
            }
        }
    }
}
