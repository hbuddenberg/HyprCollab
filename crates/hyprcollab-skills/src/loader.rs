//! YAML skill loader and validator.
//!
//! Supports Hermes-style frontmatter format:
//!
//! ```text
//! ---
//! name: my-skill
//! description: Does something useful.
//! category: devops
//! trigger_patterns:
//!   - "docker"
//!   - "container"
//! priority: 10
//! ---
//! Instructions text injected into the system prompt...
//! ```
//!
//! Files without a frontmatter delimiter are parsed as plain YAML where
//! `instructions` is a top-level field.

use std::path::Path;

use regex::Regex;
use serde::Deserialize;

use crate::{Skill, SkillId};

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("I/O error at {path}: {source}")]
    Io { path: String, #[source] source: std::io::Error },

    #[error("YAML parse error in {path}: {source}")]
    Yaml { path: String, #[source] source: serde_yaml::Error },

    #[error("validation failed for skill in {path}: {reason}")]
    Validation { path: String, reason: String },

    #[error("skill validation failed: {0}")]
    ValidationDirect(String),
}

pub type Result<T> = std::result::Result<T, LoadError>;

// ── Frontmatter helper ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    name: String,
    description: String,
    category: Option<String>,
    trigger_patterns: Vec<String>,
    #[serde(default)]
    priority: i32,
    /// Optional inline instructions (used when file has no body section).
    #[serde(default)]
    instructions: Option<String>,
}

/// Split `content` into (frontmatter_yaml, body) if it starts with `---\n`.
fn split_frontmatter(content: &str) -> Option<(&str, &str)> {
    let rest = if let Some(r) = content.strip_prefix("---\n") {
        r
    } else {
        content.strip_prefix("---\r\n")?
    };
    // Find the closing `---`
    if let Some(end) = rest.find("\n---\n") {
        Some((&rest[..end], rest[end + 5..].trim_start()))
    } else if let Some(end) = rest.find("\n---\r\n") {
        Some((&rest[..end], rest[end + 6..].trim_start()))
    } else {
        None
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Load a single skill from a YAML file at `path`.
pub fn load_skill_from_yaml(path: &Path) -> Result<Skill> {
    let path_str = path.display().to_string();

    let content = std::fs::read_to_string(path).map_err(|e| LoadError::Io {
        path: path_str.clone(),
        source: e,
    })?;

    let (frontmatter_str, instructions) = if let Some((fm, body)) = split_frontmatter(&content) {
        (fm, body.to_string())
    } else {
        // Plain YAML — expect an `instructions` field
        let fm: SkillFrontmatter =
            serde_yaml::from_str(&content).map_err(|e| LoadError::Yaml {
                path: path_str.clone(),
                source: e,
            })?;
        let instr = fm.instructions.clone().unwrap_or_default();
        let skill = build_skill(fm, instr);
        validate_skill(&skill)
            .map_err(|e| LoadError::Validation { path: path_str, reason: e.to_string() })?;
        return Ok(skill);
    };

    let fm: SkillFrontmatter =
        serde_yaml::from_str(frontmatter_str).map_err(|e| LoadError::Yaml {
            path: path_str.clone(),
            source: e,
        })?;

    let skill = build_skill(fm, instructions);
    validate_skill(&skill)
        .map_err(|e| LoadError::Validation { path: path_str, reason: e.to_string() })?;
    Ok(skill)
}

fn build_skill(fm: SkillFrontmatter, instructions: String) -> Skill {
    let now = chrono::Utc::now().to_rfc3339();
    Skill {
        id: SkillId::new(),
        name: fm.name,
        description: fm.description,
        category: fm.category,
        trigger_patterns: fm.trigger_patterns,
        instructions,
        enabled: true,
        priority: fm.priority,
        usage_count: 0,
        success_rate: 0.0,
        created_at: now.clone(),
        updated_at: now,
    }
}

/// Recursively load all `*.yaml` / `*.yml` files from `dir`.
pub fn load_skills_from_dir(dir: &Path) -> Result<Vec<Skill>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut skills = Vec::new();
    let entries =
        std::fs::read_dir(dir).map_err(|e| LoadError::Io { path: dir.display().to_string(), source: e })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            skills.extend(load_skills_from_dir(&path)?);
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "yaml" || ext == "yml" {
            match load_skill_from_yaml(&path) {
                Ok(skill) => skills.push(skill),
                Err(e) => tracing::warn!("skipping skill file {}: {e}", path.display()),
            }
        }
    }

    Ok(skills)
}

