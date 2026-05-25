//! Working memory — persistent facts with FTS5 full-text search.
//!
//! Implements the F5 S21 sprint item: a `facts` table with BM25-ranked
//! full-text search via SQLite FTS5, plus confidence-based pruning.

use std::fmt;
use std::str::FromStr;

use hyprcollab_core::errors::{CoreError, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::store::MemoryStore;

// ── FactId ────────────────────────────────────────────────────────────────────

/// Newtype wrapper for a fact's UUID, mirroring the ChatId / MessageId pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FactId(pub Uuid);

impl FactId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for FactId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for FactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── FactCategory ──────────────────────────────────────────────────────────────

/// The semantic type of a stored fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FactCategory {
    Preference,
    Fact,
    Pattern,
    Correction,
    Environment,
}

impl fmt::Display for FactCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            FactCategory::Preference => "preference",
            FactCategory::Fact => "fact",
            FactCategory::Pattern => "pattern",
            FactCategory::Correction => "correction",
            FactCategory::Environment => "environment",
        };
        write!(f, "{s}")
    }
}

impl FromStr for FactCategory {
    type Err = CoreError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "preference" => Ok(FactCategory::Preference),
            "fact" => Ok(FactCategory::Fact),
            "pattern" => Ok(FactCategory::Pattern),
            "correction" => Ok(FactCategory::Correction),
            "environment" => Ok(FactCategory::Environment),
            _ => Err(CoreError::Memory(format!("unknown fact category: '{s}'"))),
        }
    }
}

// ── Fact ──────────────────────────────────────────────────────────────────────

/// A single persistent fact stored in working memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fact {
    pub id: FactId,
    pub category: FactCategory,
    pub content: String,
    /// Confidence in [0.0, 1.0] — facts below a threshold can be pruned.
    pub confidence: f64,
    /// The chat_id or `"manual"` that created this fact.
    pub source: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub access_count: i64,
}

// ── WorkingMemory ─────────────────────────────────────────────────────────────

/// Working memory layer on top of [`MemoryStore`].
///
/// All methods share the store's `Arc<Mutex<Connection>>` — no extra locking.
pub struct WorkingMemory<'a> {
    store: &'a MemoryStore,
}

impl<'a> WorkingMemory<'a> {
    pub fn new(store: &'a MemoryStore) -> Self {
        Self { store }
    }

    // ── Write ─────────────────────────────────────────────────────────────

    /// Persist a new fact and return it.
    pub fn add_fact(
        &self,
        category: FactCategory,
        content: impl Into<String>,
        confidence: f64,
        source: Option<String>,
    ) -> Result<Fact> {
        let id = FactId::new();
        let content = content.into();
        let now = chrono::Utc::now().to_rfc3339();
        let confidence = confidence.clamp(0.0, 1.0);

        let conn = self.store.lock_conn()?;
        conn.execute(
            "INSERT INTO facts (id, category, content, confidence, source, created_at, updated_at, access_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
            rusqlite::params![
                id.to_string(),
                category.to_string(),
                content,
                confidence,
                source,
                now,
                now,
            ],
        )
        .map_err(|e| CoreError::Memory(format!("add_fact: {e}")))?;

        Ok(Fact { id, category, content, confidence, source, created_at: now.clone(), updated_at: now, access_count: 0 })
    }

    /// Update a fact's content and/or confidence. Returns the updated fact, or
    /// `None` if no row with that id exists.
    pub fn update_fact(
        &self,
        id: FactId,
        content: Option<String>,
        confidence: Option<f64>,
    ) -> Result<Option<Fact>> {
        let now = chrono::Utc::now().to_rfc3339();
        let id_str = id.to_string();

        {
            let conn = self.store.lock_conn()?;
            let affected = match (content.as_deref(), confidence) {
                (Some(c), Some(cf)) => conn.execute(
                    "UPDATE facts SET content = ?1, confidence = ?2, updated_at = ?3 WHERE id = ?4",
                    rusqlite::params![c, cf.clamp(0.0, 1.0), now, id_str],
                ),
                (Some(c), None) => conn.execute(
                    "UPDATE facts SET content = ?1, updated_at = ?2 WHERE id = ?3",
                    rusqlite::params![c, now, id_str],
                ),
                (None, Some(cf)) => conn.execute(
                    "UPDATE facts SET confidence = ?1, updated_at = ?2 WHERE id = ?3",
                    rusqlite::params![cf.clamp(0.0, 1.0), now, id_str],
                ),
                (None, None) => return self.get_fact(id),
            }
            .map_err(|e| CoreError::Memory(format!("update_fact: {e}")))?;

            if affected == 0 {
                return Ok(None);
            }
        }

        self.get_fact(id)
    }

