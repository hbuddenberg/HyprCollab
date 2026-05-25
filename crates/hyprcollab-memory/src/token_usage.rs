//! Token usage tracking — per-call records with summaries and daily aggregates.

use std::collections::HashMap;

use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::types::ChatId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::store::MemoryStore;

// ── TokenUsageId ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TokenUsageId(pub Uuid);

impl TokenUsageId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TokenUsageId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TokenUsageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── TokenUsageRecord ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageRecord {
    pub id: TokenUsageId,
    pub chat_id: Option<ChatId>,
    pub model: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub created_at: String,
}

// ── TokenUsageSummary ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsageSummary {
    pub total_tokens: i64,
    pub total_prompt: i64,
    pub total_completion: i64,
    pub by_model: HashMap<String, ModelUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelUsage {
    pub total_tokens: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub call_count: i64,
}

// ── DailyUsage ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyUsage {
    pub date: String,
    pub total_tokens: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

// ── TokenTracker ──────────────────────────────────────────────────────────────

/// Token usage operations backed by the shared [`MemoryStore`].
pub struct TokenTracker<'a> {
    store: &'a MemoryStore,
}

impl<'a> TokenTracker<'a> {
    pub fn new(store: &'a MemoryStore) -> Self {
        Self { store }
    }

    /// Record a single API call's token usage. `chat_id` may be `None` for
    /// calls not tied to a specific conversation.
    pub fn record_usage(
        &self,
        chat_id: Option<ChatId>,
        model: &str,
        prompt_tokens: i64,
        completion_tokens: i64,
    ) -> Result<TokenUsageRecord> {
        let id = TokenUsageId::new();
        let total_tokens = prompt_tokens + completion_tokens;
        let now = chrono::Utc::now().to_rfc3339();

        let conn = self.store.lock_conn()?;
        conn.execute(
            "INSERT INTO token_usage
                (id, chat_id, model, prompt_tokens, completion_tokens, total_tokens, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                id.to_string(),
                chat_id.map(|c| c.to_string()),
                model,
                prompt_tokens,
                completion_tokens,
                total_tokens,
                now,
            ],
        )
        .map_err(|e| CoreError::Memory(format!("record_usage: {e}")))?;

        Ok(TokenUsageRecord {
            id,
            chat_id,
            model: model.to_string(),
            prompt_tokens,
            completion_tokens,
            total_tokens,
            created_at: now,
        })
    }

    /// All usage records for a single chat, newest first.
    pub fn get_usage_by_chat(&self, chat_id: ChatId) -> Result<Vec<TokenUsageRecord>> {
        let conn = self.store.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, chat_id, model, prompt_tokens, completion_tokens, total_tokens, created_at
                 FROM token_usage WHERE chat_id = ?1 ORDER BY created_at DESC",
            )
            .map_err(|e| CoreError::Memory(format!("get_usage_by_chat prepare: {e}")))?;

        let rows = stmt
            .query_map(rusqlite::params![chat_id.to_string()], usage_from_row)
            .map_err(|e| CoreError::Memory(format!("get_usage_by_chat query: {e}")))?;