/// Validate a skill's fields. Returns `Err` with a human-readable reason on failure.
pub fn validate_skill(skill: &Skill) -> std::result::Result<(), LoadError> {
    // name: lowercase, hyphens and digits only, max 64 chars
    if skill.name.is_empty() || skill.name.len() > 64 {
        return Err(LoadError::ValidationDirect(format!(
            "name '{}' must be 1–64 chars",
            skill.name
        )));
    }
    let name_re = Regex::new(r"^[a-z0-9][a-z0-9-]*$").unwrap();
    if !name_re.is_match(&skill.name) {
        return Err(LoadError::ValidationDirect(format!(
            "name '{}' must contain only lowercase letters, digits, and hyphens",
            skill.name
        )));
    }

    // description: required, max 500 chars
    if skill.description.is_empty() {
        return Err(LoadError::ValidationDirect("description is required".into()));
    }
    if skill.description.len() > 500 {
        return Err(LoadError::ValidationDirect("description exceeds 500 chars".into()));
    }

    // trigger_patterns: at least 1, each must be a valid regex
    if skill.trigger_patterns.is_empty() {
        return Err(LoadError::ValidationDirect("at least one trigger_pattern is required".into()));
    }
    for pat in &skill.trigger_patterns {
        Regex::new(pat).map_err(|e| {
            LoadError::ValidationDirect(format!("invalid trigger pattern '{pat}': {e}"))
        })?;
    }

    // instructions: required, max 50 000 chars
    if skill.instructions.is_empty() {
        return Err(LoadError::ValidationDirect("instructions are required".into()));
    }
    if skill.instructions.len() > 50_000 {
        return Err(LoadError::ValidationDirect("instructions exceed 50 000 chars".into()));
    }

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn write_temp_skill(content: &str) -> (tempfile::NamedTempFile, std::path::PathBuf) {
        let mut f = tempfile::Builder::new().suffix(".yaml").tempfile().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        let path = f.path().to_path_buf();
        (f, path)
    }

    fn minimal_skill() -> Skill {
        let now = chrono::Utc::now().to_rfc3339();
        Skill {
            id: SkillId::new(),
            name: "my-skill".into(),
            description: "A test skill.".into(),
            category: None,
            trigger_patterns: vec!["test".into()],
            instructions: "Do the thing.".into(),
            enabled: true,
            priority: 0,
            usage_count: 0,
            success_rate: 0.0,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    // ── validate_skill ────────────────────────────────────────────────────

    #[test]
    fn validate_accepts_good_skill() {
        assert!(validate_skill(&minimal_skill()).is_ok());
    }

    #[test]
    fn validate_rejects_empty_name() {
        let mut s = minimal_skill();
        s.name = "".into();
        assert!(validate_skill(&s).is_err());
    }

    #[test]
    fn validate_rejects_uppercase_name() {
        let mut s = minimal_skill();
        s.name = "MySkill".into();
        assert!(validate_skill(&s).is_err());
    }

    #[test]
    fn validate_rejects_name_too_long() {
        let mut s = minimal_skill();
        s.name = "a".repeat(65);
        assert!(validate_skill(&s).is_err());
    }

    #[test]
    fn validate_rejects_empty_description() {
        let mut s = minimal_skill();
        s.description = "".into();
        assert!(validate_skill(&s).is_err());
    }

    #[test]
    fn validate_rejects_no_trigger_patterns() {
        let mut s = minimal_skill();
        s.trigger_patterns = vec![];
        assert!(validate_skill(&s).is_err());
    }

    #[test]
    fn validate_rejects_invalid_regex_pattern() {
        let mut s = minimal_skill();
        s.trigger_patterns = vec!["[invalid".into()];
        assert!(validate_skill(&s).is_err());
    }

    #[test]
    fn validate_rejects_empty_instructions() {
        let mut s = minimal_skill();
        s.instructions = "".into();
        assert!(validate_skill(&s).is_err());
    }

    // ── load_skill_from_yaml ──────────────────────────────────────────────

    #[test]
    fn load_frontmatter_yaml() {
        let content = "\
---
name: code-review
description: Helps with code reviews.
trigger_patterns:
  - review
  - \"code review\"
priority: 5
---
You are an expert code reviewer.
";
        let (_f, path) = write_temp_skill(content);
        let skill = load_skill_from_yaml(&path).unwrap();
        assert_eq!(skill.name, "code-review");
        assert_eq!(skill.trigger_patterns, vec!["review", "code review"]);
        assert_eq!(skill.priority, 5);
        assert!(skill.instructions.contains("code reviewer"));
    }

    #[test]
    fn load_plain_yaml_with_instructions_field() {
        let content = "\
name: debugger
description: Helps debug issues.
trigger_patterns:
  - debug
instructions: You are a debugging expert.
";
        let (_f, path) = write_temp_skill(content);
        let skill = load_skill_from_yaml(&path).unwrap();
        assert_eq!(skill.name, "debugger");
        assert_eq!(skill.instructions, "You are a debugging expert.");
    }

    #[test]
    fn load_skill_with_category() {
        let content = "\
---
name: docker-helper
description: Docker commands assistant.
category: devops
trigger_patterns:
  - docker
priority: 10
---
Help with Docker commands.
";
        let (_f, path) = write_temp_skill(content);
        let skill = load_skill_from_yaml(&path).unwrap();
        assert_eq!(skill.category.as_deref(), Some("devops"));
        assert_eq!(skill.priority, 10);
    }

    // ── load_skills_from_dir ──────────────────────────────────────────────

    #[test]
    fn load_dir_returns_all_skills() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["alpha", "beta"] {
            let content = format!(
                "---\nname: {name}\ndescription: Skill {name}.\ntrigger_patterns:\n  - {name}\n---\nInstructions for {name}.\n"
            );
            std::fs::write(dir.path().join(format!("{name}.yaml")), content).unwrap();
        }
        let skills = load_skills_from_dir(dir.path()).unwrap();
        assert_eq!(skills.len(), 2);
    }

    #[test]
    fn load_dir_missing_returns_empty() {
        let skills = load_skills_from_dir(std::path::Path::new("/no/such/dir")).unwrap();
        assert!(skills.is_empty());
    }
}
