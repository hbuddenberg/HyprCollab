//! Artifact types for HyprCollab.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Opaque identifier for a stored artifact.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ArtifactId(pub String);

impl ArtifactId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for ArtifactId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ArtifactId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Rich artifact type representing all content variants supported by HyprCollab.
///
/// Stored as JSON in SQLite; text content is additionally written to the
/// filesystem at `~/.local/share/hyprcollab/artifacts/{id}.{ext}`.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Artifact {
    Code {
        language: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
    },
    Markdown {
        content: String,
    },
    Html {
        content: String,
        sandboxed: bool,
    },
    Svg {
        content: String,
    },
    Mermaid {
        content: String,
    },
    React {
        code: String,
        #[serde(default)]
        dependencies: HashMap<String, String>,
    },
    Latex {
        content: String,
    },
    Image {
        url: String,
        alt: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        width: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        height: Option<u32>,
    },
    Pdf {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        page_count: Option<u32>,
    },
}

impl Artifact {
    /// Discriminant string matching the `type` tag in JSON.
    pub fn type_name(&self) -> &'static str {
        match self {
            Artifact::Code { .. } => "code",
            Artifact::Markdown { .. } => "markdown",
            Artifact::Html { .. } => "html",
            Artifact::Svg { .. } => "svg",
            Artifact::Mermaid { .. } => "mermaid",
            Artifact::React { .. } => "react",
            Artifact::Latex { .. } => "latex",
            Artifact::Image { .. } => "image",
            Artifact::Pdf { .. } => "pdf",
        }
    }

    /// File extension used when writing content to the filesystem.
    pub fn extension(&self) -> &'static str {
        match self {
            Artifact::Code { language, .. } => match language.to_lowercase().as_str() {
                "rust" | "rs" => "rs",
                "python" | "py" => "py",
                "javascript" | "js" => "js",
                "typescript" | "ts" => "ts",
                "go" => "go",
                "c" => "c",
                "cpp" | "c++" => "cpp",
                "java" => "java",
                "ruby" | "rb" => "rb",
                "shell" | "sh" | "bash" => "sh",
                "sql" => "sql",
                "html" => "html",
                "css" => "css",
                "json" => "json",
                "toml" => "toml",
                "yaml" | "yml" => "yaml",
                _ => "txt",
            },
            Artifact::Markdown { .. } => "md",
            Artifact::Html { .. } => "html",
            Artifact::Svg { .. } => "svg",
            Artifact::Mermaid { .. } => "mmd",
            Artifact::React { .. } => "jsx",
            Artifact::Latex { .. } => "tex",
            // Image and Pdf have no local text content; store metadata as JSON.
            Artifact::Image { .. } | Artifact::Pdf { .. } => "json",
        }
    }

    /// Returns the text content to write to the filesystem, if any.
    pub fn content_str(&self) -> Option<&str> {
        match self {
            Artifact::Code { content, .. } => Some(content.as_str()),
            Artifact::Markdown { content } => Some(content.as_str()),
            Artifact::Html { content, .. } => Some(content.as_str()),
            Artifact::Svg { content } => Some(content.as_str()),
            Artifact::Mermaid { content } => Some(content.as_str()),
            Artifact::React { code, .. } => Some(code.as_str()),
            Artifact::Latex { content } => Some(content.as_str()),
            Artifact::Image { .. } | Artifact::Pdf { .. } => None,
        }
    }
}

/// Lightweight metadata row returned by list operations.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ArtifactMeta {
    pub id: ArtifactId,
    pub chat_id: String,
    pub artifact_type: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