        collect_rows(rows, "get_usage_by_chat")
    }

    /// All usage records for a model name, newest first.
    pub fn get_usage_by_model(&self, model: &str) -> Result<Vec<TokenUsageRecord>> {
        let conn = self.store.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, chat_id, model, prompt_tokens, completion_tokens, total_tokens, created_at
                 FROM token_usage WHERE model = ?1 ORDER BY created_at DESC",
            )
            .map_err(|e| CoreError::Memory(format!("get_usage_by_model prepare: {e}")))?;

        let rows = stmt
            .query_map(rusqlite::params![model], usage_from_row)
            .map_err(|e| CoreError::Memory(format!("get_usage_by_model query: {e}")))?;

        collect_rows(rows, "get_usage_by_model")
    }

    /// Aggregate token usage, optionally filtered to records on or after `since`
    /// (RFC 3339 string). Totals are broken down by model.
    pub fn get_usage_summary(&self, since: Option<&str>) -> Result<TokenUsageSummary> {
        let conn = self.store.lock_conn()?;

        type Row = (String, i64, i64, i64, i64);

        let rows: Vec<Row> = if let Some(since_str) = since {
            let mut stmt = conn
                .prepare(
                    "SELECT model,
                            SUM(prompt_tokens), SUM(completion_tokens), SUM(total_tokens),
                            COUNT(*)
                     FROM token_usage WHERE created_at >= ?1
                     GROUP BY model",
                )
                .map_err(|e| CoreError::Memory(format!("get_usage_summary prepare: {e}")))?;

            stmt.query_map(rusqlite::params![since_str], summary_row)
                .map_err(|e| CoreError::Memory(format!("get_usage_summary query: {e}")))?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|e| CoreError::Memory(format!("get_usage_summary rows: {e}")))?
        } else {
            let mut stmt = conn
                .prepare(
                    "SELECT model,
                            SUM(prompt_tokens), SUM(completion_tokens), SUM(total_tokens),
                            COUNT(*)
                     FROM token_usage
                     GROUP BY model",
                )
                .map_err(|e| CoreError::Memory(format!("get_usage_summary prepare: {e}")))?;

            stmt.query_map([], summary_row)
                .map_err(|e| CoreError::Memory(format!("get_usage_summary query: {e}")))?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|e| CoreError::Memory(format!("get_usage_summary rows: {e}")))?
        };

        let mut summary = TokenUsageSummary::default();
        for (model, prompt, completion, total, count) in rows {
            summary.total_tokens += total;
            summary.total_prompt += prompt;
            summary.total_completion += completion;
            let entry = summary.by_model.entry(model).or_default();
            entry.total_tokens += total;
            entry.prompt_tokens += prompt;
            entry.completion_tokens += completion;
            entry.call_count += count;
        }

        Ok(summary)
    }

    /// Daily token usage aggregates for the last `days` days (inclusive).
    pub fn get_daily_usage(&self, days: i64) -> Result<Vec<DailyUsage>> {
        let since = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        let conn = self.store.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT date(created_at) AS day,
                        SUM(total_tokens), SUM(prompt_tokens), SUM(completion_tokens)
                 FROM token_usage
                 WHERE created_at >= ?1
                 GROUP BY day
                 ORDER BY day ASC",
            )
            .map_err(|e| CoreError::Memory(format!("get_daily_usage prepare: {e}")))?;

        let rows = stmt
            .query_map(rusqlite::params![since], |row| {
                Ok(DailyUsage {
                    date: row.get(0)?,
                    total_tokens: row.get::<_, i64>(1).unwrap_or(0),
                    prompt_tokens: row.get::<_, i64>(2).unwrap_or(0),
                    completion_tokens: row.get::<_, i64>(3).unwrap_or(0),
                })
            })
            .map_err(|e| CoreError::Memory(format!("get_daily_usage query: {e}")))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| CoreError::Memory(format!("get_daily_usage row: {e}")))?);
        }
        Ok(result)
    }
}

// ── Row helpers ───────────────────────────────────────────────────────────────

fn usage_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TokenUsageRecord> {
    let id_str: String = row.get(0)?;
    let chat_id_str: Option<String> = row.get(1).unwrap_or(None);
    let model: String = row.get(2)?;
    let prompt_tokens: i64 = row.get(3).unwrap_or(0);
    let completion_tokens: i64 = row.get(4).unwrap_or(0);
    let total_tokens: i64 = row.get(5).unwrap_or(0);
    let created_at: String = row.get(6)?;

    let id = Uuid::parse_str(&id_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::from(e))
    })?;

    let chat_id = chat_id_str
        .as_deref()
        .map(|s| {
            Uuid::parse_str(s).map(ChatId).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::from(e),
                )
            })
        })
        .transpose()?;

    Ok(TokenUsageRecord {
        id: TokenUsageId(id),
        chat_id,
        model,
        prompt_tokens,
        completion_tokens,
        total_tokens,
        created_at,
    })
}

fn summary_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(String, i64, i64, i64, i64)> {
    Ok((
        row.get(0)?,
        row.get::<_, i64>(1).unwrap_or(0),
        row.get::<_, i64>(2).unwrap_or(0),
        row.get::<_, i64>(3).unwrap_or(0),
        row.get::<_, i64>(4).unwrap_or(0),
    ))
}

