//! `artifact_create` agent tool — lets the LLM create artifacts during conversation.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::Tool;

use crate::store::ArtifactStore;
use crate::types::Artifact;

/// Agent tool that creates a new artifact in the given chat.
pub struct ArtifactCreateTool {
    pub store: ArtifactStore,
    pub chat_id: String,
}

#[async_trait]
impl Tool for ArtifactCreateTool {
    fn name(&self) -> &str {
        "artifact_create"
    }

    fn description(&self) -> &str {
        "Create a new artifact (code, markdown, HTML, SVG, Mermaid diagram, LaTeX, React component) in the current conversation"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "type": {
                    "type": "string",
                    "enum": ["code", "markdown", "html", "svg", "mermaid", "react", "latex"],
                    "description": "Artifact variant"
                },
                "content": {
                    "type": "string",
                    "description": "Text content of the artifact"
                },
                "language": {
                    "type": "string",
                    "description": "Programming language (for code artifacts)"
                },
                "filename": {
                    "type": "string",
                    "description": "Optional suggested filename (for code artifacts)"
                },
                "sandboxed": {
                    "type": "boolean",
                    "description": "Whether to sandbox HTML (default: true)"
                }
            },
            "required": ["type", "content"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let artifact_type = args["type"].as_str().unwrap_or("markdown");
        let content = args["content"]
            .as_str()
            .ok_or_else(|| CoreError::Tool("'content' is required".into()))?
            .to_string();

        let artifact = match artifact_type {
            "code" => Artifact::Code {
                language: args["language"]
                    .as_str()
                    .unwrap_or("text")
                    .to_string(),
                content,
                filename: args["filename"].as_str().map(str::to_string),
            },
            "markdown" => Artifact::Markdown { content },
            "html" => Artifact::Html {
                content,
                sandboxed: args["sandboxed"].as_bool().unwrap_or(true),
            },
            "svg" => Artifact::Svg { content },
            "mermaid" => Artifact::Mermaid { content },
            "react" => Artifact::React {
                code: content,
                dependencies: std::collections::HashMap::new(),
            },
            "latex" => Artifact::Latex { content },
            t => {
                return Err(CoreError::Tool(format!("Unknown artifact type: '{t}'")));
            }
        };

        let id = self
            .store
            .create(&self.chat_id, artifact)
            .await
            .map_err(|e| CoreError::Tool(e.to_string()))?;

        Ok(format!("{{\"artifact_id\":\"{id}\",\"status\":\"created\"}}"))
    }
}
