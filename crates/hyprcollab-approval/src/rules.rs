use serde::{Deserialize, Serialize};

/// Pattern to match against tool calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RulePattern {
    /// Match any tool with this exact name.
    ToolName(String),
    /// Match tool names matching this regex.
    Regex(String),
    /// Match all tools (wildcard).
    All,
}

/// Action to take when a rule matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalAction {
    /// Auto-approve without asking the user.
    Allow,
    /// Always ask the user for confirmation.
    Deny,
    /// Ask the user, but allow caching the decision.
    Ask,
}

/// A single approval rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRule {
    pub id: String,
    pub pattern: RulePattern,
    pub action: ApprovalAction,
}