fn collect_rows(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<TokenUsageRecord>>,
    ctx: &str,
) -> Result<Vec<TokenUsageRecord>> {
    let mut records = Vec::new();
    for row in rows {
        records.push(row.map_err(|e| CoreError::Memory(format!("{ctx} row: {e}")))?);
    }
    Ok(records)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn open_store() -> MemoryStore {
        MemoryStore::open_in_memory().unwrap()
    }

    fn new_chat(store: &MemoryStore) -> ChatId {
        let id = ChatId::new();
        store.create_chat(id, "Test", None, None, None, "gpt-4", false).unwrap();
        id
    }

    #[test]
    fn record_and_retrieve_by_chat() {
        let store = open_store();
        let chat_id = new_chat(&store);
        let tracker = TokenTracker::new(&store);

        let rec = tracker.record_usage(Some(chat_id), "gpt-4", 10, 20).unwrap();
        assert_eq!(rec.prompt_tokens, 10);
        assert_eq!(rec.completion_tokens, 20);
        assert_eq!(rec.total_tokens, 30);
        assert_eq!(rec.model, "gpt-4");

        let records = tracker.get_usage_by_chat(chat_id).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, rec.id);
    }

    #[test]
    fn record_without_chat_id() {
        let store = open_store();
        let tracker = TokenTracker::new(&store);

        let rec = tracker.record_usage(None, "claude-3", 5, 15).unwrap();
        assert!(rec.chat_id.is_none());
        assert_eq!(rec.total_tokens, 20);
    }

    #[test]
    fn get_usage_by_model() {
        let store = open_store();
        let tracker = TokenTracker::new(&store);

        tracker.record_usage(None, "gpt-4", 10, 10).unwrap();
        tracker.record_usage(None, "gpt-4", 5, 5).unwrap();
        tracker.record_usage(None, "claude-3", 20, 30).unwrap();

        let gpt4 = tracker.get_usage_by_model("gpt-4").unwrap();
        assert_eq!(gpt4.len(), 2);
        assert!(gpt4.iter().all(|r| r.model == "gpt-4"));

        let claude = tracker.get_usage_by_model("claude-3").unwrap();
        assert_eq!(claude.len(), 1);
    }

    #[test]
    fn get_usage_by_model_empty() {
        let store = open_store();
        let tracker = TokenTracker::new(&store);
        let results = tracker.get_usage_by_model("nonexistent").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn usage_summary_totals() {
        let store = open_store();
        let tracker = TokenTracker::new(&store);

        tracker.record_usage(None, "gpt-4", 10, 20).unwrap();
        tracker.record_usage(None, "gpt-4", 5, 10).unwrap();
        tracker.record_usage(None, "claude-3", 100, 200).unwrap();

        let summary = tracker.get_usage_summary(None).unwrap();
        assert_eq!(summary.total_tokens, 345);
        assert_eq!(summary.total_prompt, 115);
        assert_eq!(summary.total_completion, 230);
        assert_eq!(summary.by_model.len(), 2);
        assert_eq!(summary.by_model["gpt-4"].call_count, 2);
        assert_eq!(summary.by_model["claude-3"].call_count, 1);
    }

    #[test]
    fn usage_summary_empty() {
        let store = open_store();
        let tracker = TokenTracker::new(&store);
        let summary = tracker.get_usage_summary(None).unwrap();
        assert_eq!(summary.total_tokens, 0);
        assert!(summary.by_model.is_empty());
    }

    #[test]
    fn usage_summary_with_since_filter() {
        let store = open_store();
        let tracker = TokenTracker::new(&store);

        tracker.record_usage(None, "gpt-4", 100, 100).unwrap();

        let far_future = "2099-01-01T00:00:00Z";
        let summary = tracker.get_usage_summary(Some(far_future)).unwrap();
        assert_eq!(summary.total_tokens, 0);

        let past = "2000-01-01T00:00:00Z";
        let summary2 = tracker.get_usage_summary(Some(past)).unwrap();
        assert_eq!(summary2.total_tokens, 200);
    }

    #[test]
    fn daily_usage_returns_grouped_data() {
        let store = open_store();
        let tracker = TokenTracker::new(&store);

        tracker.record_usage(None, "gpt-4", 50, 50).unwrap();
        tracker.record_usage(None, "gpt-4", 30, 20).unwrap();

        let daily = tracker.get_daily_usage(7).unwrap();
        assert_eq!(daily.len(), 1);
        assert_eq!(daily[0].total_tokens, 150);
        assert_eq!(daily[0].prompt_tokens, 80);
    }

    #[test]
    fn daily_usage_empty_for_old_records() {
        let store = open_store();
        let tracker = TokenTracker::new(&store);
        tracker.record_usage(None, "gpt-4", 10, 10).unwrap();

        let daily = tracker.get_daily_usage(0).unwrap();
        assert!(daily.is_empty());
    }
}
