use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use hyprcollab_core::ApprovalMode;

/// Agent-level configuration parsed from a markdown file with YAML frontmatter.
///
/// File location: `~/.config/hyprcollab/agents/<name>.md`
///
/// Example file:
/// ```markdown
/// ---
/// name: coder
/// model: anthropic/claude-sonnet-4
/// tools:
///   - fs_read
///   - fs_write
/// mcp_servers:
///   - github
/// approval_mode: auto
/// max_turns: 50
/// ---
///
/// You are a coding assistant specialized in Rust.
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,

    #[serde(default)]
    pub description: String,

    /// System prompt is derived from the body of the markdown file (after frontmatter).
    #[serde(skip)]
    pub system_prompt: String,

    #[serde(default)]
    pub model: Option<String>,

    #[serde(default)]
    pub tools: Vec<String>,

    #[serde(default)]
    pub mcp_servers: Vec<String>,

    #[serde(default)]
    pub approval_mode: Option<ApprovalMode>,

    #[serde(default)]
    pub max_turns: Option<u32>,
}

/// Frontmatter struct used for deserialization before merging with the body.
#[derive(Debug, Deserialize)]
struct Frontmatter {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    tools: Vec<String>,
    #[serde(default)]
    mcp_servers: Vec<String>,
    #[serde(default)]
    approval_mode: Option<ApprovalMode>,
    #[serde(default)]
    max_turns: Option<u32>,
}

impl AgentConfig {
    /// Return the agents directory: `~/.config/hyprcollab/agents/`
    pub fn agents_dir() -> Result<PathBuf> {
        let base = dirs::config_dir().context("cannot determine config directory")?;
        Ok(base.join("hyprcollab").join("agents"))
    }

    /// Load an agent config by name from `~/.config/hyprcollab/agents/<name>.md`.
    pub fn load(name: &str) -> Result<Self> {
        let dir = Self::agents_dir()?;
        let path = dir.join(format!("{name}.md"));
        Self::load_from_file(&path)
    }

    /// Load an agent config from a specific file path.
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading agent {}", path.display()))?;
        Self::parse(&raw)
    }

    /// Parse an agent config from a raw markdown string with YAML frontmatter.
    ///
    /// Frontmatter is delimited by `---` on its own line at the start of the file.
    pub fn parse(raw: &str) -> Result<Self> {
        let raw = crate::global::interpolate_env_vars(raw);

        let (frontmatter_str, body) = split_frontmatter(&raw);

        let fm: Frontmatter = if frontmatter_str.is_empty() {
            return Err(anyhow::anyhow!("agent config missing YAML frontmatter")
                .context("parsing agent config — expected `---` delimiters"));
        } else {
            serde_yaml::from_str(frontmatter_str).with_context(|| "parsing agent frontmatter")?
        };

        let system_prompt = body.trim().to_owned();

        Ok(Self {
            name: fm.name,
            description: fm.description,
            system_prompt,
            model: fm.model,
            tools: fm.tools,
            mcp_servers: fm.mcp_servers,
            approval_mode: fm.approval_mode,
            max_turns: fm.max_turns,
        })
    }
}

/// Split a markdown file into `(frontmatter_yaml, body)`.
/// Returns `("", full_input)` if no frontmatter delimiters are found.
fn split_frontmatter(input: &str) -> (&str, &str) {
    let trimmed = input.trim_start();
    if !trimmed.starts_with("---") {
        return ("", input);
    }

    // Find the closing `---`.
    let after_first = &trimmed[3..];
    // Skip the newline right after opening ---
    let rest = after_first.trim_start_matches(['\n', '\r']);

    if let Some(end) = rest.find("\n---") {
        let fm = &rest[..end];
        let body_start = end + 4; // skip `\n---`
        let body = &rest[body_start..];
        // Strip leading whitespace/newline from body
        let body = body.trim_start_matches(['\n', '\r']);
        return (fm, body);
    }

    ("", input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_agent_with_frontmatter() {
        let raw = r#"---
name: coder
description: A coding assistant
model: anthropic/claude-sonnet-4
tools:
  - fs_read
  - fs_write
approval_mode: auto
max_turns: 50
---

You are a coding assistant specialized in Rust.
"#;
        let config = AgentConfig::parse(raw).unwrap();
        assert_eq!(config.name, "coder");
        assert_eq!(config.description, "A coding assistant");
        assert_eq!(config.model.as_deref(), Some("anthropic/claude-sonnet-4"));
        assert_eq!(config.tools, vec!["fs_read", "fs_write"]);
        assert_eq!(config.approval_mode, Some(ApprovalMode::Auto));
        assert_eq!(config.max_turns, Some(50));
        assert_eq!(
            config.system_prompt,
            "You are a coding assistant specialized in Rust."
        );
    }

    #[test]
    fn parse_agent_without_frontmatter_fails() {
        let raw = "Just a plain markdown file.\nNo frontmatter.\n";
        assert!(AgentConfig::parse(raw).is_err());
    }
}
