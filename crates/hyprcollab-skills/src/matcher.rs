//! Regex-based skill matcher with LRU-style pattern cache.

use std::collections::HashMap;
use std::sync::Mutex;

use regex::Regex;

use crate::{Skill, SkillId};

// ── SkillMatcher ──────────────────────────────────────────────────────────────

/// Matches user queries against skill trigger patterns.
///
/// Compiled `Regex` objects are cached by pattern string so each pattern is
/// compiled at most once across all calls.
pub struct SkillMatcher {
    cache: Mutex<HashMap<String, Option<Regex>>>,
}

impl SkillMatcher {
    pub fn new() -> Self {
        Self { cache: Mutex::new(HashMap::new()) }
    }

    /// Return all enabled skills whose trigger patterns match `query`, sorted
    /// by descending priority then descending usage_count.
    pub fn match_skills<'a>(&self, query: &str, skills: &'a [Skill]) -> Vec<&'a Skill> {
        let matched_ids: Vec<SkillId> = skills
            .iter()
            .filter(|s| s.enabled && self.skill_matches(query, s))
            .map(|s| s.id)
            .collect();

        let mut result: Vec<&Skill> = skills
            .iter()
            .filter(|s| matched_ids.contains(&s.id))
            .collect();

        result.sort_by(|a, b| {
            b.priority.cmp(&a.priority).then_with(|| b.usage_count.cmp(&a.usage_count))
        });

        result
    }

    /// Return the single highest-priority matching skill, if any.
    pub fn match_best_skill<'a>(&self, query: &str, skills: &'a [Skill]) -> Option<&'a Skill> {
        self.match_skills(query, skills).into_iter().next()
    }

    /// Append matched skill instructions to `system_prompt`.
    ///
    /// Each skill is formatted as:
    /// ```text
    /// ## Skill: {name}
    /// {instructions}
    /// ```
    pub fn inject_skills(system_prompt: &str, matched_skills: &[&Skill]) -> String {
        if matched_skills.is_empty() {
            return system_prompt.to_string();
        }

        let blocks: String = matched_skills
            .iter()
            .map(|s| format!("## Skill: {}\n{}\n", s.name, s.instructions))
            .collect::<Vec<_>>()
            .join("\n");

        format!("{system_prompt}\n\n{blocks}")
    }

    // ── Private ───────────────────────────────────────────────────────────

    fn skill_matches(&self, query: &str, skill: &Skill) -> bool {
        skill.trigger_patterns.iter().any(|pat| self.pattern_matches(query, pat))
    }

    fn pattern_matches(&self, query: &str, pattern: &str) -> bool {
        let mut cache = self.cache.lock().unwrap_or_else(|p| p.into_inner());
        let re_opt = cache.entry(pattern.to_string()).or_insert_with(|| {
            // Case-insensitive match; (?i) prefix
            Regex::new(&format!("(?i){pattern}")).ok()
        });
        match re_opt {
            Some(re) => re.is_match(query),
            None => false,
        }
    }
}

impl Default for SkillMatcher {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Skill, SkillId};

    fn skill(name: &str, patterns: &[&str], priority: i32, usage_count: i64) -> Skill {
        let now = chrono::Utc::now().to_rfc3339();
        Skill {
            id: SkillId::new(),
            name: name.into(),
            description: "test".into(),
            category: None,
            trigger_patterns: patterns.iter().map(|s| s.to_string()).collect(),
            instructions: format!("Instructions for {name}."),
            enabled: true,
            priority,
            usage_count,
            success_rate: 0.0,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    fn disabled_skill(name: &str, patterns: &[&str]) -> Skill {
        let mut s = skill(name, patterns, 0, 0);
        s.enabled = false;
        s
    }

    // ── match_skills ──────────────────────────────────────────────────────

    #[test]
    fn matches_basic_pattern() {
        let m = SkillMatcher::new();
        let skills = vec![skill("reviewer", &["review"], 0, 0)];
        assert_eq!(m.match_skills("please review my PR", &skills).len(), 1);
    }

    #[test]
    fn no_match_returns_empty() {
        let m = SkillMatcher::new();
        let skills = vec![skill("reviewer", &["review"], 0, 0)];
        assert!(m.match_skills("deploy to production", &skills).is_empty());
    }

    #[test]
    fn disabled_skill_is_excluded() {
        let m = SkillMatcher::new();
        let skills = vec![disabled_skill("reviewer", &["review"])];
        assert!(m.match_skills("please review my code", &skills).is_empty());
    }

    #[test]
    fn sorted_by_priority_descending() {
        let m = SkillMatcher::new();
        let skills = vec![
            skill("low", &["debug"], 1, 0),
            skill("high", &["debug"], 10, 0),
            skill("mid", &["debug"], 5, 0),
        ];
        let matched = m.match_skills("please debug this", &skills);
        assert_eq!(matched.len(), 3);
        assert_eq!(matched[0].name, "high");
        assert_eq!(matched[1].name, "mid");
        assert_eq!(matched[2].name, "low");
    }

    #[test]
    fn tiebreak_by_usage_count() {
        let m = SkillMatcher::new();
        let skills = vec![
            skill("rare", &["help"], 5, 1),
            skill("popular", &["help"], 5, 100),
        ];
        let matched = m.match_skills("help me", &skills);
        assert_eq!(matched[0].name, "popular");
    }

    #[test]
    fn case_insensitive_match() {
        let m = SkillMatcher::new();
        let skills = vec![skill("doc", &["readme"], 0, 0)];
        assert!(!m.match_skills("Generate a README file", &skills).is_empty());
    }

    #[test]
    fn pattern_cache_used_across_calls() {
        let m = SkillMatcher::new();
        let skills = vec![skill("x", &["foo"], 0, 0)];
        // Call twice — second call hits the cache.
        m.match_skills("foo bar", &skills);
        let result = m.match_skills("foo bar", &skills);
        assert_eq!(result.len(), 1);
    }

    // ── match_best_skill ──────────────────────────────────────────────────

    #[test]
    fn best_skill_returns_highest_priority() {
        let m = SkillMatcher::new();
        let skills = vec![
            skill("low", &["fix"], 1, 0),
            skill("high", &["fix"], 20, 0),
        ];
        let best = m.match_best_skill("please fix this bug", &skills).unwrap();
        assert_eq!(best.name, "high");
    }

    #[test]
    fn best_skill_returns_none_on_no_match() {
        let m = SkillMatcher::new();
        let skills = vec![skill("x", &["foo"], 0, 0)];
        assert!(m.match_best_skill("bar baz", &skills).is_none());
    }

    // ── inject_skills ─────────────────────────────────────────────────────

    #[test]
    fn inject_appends_skill_block() {
        let s = skill("my-skill", &["x"], 0, 0);
        let result = SkillMatcher::inject_skills("base prompt", &[&s]);
        assert!(result.starts_with("base prompt"));
        assert!(result.contains("## Skill: my-skill"));
        assert!(result.contains("Instructions for my-skill."));
    }

    #[test]
    fn inject_empty_list_returns_prompt_unchanged() {
        let result = SkillMatcher::inject_skills("base prompt", &[]);
        assert_eq!(result, "base prompt");
    }
}