    /// Delete a fact by id. Returns `true` if a row was removed.
    pub fn delete_fact(&self, id: FactId) -> Result<bool> {
        let conn = self.store.lock_conn()?;
        let affected = conn
            .execute(
                "DELETE FROM facts WHERE id = ?1",
                rusqlite::params![id.to_string()],
            )
            .map_err(|e| CoreError::Memory(format!("delete_fact: {e}")))?;
        Ok(affected > 0)
    }

    /// Increment the access counter for a fact (best-effort; ignores missing ids).
    pub fn touch_fact(&self, id: FactId) -> Result<()> {
        let conn = self.store.lock_conn()?;
        conn.execute(
            "UPDATE facts SET access_count = access_count + 1 WHERE id = ?1",
            rusqlite::params![id.to_string()],
        )
        .map_err(|e| CoreError::Memory(format!("touch_fact: {e}")))?;
        Ok(())
    }

    /// Delete all facts with `confidence < min_confidence`. Returns count removed.
    pub fn prune_facts(&self, min_confidence: f64) -> Result<usize> {
        let conn = self.store.lock_conn()?;
        let affected = conn
            .execute(
                "DELETE FROM facts WHERE confidence < ?1",
                rusqlite::params![min_confidence],
            )
            .map_err(|e| CoreError::Memory(format!("prune_facts: {e}")))?;
        Ok(affected)
    }

    // ── Read ──────────────────────────────────────────────────────────────

    /// Retrieve a single fact by id, incrementing its access count.
    pub fn get_fact(&self, id: FactId) -> Result<Option<Fact>> {
        let conn = self.store.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, category, content, confidence, source, created_at, updated_at, access_count
                 FROM facts WHERE id = ?1",
            )
            .map_err(|e| CoreError::Memory(format!("get_fact prepare: {e}")))?;

        let id_str = id.to_string();
        let mut rows = stmt
            .query(rusqlite::params![id_str])
            .map_err(|e| CoreError::Memory(format!("get_fact query: {e}")))?;

