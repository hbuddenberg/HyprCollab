//! Auto-learner: detects repeated patterns in conversation and suggests new skills.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{Skill, SkillId};

// ── LearnerSuggestion ─────────────────────────────────────────────────────────

/// A suggested skill derived from observed conversation patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnerSuggestion {
    pub suggested_name: String,
    pub suggested_patterns: Vec<String>,
    pub suggested_instructions: String,
    /// 0.0–1.0 confidence based on evidence strength.
    pub confidence: f64,
    /// Number of times the pattern was observed.
    pub evidence_count: usize,
}

// ── SkillLearner ──────────────────────────────────────────────────────────────

/// Learns new skill candidates by analysing repeated patterns in messages.
pub struct SkillLearner {
    /// Per-skill usage tracking: (success_count, total_count).
    usage: HashMap<SkillId, (u64, u64)>,
    /// Minimum observations before a pattern qualifies as a suggestion.
    threshold: usize,
}

impl SkillLearner {
    pub fn new() -> Self {
        Self { usage: HashMap::new(), threshold: 3 }
    }

    /// Override the detection threshold (default 3).
    pub fn with_threshold(mut self, threshold: usize) -> Self {
        self.threshold = threshold;
        self
    }

    /// Analyse `messages` for repeated n-gram patterns.
    ///
    /// Returns a suggestion when one or more 2-grams appear ≥ `threshold` times.
    /// The most-frequent qualifying n-gram becomes the skill candidate.
    pub fn detect_patterns(&self, messages: &[String]) -> Option<LearnerSuggestion> {
        let mut freq: HashMap<String, usize> = HashMap::new();

        for msg in messages {
            let words: Vec<&str> = msg
                .split_whitespace()
                .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
                .filter(|w| !w.is_empty())
                .collect();

            // Unigrams
            for w in &words {
                *freq.entry(w.to_lowercase()).or_insert(0) += 1;
            }

            // Bigrams
            for pair in words.windows(2) {
                let bigram = format!("{} {}", pair[0].to_lowercase(), pair[1].to_lowercase());
                *freq.entry(bigram).or_insert(0) += 1;
            }

            // Trigrams
            for triple in words.windows(3) {
                let trigram = format!(
                    "{} {} {}",
                    triple[0].to_lowercase(),
                    triple[1].to_lowercase(),
                    triple[2].to_lowercase()
                );
                *freq.entry(trigram).or_insert(0) += 1;
            }
        }

        // Stop words to skip as standalone patterns.
        let stop: &[&str] = &[
            "the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
            "have", "has", "had", "do", "does", "did", "will", "would", "could",
            "should", "may", "might", "shall", "can", "to", "of", "in", "for",
            "on", "with", "at", "by", "from", "and", "or", "but", "not", "no",
            "i", "me", "my", "we", "you", "your", "it", "its", "this", "that",
            "please", "help", "how", "what", "when", "where", "why", "which",
        ];

        // Prefer higher counts; break ties by longer pattern (bigrams > unigrams).
        let best = freq
            .iter()
            .filter(|&(k, v)| *v >= self.threshold && !stop.contains(&k.as_str()))
            .max_by(|&(ka, va), &(kb, vb)| {
                va.cmp(vb).then_with(|| ka.len().cmp(&kb.len()))
            });

        let (pattern, count) = best?;

        // Slug: replace non-alphanumeric runs with hyphens, strip leading/trailing hyphens.
        let slug = pattern
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");

        if slug.is_empty() {
            return None;
        }

        let confidence = (*count as f64 / messages.len() as f64).min(1.0);

        Some(LearnerSuggestion {
            suggested_name: format!("auto-{slug}"),
            suggested_patterns: vec![pattern.to_string()],
            suggested_instructions: format!(
                "You are an expert assistant for tasks involving \"{pattern}\". \
                 Help the user efficiently and accurately."
            ),
            confidence,
            evidence_count: *count,
        })
    }

