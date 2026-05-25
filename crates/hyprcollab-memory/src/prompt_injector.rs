//! Memory injection for system prompts (F5 S22).
//!
//! Reads facts from working memory, deduplicates similar content, applies a
//! character budget, and appends a structured block to the system prompt.

use std::collections::HashSet;

use hyprcollab_core::types::PersonaId;

use crate::store::MemoryStore;
use crate::working_memory::{Fact, WorkingMemory};

/// Maximum character budget for the injected memory block.
pub const MEMORY_BUDGET: usize = 2000;

// ── PromptInjector ────────────────────────────────────────────────────────────

/// Stateless helper that enriches a system prompt with relevant working-memory facts.
pub struct PromptInjector;

impl PromptInjector {
    /// Read facts from `store`, deduplicate and budget them, then return
    /// `system_prompt` with the memory block appended.
    ///
    /// If there are no facts the prompt is returned unchanged.
    pub fn inject_memory(
        system_prompt: &str,
        _persona_id: Option<PersonaId>,
        store: &MemoryStore,
    ) -> String {
        let wm = WorkingMemory::new(store);
        let facts = wm.list_facts(None, None).unwrap_or_default();

        if facts.is_empty() {
            return system_prompt.to_string();
        }

        let deduped = Self::deduplicate_facts(facts);
        let budgeted = Self::budget_facts(deduped, MEMORY_BUDGET);

        if budgeted.is_empty() {
            return system_prompt.to_string();
        }

        let block = Self::format_fact_block(&budgeted, MEMORY_BUDGET);
        format!("{system_prompt}\n\n{block}")
    }

    /// Format `facts` as a fenced Markdown block.
    ///
    /// Header shows current vs max character usage.
    pub fn format_fact_block(facts: &[Fact], max_chars: usize) -> String {
        let lines: String = facts
            .iter()
            .map(|f| format!("{}: {} ({:.0}%)\n", f.category, f.content, f.confidence * 100.0))
            .collect();

        let char_count = lines.len();
        format!(
            "---\n## Memory (your personal notes) [{char_count} chars / {max_chars} chars]\n{lines}---"
        )
    }

    /// Remove near-duplicate facts (word-overlap Jaccard > 0.80).
    ///
    /// When two facts overlap, the one with higher confidence is kept.
    pub fn deduplicate_facts(facts: Vec<Fact>) -> Vec<Fact> {
        let mut result: Vec<Fact> = Vec::new();

        'outer: for fact in facts {
            for existing in &mut result {
                if word_overlap_jaccard(&fact.content, &existing.content) > 0.80 {
                    if fact.confidence > existing.confidence {
                        *existing = fact;
                    }
                    continue 'outer;
                }
            }
            result.push(fact);
        }