        match rows.next().map_err(|e| CoreError::Memory(format!("get_fact next: {e}")))? {
            Some(row) => Ok(Some(fact_from_row(row)?)),
            None => Ok(None),
        }
    }

    /// List facts, optionally filtered by category and capped at `limit` rows.
    /// Results are ordered by `updated_at DESC`.
    pub fn list_facts(
        &self,
        category: Option<FactCategory>,
        limit: Option<usize>,
    ) -> Result<Vec<Fact>> {
        let conn = self.store.lock_conn()?;
        let cap = limit.unwrap_or(1000) as i64;

        let facts = if let Some(cat) = category {
            let mut stmt = conn
                .prepare(
                    "SELECT id, category, content, confidence, source, created_at, updated_at, access_count
                     FROM facts WHERE category = ?1 ORDER BY updated_at DESC LIMIT ?2",
                )
                .map_err(|e| CoreError::Memory(format!("list_facts prepare: {e}")))?;

            stmt.query_map(rusqlite::params![cat.to_string(), cap], |row| {
                fact_from_row(row)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
            })
            .map_err(|e| CoreError::Memory(format!("list_facts query: {e}")))?
            .map(|r| r.map_err(|e| CoreError::Memory(format!("list_facts row: {e}"))))
            .collect::<Result<Vec<_>>>()?
        } else {
            let mut stmt = conn
                .prepare(
                    "SELECT id, category, content, confidence, source, created_at, updated_at, access_count
                     FROM facts ORDER BY updated_at DESC LIMIT ?1",
                )
                .map_err(|e| CoreError::Memory(format!("list_facts prepare: {e}")))?;

            stmt.query_map(rusqlite::params![cap], |row| {
                fact_from_row(row)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
            })
            .map_err(|e| CoreError::Memory(format!("list_facts query: {e}")))?
            .map(|r| r.map_err(|e| CoreError::Memory(format!("list_facts row: {e}"))))
            .collect::<Result<Vec<_>>>()?
        };

        Ok(facts)
    }

    /// Full-text search over fact content and category using FTS5 BM25 ranking.
    pub fn search_facts(&self, query: &str, limit: usize) -> Result<Vec<Fact>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.store.lock_conn()?;
        let cap = limit as i64;

        let mut stmt = conn
            .prepare(
                "SELECT f.id, f.category, f.content, f.confidence, f.source,
                        f.created_at, f.updated_at, f.access_count
                 FROM facts_fts
                 INNER JOIN facts AS f ON f.rowid = facts_fts.rowid
                 WHERE facts_fts MATCH ?1
                 ORDER BY rank
                 LIMIT ?2",
            )
            .map_err(|e| CoreError::Memory(format!("search_facts prepare: {e}")))?;

        let facts = stmt
            .query_map(rusqlite::params![query, cap], |row| {
                fact_from_row(row)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
            })
            .map_err(|e| CoreError::Memory(format!("search_facts query: {e}")))?
            .map(|r| r.map_err(|e| CoreError::Memory(format!("search_facts row: {e}"))))
            .collect::<Result<Vec<_>>>()?;

        Ok(facts)
    }
}

// ── Row helper ────────────────────────────────────────────────────────────────

