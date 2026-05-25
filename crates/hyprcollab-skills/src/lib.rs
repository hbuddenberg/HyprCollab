//! Skills engine — YAML loader, regex matcher, auto-learner, SQLite store.
//!
//! Implements the F5 S23 sprint item.

pub mod learner;
pub mod loader;
pub mod matcher;
pub mod store;

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── SkillId ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SkillId(pub Uuid);

impl SkillId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SkillId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SkillId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── Skill ─────────────────────────────────────────────────────────────────────

/// A named, regex-triggered skill with associated LLM instructions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: SkillId,
    /// Lowercase, hyphen-separated identifier (max 64 chars).
    pub name: String,
    pub description: String,
    pub category: Option<String>,
    /// Regex patterns that trigger this skill when matched against user input.
    pub trigger_patterns: Vec<String>,
    /// Instructions injected into the system prompt when the skill is active.
    pub instructions: String,
    pub enabled: bool,
    /// Higher priority skills are matched first.
    pub priority: i32,
    pub usage_count: i64,
    /// Fraction of uses that were recorded as successful (0.0–1.0).
    pub success_rate: f64,
    pub created_at: String,
    pub updated_at: String,
}

// ── SkillCreateRequest ────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct SkillCreateRequest {
    pub name: String,
    pub description: String,
    pub category: Option<String>,
    pub trigger_patterns: Vec<String>,
    pub instructions: String,
    pub enabled: Option<bool>,
    pub priority: Option<i32>,
}

// ── SkillUpdateRequest ────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct SkillUpdateRequest {
    pub description: Option<String>,
    pub category: Option<String>,
    pub trigger_patterns: Option<Vec<String>>,
    pub instructions: Option<String>,
    pub enabled: Option<bool>,
    pub priority: Option<i32>,
}

// ── Re-exports ────────────────────────────────────────────────────────────────

pub use learner::{LearnerSuggestion, SkillLearner};
pub use loader::{load_skill_from_yaml, load_skills_from_dir, validate_skill};
pub use matcher::SkillMatcher;
pub use store::SkillStore;