    /// Build a full `Skill` from a `LearnerSuggestion`.
    ///
    /// Returns `Err` if the suggestion cannot produce a valid skill (empty name, etc.).
    pub fn generate_skill_from_suggestion(
        &self,
        suggestion: &LearnerSuggestion,
    ) -> Result<Skill, String> {
        if suggestion.suggested_name.is_empty() {
            return Err("suggested_name is empty".into());
        }
        if suggestion.suggested_patterns.is_empty() {
            return Err("suggested_patterns is empty".into());
        }
        if suggestion.suggested_instructions.is_empty() {
            return Err("suggested_instructions is empty".into());
        }

        let now = chrono::Utc::now().to_rfc3339();
        Ok(Skill {
            id: SkillId::new(),
            name: suggestion.suggested_name.clone(),
            description: format!(
                "Auto-generated skill (confidence {:.0}%, {} observations).",
                suggestion.confidence * 100.0,
                suggestion.evidence_count
            ),
            category: Some("auto-learned".into()),
            trigger_patterns: suggestion.suggested_patterns.clone(),
            instructions: suggestion.suggested_instructions.clone(),
            enabled: true,
            priority: 0,
            usage_count: 0,
            success_rate: 0.0,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Record a skill usage event (in-memory only; persisted separately by SkillStore).
    pub fn record_skill_usage(&mut self, skill_id: SkillId, success: bool) {
        let entry = self.usage.entry(skill_id).or_insert((0, 0));
        entry.1 += 1;
        if success {
            entry.0 += 1;
        }
    }

    /// Compute success rate for a skill from in-memory tracking.
    /// Returns `None` if the skill has not been used.
    pub fn success_rate(&self, skill_id: SkillId) -> Option<f64> {
        self.usage.get(&skill_id).map(|&(ok, total)| {
            if total == 0 { 0.0 } else { ok as f64 / total as f64 }
        })
    }
}

impl Default for SkillLearner {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn msgs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // ── detect_patterns ───────────────────────────────────────────────────

    #[test]
    fn detects_repeated_bigram() {
        let learner = SkillLearner::new();
        let messages = msgs(&[
            "docker build this project",
            "docker build failed again",
            "docker build works now",
        ]);
        let suggestion = learner.detect_patterns(&messages).unwrap();
        assert!(suggestion.suggested_patterns[0].contains("docker build"));
        assert!(suggestion.evidence_count >= 3);
    }

    #[test]
    fn returns_none_below_threshold() {
        let learner = SkillLearner::new();
        let messages = msgs(&["review my code", "check this out"]);
        // No pattern appears 3+ times.
        assert!(learner.detect_patterns(&messages).is_none());
    }

    #[test]
    fn custom_threshold_respected() {
        let learner = SkillLearner::new().with_threshold(2);
        let messages = msgs(&["debug this", "debug that"]);
        let suggestion = learner.detect_patterns(&messages);
        assert!(suggestion.is_some());
    }

    #[test]
    fn empty_messages_returns_none() {
        let learner = SkillLearner::new();
        assert!(learner.detect_patterns(&[]).is_none());
    }

    #[test]
    fn stop_words_not_suggested() {
        let learner = SkillLearner::new();
        // "please" + "help" appear many times but are stop words.
        let messages = msgs(&[
            "please help me",
            "please help me again",
            "please help",
            "please help now",
        ]);
        // If a suggestion exists, it must not be solely a stop-word pattern.
        if let Some(s) = learner.detect_patterns(&messages) {
            let p = &s.suggested_patterns[0];
            assert!(
                !["the", "a", "please", "help", "i", "me", "my"].contains(&p.as_str()),
                "stop-word pattern slipped through: {p}"
            );
        }
    }

    #[test]
    fn confidence_bounded_at_one() {
        let learner = SkillLearner::new().with_threshold(1);
        // A single message where "rust" appears once → confidence = 1/1 capped at 1.0.
        let messages = msgs(&["rust programming"]);
        if let Some(s) = learner.detect_patterns(&messages) {
            assert!(s.confidence <= 1.0);
        }
    }

    // ── generate_skill_from_suggestion ────────────────────────────────────

    #[test]
    fn generate_skill_from_valid_suggestion() {
        let learner = SkillLearner::new();
        let suggestion = LearnerSuggestion {
            suggested_name: "auto-docker-build".into(),
            suggested_patterns: vec!["docker build".into()],
            suggested_instructions: "Help with docker build tasks.".into(),
            confidence: 0.9,
            evidence_count: 5,
        };
        let skill = learner.generate_skill_from_suggestion(&suggestion).unwrap();
        assert_eq!(skill.name, "auto-docker-build");
        assert_eq!(skill.category.as_deref(), Some("auto-learned"));
        assert!(skill.enabled);
    }

    #[test]
    fn generate_skill_rejects_empty_name() {
        let learner = SkillLearner::new();
        let suggestion = LearnerSuggestion {
            suggested_name: "".into(),
            suggested_patterns: vec!["docker".into()],
            suggested_instructions: "Help.".into(),
            confidence: 0.5,
            evidence_count: 3,
        };
        assert!(learner.generate_skill_from_suggestion(&suggestion).is_err());
    }

    // ── record_skill_usage / success_rate ─────────────────────────────────

    #[test]
    fn usage_tracking_success_rate() {
        let mut learner = SkillLearner::new();
        let id = SkillId::new();
        learner.record_skill_usage(id, true);
        learner.record_skill_usage(id, true);
        learner.record_skill_usage(id, false);
        let rate = learner.success_rate(id).unwrap();
        assert!((rate - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn unknown_skill_returns_none() {
        let learner = SkillLearner::new();
        assert!(learner.success_rate(SkillId::new()).is_none());
    }
}