fn fact_from_row(row: &rusqlite::Row<'_>) -> Result<Fact> {
    let id_str: String =
        row.get(0).map_err(|e| CoreError::Memory(format!("fact id: {e}")))?;
    let category_str: String =
        row.get(1).map_err(|e| CoreError::Memory(format!("fact category: {e}")))?;
    let content: String =
        row.get(2).map_err(|e| CoreError::Memory(format!("fact content: {e}")))?;
    let confidence: f64 =
        row.get(3).map_err(|e| CoreError::Memory(format!("fact confidence: {e}")))?;
    let source: Option<String> = row.get(4).unwrap_or(None);
    let created_at: String =
        row.get(5).map_err(|e| CoreError::Memory(format!("fact created_at: {e}")))?;
    let updated_at: String =
        row.get(6).map_err(|e| CoreError::Memory(format!("fact updated_at: {e}")))?;
    let access_count: i64 =
        row.get(7).map_err(|e| CoreError::Memory(format!("fact access_count: {e}")))?;

    let id = FactId(
        Uuid::parse_str(&id_str)
            .map_err(|e| CoreError::Memory(format!("invalid fact id '{id_str}': {e}")))?,
    );
    let category = category_str.parse::<FactCategory>()?;

    Ok(Fact { id, category, content, confidence, source, created_at, updated_at, access_count })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::MemoryStore;

    fn store() -> MemoryStore {
        MemoryStore::open_in_memory().expect("in-memory db")
    }

    // ── FactCategory ──────────────────────────────────────────────────────

    #[test]
    fn category_roundtrip_display_and_parse() {
        for (cat, s) in [
            (FactCategory::Preference, "preference"),
            (FactCategory::Fact, "fact"),
            (FactCategory::Pattern, "pattern"),
            (FactCategory::Correction, "correction"),
            (FactCategory::Environment, "environment"),
        ] {
            assert_eq!(cat.to_string(), s);
            assert_eq!(s.parse::<FactCategory>().unwrap(), cat);
        }
    }

    #[test]
    fn category_parse_unknown_returns_error() {
        assert!("unknown_cat".parse::<FactCategory>().is_err());
    }

    #[test]
    fn category_serde_roundtrip() {
        let cat = FactCategory::Preference;
        let json = serde_json::to_string(&cat).unwrap();
        assert_eq!(json, r#""preference""#);
        let back: FactCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cat);
    }

    // ── FactId ────────────────────────────────────────────────────────────

    #[test]
    fn fact_id_display_and_roundtrip() {
        let id = FactId::new();
        let s = id.to_string();
        assert_eq!(s.len(), 36); // UUID v4 string length
        let parsed = Uuid::parse_str(&s).unwrap();
        assert_eq!(id.0, parsed);
    }

    // ── add_fact + get_fact ───────────────────────────────────────────────

    #[test]
    fn add_and_get_fact() {
        let store = store();
        let wm = WorkingMemory::new(&store);

        let fact = wm
            .add_fact(FactCategory::Preference, "dark mode", 0.9, Some("manual".into()))
            .unwrap();

        assert_eq!(fact.category, FactCategory::Preference);
        assert_eq!(fact.content, "dark mode");
        assert!((fact.confidence - 0.9).abs() < 1e-9);
        assert_eq!(fact.source.as_deref(), Some("manual"));
        assert_eq!(fact.access_count, 0);

        let fetched = wm.get_fact(fact.id).unwrap().expect("fact should exist");
        assert_eq!(fetched.id, fact.id);
        assert_eq!(fetched.content, "dark mode");
    }

    #[test]
    fn get_fact_missing_returns_none() {
        let store = store();
        let wm = WorkingMemory::new(&store);
        let missing = FactId::new();
        assert!(wm.get_fact(missing).unwrap().is_none());
    }

    #[test]
    fn add_fact_clamps_confidence() {
        let store = store();
        let wm = WorkingMemory::new(&store);

        let over = wm.add_fact(FactCategory::Fact, "x", 1.5, None).unwrap();
        assert!((over.confidence - 1.0).abs() < 1e-9);

        let under = wm.add_fact(FactCategory::Fact, "y", -0.5, None).unwrap();
        assert!((under.confidence - 0.0).abs() < 1e-9);
    }

    // ── list_facts ────────────────────────────────────────────────────────

    #[test]
    fn list_facts_no_filter() {
        let store = store();
        let wm = WorkingMemory::new(&store);

        wm.add_fact(FactCategory::Preference, "vim keybindings", 0.9, None).unwrap();
        wm.add_fact(FactCategory::Fact, "Rust 2024 edition", 1.0, None).unwrap();
        wm.add_fact(FactCategory::Pattern, "snake_case everywhere", 0.8, None).unwrap();

        let all = wm.list_facts(None, None).unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn list_facts_by_category() {
        let store = store();
        let wm = WorkingMemory::new(&store);

        wm.add_fact(FactCategory::Preference, "light theme", 0.7, None).unwrap();
        wm.add_fact(FactCategory::Preference, "spaces not tabs", 0.9, None).unwrap();
        wm.add_fact(FactCategory::Fact, "uses tokio", 1.0, None).unwrap();

        let prefs = wm.list_facts(Some(FactCategory::Preference), None).unwrap();
        assert_eq!(prefs.len(), 2);
        assert!(prefs.iter().all(|f| f.category == FactCategory::Preference));
    }

    #[test]
    fn list_facts_limit() {
        let store = store();
        let wm = WorkingMemory::new(&store);

        for i in 0..5 {
            wm.add_fact(FactCategory::Fact, format!("fact {i}"), 0.8, None).unwrap();
        }

        let limited = wm.list_facts(None, Some(3)).unwrap();
        assert_eq!(limited.len(), 3);
    }

    // ── update_fact ───────────────────────────────────────────────────────

    #[test]
    fn update_fact_content_and_confidence() {
        let store = store();
        let wm = WorkingMemory::new(&store);

        let fact = wm
            .add_fact(FactCategory::Pattern, "camelCase", 0.5, None)
            .unwrap();

        let updated = wm
            .update_fact(fact.id, Some("snake_case".into()), Some(0.95))
            .unwrap()
            .expect("fact should exist");

        assert_eq!(updated.content, "snake_case");
        assert!((updated.confidence - 0.95).abs() < 1e-9);
    }

    #[test]
    fn update_fact_missing_id_returns_none() {
        let store = store();
        let wm = WorkingMemory::new(&store);
        let result = wm.update_fact(FactId::new(), Some("x".into()), None).unwrap();
        assert!(result.is_none());
    }

    // ── delete_fact ───────────────────────────────────────────────────────

    #[test]
    fn delete_fact_removes_it() {
        let store = store();
        let wm = WorkingMemory::new(&store);

        let fact = wm.add_fact(FactCategory::Correction, "fix me", 0.3, None).unwrap();
        assert!(wm.delete_fact(fact.id).unwrap());
        assert!(wm.get_fact(fact.id).unwrap().is_none());
    }

    #[test]
    fn delete_fact_missing_returns_false() {
        let store = store();
        let wm = WorkingMemory::new(&store);
        assert!(!wm.delete_fact(FactId::new()).unwrap());
    }

    // ── touch_fact ────────────────────────────────────────────────────────

    #[test]
    fn touch_fact_increments_count() {
        let store = store();
        let wm = WorkingMemory::new(&store);

        let fact = wm.add_fact(FactCategory::Environment, "linux", 1.0, None).unwrap();
        assert_eq!(fact.access_count, 0);

        wm.touch_fact(fact.id).unwrap();
        wm.touch_fact(fact.id).unwrap();

        let fetched = wm.get_fact(fact.id).unwrap().unwrap();
        assert_eq!(fetched.access_count, 2);
    }

    // ── prune_facts ───────────────────────────────────────────────────────

    #[test]
    fn prune_facts_removes_low_confidence() {
        let store = store();
        let wm = WorkingMemory::new(&store);

        wm.add_fact(FactCategory::Fact, "keep me", 0.8, None).unwrap();
        wm.add_fact(FactCategory::Fact, "borderline", 0.5, None).unwrap();
        wm.add_fact(FactCategory::Fact, "remove me", 0.2, None).unwrap();

        let pruned = wm.prune_facts(0.5).unwrap();
        assert_eq!(pruned, 1);

        let remaining = wm.list_facts(None, None).unwrap();
        assert_eq!(remaining.len(), 2);
        assert!(remaining.iter().all(|f| f.confidence >= 0.5));
    }

    // ── search_facts ──────────────────────────────────────────────────────

    #[test]
    fn search_facts_fts5_basic() {
        let store = store();
        let wm = WorkingMemory::new(&store);

        wm.add_fact(FactCategory::Preference, "user prefers dark theme", 0.9, None).unwrap();
        wm.add_fact(FactCategory::Fact, "project uses Rust 2024", 1.0, None).unwrap();
        wm.add_fact(FactCategory::Pattern, "snake_case naming convention", 0.8, None).unwrap();

        let results = wm.search_facts("dark", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("dark"));
    }

    #[test]
    fn search_facts_empty_query_returns_empty() {
        let store = store();
        let wm = WorkingMemory::new(&store);

        wm.add_fact(FactCategory::Fact, "something", 0.9, None).unwrap();
        let results = wm.search_facts("", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_facts_limit_respected() {
        let store = store();
        let wm = WorkingMemory::new(&store);

        for i in 0..5 {
            wm.add_fact(
                FactCategory::Fact,
                format!("rust programming tip {i}"),
                0.8,
                None,
            )
            .unwrap();
        }

        let results = wm.search_facts("rust", 3).unwrap();
        assert!(results.len() <= 3);
    }

    #[test]
    fn search_facts_no_match_returns_empty() {
        let store = store();
        let wm = WorkingMemory::new(&store);

        wm.add_fact(FactCategory::Fact, "Rust is fast", 0.9, None).unwrap();
        let results = wm.search_facts("python", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_facts_whitespace_only_returns_empty() {
        let store = store();
        let wm = WorkingMemory::new(&store);
        let results = wm.search_facts("   ", 10).unwrap();
        assert!(results.is_empty());
    }
}
