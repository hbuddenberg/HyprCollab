use crate::rules::{ApprovalAction, ApprovalRule, RulePattern};
use hyprcollab_core::types::ApprovalMode;
use std::collections::HashMap;

/// Engine that decides whether a tool call needs user approval.
pub struct ApprovalEngine {
    rules: Vec<ApprovalRule>,
    mode: ApprovalMode,
    /// Cache of user decisions: (tool_name, args_hash) → allowed.
    cache: HashMap<String, bool>,
}

impl ApprovalEngine {
    pub fn new(mode: ApprovalMode) -> Self {
        Self {
            rules: Vec::new(),
            mode,
            cache: HashMap::new(),
        }
    }

    pub fn with_rules(mut self, rules: Vec<ApprovalRule>) -> Self {
        self.rules = rules;
        self
    }

    pub fn set_mode(&mut self, mode: ApprovalMode) {
        self.mode = mode;
    }

    /// Check whether a tool call requires explicit user approval.
    pub fn needs_approval(&self, tool_name: &str, args: &serde_json::Value) -> bool {
        // Global mode overrides everything.
        match self.mode {
            ApprovalMode::Auto => return false,
            ApprovalMode::Strict => return true,
            ApprovalMode::Normal => {}
        }

        // Check rules in order — first match wins.
        for rule in &self.rules {
            let matches = match &rule.pattern {
                RulePattern::ToolName(name) => tool_name == name,
                RulePattern::Regex(pattern) => regex::Regex::new(pattern)
                    .map(|re| re.is_match(tool_name))
                    .unwrap_or(false),
                RulePattern::All => true,
            };

            if matches {
                return match rule.action {
                    ApprovalAction::Allow => false,
                    ApprovalAction::Deny => true,
                    ApprovalAction::Ask => true,
                };
            }
        }

        // Default in Normal mode: ask for approval.
        true
    }

    /// Cache a user's approval decision for future calls.
    pub fn cache_decision(&mut self, tool_name: &str, _args: &serde_json::Value, allowed: bool) {
        self.cache.insert(tool_name.to_string(), allowed);
    }

    /// Check the cache for a previous decision.
    pub fn check_cache(&self, tool_name: &str) -> Option<bool> {
        self.cache.get(tool_name).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_mode_approves_everything() {
        let engine = ApprovalEngine::new(ApprovalMode::Auto);
        assert!(!engine.needs_approval("dangerous_tool", &serde_json::json!({})));
    }

    #[test]
    fn strict_mode_approves_nothing() {
        let engine = ApprovalEngine::new(ApprovalMode::Strict);
        assert!(engine.needs_approval("safe_tool", &serde_json::json!({})));
    }

    #[test]
    fn normal_mode_asks_by_default() {
        let engine = ApprovalEngine::new(ApprovalMode::Normal);
        assert!(engine.needs_approval("some_tool", &serde_json::json!({})));
    }

    #[test]
    fn rule_allows_specific_tool() {
        let engine = ApprovalEngine::new(ApprovalMode::Normal).with_rules(vec![ApprovalRule {
            id: "r1".into(),
            pattern: RulePattern::ToolName("read_file".into()),
            action: ApprovalAction::Allow,
        }]);
        assert!(!engine.needs_approval("read_file", &serde_json::json!({})));
        assert!(engine.needs_approval("write_file", &serde_json::json!({})));
    }

    #[test]
    fn deny_rule_blocks_specific_tool() {
        let engine = ApprovalEngine::new(ApprovalMode::Normal).with_rules(vec![
            ApprovalRule {
                id: "r1".into(),
                pattern: RulePattern::ToolName("rm_rf".into()),
                action: ApprovalAction::Deny,
            },
            ApprovalRule {
                id: "r2".into(),
                pattern: RulePattern::All,
                action: ApprovalAction::Allow,
            },
        ]);
        assert!(engine.needs_approval("rm_rf", &serde_json::json!({})));
        assert!(!engine.needs_approval("read_file", &serde_json::json!({})));
    }

    #[test]
    fn cache_stores_decisions() {
        let mut engine = ApprovalEngine::new(ApprovalMode::Normal);
        engine.cache_decision("read_file", &serde_json::json!({}), true);
        assert_eq!(engine.check_cache("read_file"), Some(true));
        assert_eq!(engine.check_cache("unknown"), None);
    }

    #[test]
    fn regex_pattern_matches() {
        let engine = ApprovalEngine::new(ApprovalMode::Normal).with_rules(vec![ApprovalRule {
            id: "r1".into(),
            pattern: RulePattern::Regex("web_.*".into()),
            action: ApprovalAction::Allow,
        }]);
        assert!(!engine.needs_approval("web_scrape", &serde_json::json!({})));
        assert!(!engine.needs_approval("web_search", &serde_json::json!({})));
        assert!(engine.needs_approval("file_read", &serde_json::json!({})));
    }
}