        result
    }

    /// Keep only facts that fit within `max_chars`, prioritising high-confidence
    /// facts.  Sorts by confidence DESC then trims from the low end.
    pub fn budget_facts(mut facts: Vec<Fact>, max_chars: usize) -> Vec<Fact> {
        facts.sort_by(|a, b| {
            b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut used = 0usize;
        let mut result = Vec::new();

        for fact in facts {
            let line_len =
                format!("{}: {} ({:.0}%)\n", fact.category, fact.content, fact.confidence * 100.0)
                    .len();

            if used + line_len <= max_chars {
                used += line_len;
                result.push(fact);
            }
            // Don't break — a later fact might be shorter and still fit.
        }

        result
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Jaccard similarity on lowercase word sets.
fn word_overlap_jaccard(a: &str, b: &str) -> f64 {
    let words_a: HashSet<&str> = a.split_whitespace().collect();
    let words_b: HashSet<&str> = b.split_whitespace().collect();
    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();
    if union == 0 { 1.0 } else { intersection as f64 / union as f64 }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::working_memory::FactCategory;

    fn store_with_facts(facts: &[(&str, &str, f64)]) -> MemoryStore {
        let store = MemoryStore::open_in_memory().unwrap();
        let wm = WorkingMemory::new(&store);
        for (cat, content, conf) in facts {
            let category = cat.parse::<FactCategory>().unwrap();
            wm.add_fact(category, *content, *conf, None).unwrap();
        }
        store
    }

    // ── inject_memory ─────────────────────────────────────────────────────

    #[test]
    fn inject_returns_prompt_unchanged_when_no_facts() {
        let store = MemoryStore::open_in_memory().unwrap();
        let result = PromptInjector::inject_memory("You are helpful.", None, &store);
        assert_eq!(result, "You are helpful.");
    }

    #[test]
    fn inject_appends_memory_block() {
        let store = store_with_facts(&[("preference", "dark mode", 0.9)]);
        let result = PromptInjector::inject_memory("You are helpful.", None, &store);
        assert!(result.contains("You are helpful."));
        assert!(result.contains("## Memory (your personal notes)"));
        assert!(result.contains("preference: dark mode"));
    }

    #[test]
    fn inject_shows_category_confidence_format() {
        let store = store_with_facts(&[("fact", "Rust 2024 edition", 1.0)]);
        let result = PromptInjector::inject_memory("", None, &store);
        assert!(result.contains("fact: Rust 2024 edition (100%)"));
    }

    #[test]
    fn inject_multiple_facts_all_appear() {
        let store = store_with_facts(&[
            ("preference", "dark mode", 0.9),
            ("environment", "Linux x86_64", 1.0),
            ("pattern", "snake_case naming", 0.8),
        ]);
        let result = PromptInjector::inject_memory("base", None, &store);
        assert!(result.contains("preference: dark mode"));
        assert!(result.contains("environment: Linux x86_64"));
        assert!(result.contains("pattern: snake_case naming"));
    }

    // ── deduplicate_facts ─────────────────────────────────────────────────

    #[test]
    fn dedup_removes_near_duplicates() {
        let store = MemoryStore::open_in_memory().unwrap();
        let wm = WorkingMemory::new(&store);
        let f1 = wm.add_fact(FactCategory::Preference, "user likes dark mode", 0.7, None).unwrap();
        let f2 = wm.add_fact(FactCategory::Preference, "user likes dark mode", 0.9, None).unwrap();

        let deduped = PromptInjector::deduplicate_facts(vec![f1, f2]);
        assert_eq!(deduped.len(), 1);
        assert!((deduped[0].confidence - 0.9).abs() < 1e-9);
    }

    #[test]
    fn dedup_keeps_distinct_facts() {
        let store = MemoryStore::open_in_memory().unwrap();
        let wm = WorkingMemory::new(&store);
        let f1 = wm.add_fact(FactCategory::Preference, "dark mode", 0.9, None).unwrap();
        let f2 = wm.add_fact(FactCategory::Fact, "Rust 2024 edition", 0.8, None).unwrap();

        let deduped = PromptInjector::deduplicate_facts(vec![f1, f2]);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn dedup_keeps_higher_confidence_on_overlap() {
        let store = MemoryStore::open_in_memory().unwrap();
        let wm = WorkingMemory::new(&store);
        let low = wm.add_fact(FactCategory::Preference, "user prefers vim", 0.4, None).unwrap();
        let high = wm.add_fact(FactCategory::Preference, "user prefers vim", 0.9, None).unwrap();

        let deduped = PromptInjector::deduplicate_facts(vec![low, high]);
        assert_eq!(deduped.len(), 1);
        assert!((deduped[0].confidence - 0.9).abs() < 1e-9);
    }

    #[test]
    fn dedup_empty_input_returns_empty() {
        assert!(PromptInjector::deduplicate_facts(vec![]).is_empty());
    }

    // ── budget_facts ──────────────────────────────────────────────────────

    #[test]
    fn budget_drops_low_confidence_when_over_limit() {
        let store = MemoryStore::open_in_memory().unwrap();
        let wm = WorkingMemory::new(&store);

        // Create a fact whose single line is > 50 chars to test a tight budget.
        let high = wm
            .add_fact(FactCategory::Fact, "critical high confidence item", 1.0, None)
            .unwrap();
        let low = wm
            .add_fact(FactCategory::Fact, "low confidence throwaway info", 0.1, None)
            .unwrap();

        // Budget of 50 chars — only one should fit.
        let budgeted = PromptInjector::budget_facts(vec![low, high], 50);
        assert_eq!(budgeted.len(), 1);
        assert!((budgeted[0].confidence - 1.0).abs() < 1e-9);
    }

    #[test]
    fn budget_respects_max_chars() {
        let store = MemoryStore::open_in_memory().unwrap();
        let wm = WorkingMemory::new(&store);

        for i in 0..20 {
            wm.add_fact(FactCategory::Fact, format!("fact number {i} content"), 0.8, None).unwrap();
        }

        let facts = wm.list_facts(None, None).unwrap();
        let budgeted = PromptInjector::budget_facts(facts, 100);

        let total: usize = budgeted
            .iter()
            .map(|f| {
                format!("{}: {} ({:.0}%)\n", f.category, f.content, f.confidence * 100.0).len()
            })
            .sum();
        assert!(total <= 100);
    }

    #[test]
    fn budget_empty_input_returns_empty() {
        assert!(PromptInjector::budget_facts(vec![], 2000).is_empty());
    }

    // ── format_fact_block ─────────────────────────────────────────────────

    #[test]
    fn format_includes_header_and_fences() {
        let store = MemoryStore::open_in_memory().unwrap();
        let wm = WorkingMemory::new(&store);
        let fact = wm.add_fact(FactCategory::Preference, "dark mode", 0.9, None).unwrap();
        let block = PromptInjector::format_fact_block(&[fact], 2000);
        assert!(block.starts_with("---\n## Memory (your personal notes)"));
        assert!(block.ends_with("---"));
    }

    #[test]
    fn format_shows_char_budget_in_header() {
        let store = MemoryStore::open_in_memory().unwrap();
        let wm = WorkingMemory::new(&store);
        let fact = wm.add_fact(FactCategory::Fact, "x", 1.0, None).unwrap();
        let block = PromptInjector::format_fact_block(&[fact], 500);
        assert!(block.contains("/ 500 chars"));
    }
}
