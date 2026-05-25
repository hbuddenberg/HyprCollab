//! FTS5-based full-text search across messages.

use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::types::{ChatId, Message, MessageId, MessageRole, ToolCall, Artifact};
use serde::{Deserialize, Serialize};

use crate::store::MemoryStore;

// ── SearchResult ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub message: Message,
    pub chat_title: String,
    /// BM25 rank (lower is better from SQLite; we negate so higher = better).
    pub rank: f64,
}

// ── ConversationSearch ────────────────────────────────────────────────────────

/// FTS5-powered search over the messages table.
pub struct ConversationSearch<'a> {
    store: &'a MemoryStore,
}

impl<'a> ConversationSearch<'a> {
    pub fn new(store: &'a MemoryStore) -> Self {
        Self { store }
    }

    /// Search all chats for messages matching `query`.
    ///
    /// Results are sorted by BM25 rank (best match first).
    pub fn search_conversations(
        &self,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<SearchResult>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.store.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT m.id, m.chat_id, m.role, m.content,
                        m.tool_calls, m.artifacts, m.timestamp, m.metadata, m.parent_id,
                        c.title,
                        -rank AS bm25_score
                 FROM messages_fts
                 INNER JOIN messages AS m ON m.rowid = messages_fts.rowid
                 LEFT  JOIN chats    AS c ON c.id    = m.chat_id
                 WHERE messages_fts MATCH ?1
                 ORDER BY rank
                 LIMIT ?2 OFFSET ?3",
            )
            .map_err(|e| CoreError::Memory(format!("search_conversations prepare: {e}")))?;

        let rows = stmt
            .query_map(
                rusqlite::params![query, limit as i64, offset as i64],
                result_from_row,
            )
            .map_err(|e| CoreError::Memory(format!("search_conversations query: {e}")))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(
                row.map_err(|e| CoreError::Memory(format!("search_conversations row: {e}")))?,
            );
        }
        Ok(results)
    }

    /// Search within a single chat.
    pub fn search_in_chat(
        &self,
        chat_id: ChatId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.store.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT m.id, m.chat_id, m.role, m.content,
                        m.tool_calls, m.artifacts, m.timestamp, m.metadata, m.parent_id,
                        c.title,
                        -rank AS bm25_score
                 FROM messages_fts
                 INNER JOIN messages AS m ON m.rowid = messages_fts.rowid
                 LEFT  JOIN chats    AS c ON c.id    = m.chat_id
                 WHERE messages_fts MATCH ?1
                   AND m.chat_id = ?2
                 ORDER BY rank
                 LIMIT ?3",
            )
            .map_err(|e| CoreError::Memory(format!("search_in_chat prepare: {e}")))?;

        let rows = stmt
            .query_map(
                rusqlite::params![query, chat_id.to_string(), limit as i64],
                result_from_row,
            )
            .map_err(|e| CoreError::Memory(format!("search_in_chat query: {e}")))?;

        let mut results = Vec::new();
        for row in rows {
            results
                .push(row.map_err(|e| CoreError::Memory(format!("search_in_chat row: {e}")))?);
        }
        Ok(results)
    }
}

// ── Row helper ────────────────────────────────────────────────────────────────

fn result_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SearchResult> {
    let id_str: String = row.get(0)?;
    let chat_id_str: String = row.get(1)?;
    let role_str: String = row.get(2)?;
    let content: String = row.get(3).unwrap_or_default();
    let tool_calls_json: Option<String> = row.get(4).unwrap_or(None);
    let artifacts_json: Option<String> = row.get(5).unwrap_or(None);
    let timestamp_str: String = row.get(6)?;
    let metadata_json: Option<String> = row.get(7).unwrap_or(None);
    let parent_id_str: Option<String> = row.get(8).unwrap_or(None);
    let chat_title: String = row.get(9).unwrap_or_else(|_| "(unknown)".to_string());
    let rank: f64 = row.get(10).unwrap_or(0.0);

    let role = match role_str.as_str() {
        "user" => MessageRole::User,
        "assistant" => MessageRole::Assistant,
        "system" => MessageRole::System,
        "tool" => MessageRole::Tool,
        other => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                2,
                rusqlite::types::Type::Text,
                Box::from(format!("unknown role: {other}")),
            ));
        }
    };

    let tool_calls: Vec<ToolCall> = tool_calls_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::from(e))
        })?
        .unwrap_or_default();

    let artifacts: Vec<Artifact> = artifacts_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::from(e))
        })?
        .unwrap_or_default();

    let metadata: serde_json::Value = metadata_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::from(e))
        })?
        .unwrap_or(serde_json::Value::Null);

    let timestamp = chrono::DateTime::parse_from_rfc3339(&timestamp_str)
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::from(e))
        })?
        .to_utc();

    let parse_uuid = |s: &str, col: usize| {
        uuid::Uuid::parse_str(s).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(col, rusqlite::types::Type::Text, Box::from(e))
        })
    };

    let parent_id = parent_id_str
        .as_deref()
        .map(|s| parse_uuid(s, 8).map(MessageId))
        .transpose()?;

    Ok(SearchResult {
        message: Message {
            id: MessageId(parse_uuid(&id_str, 0)?),
            chat_id: ChatId(parse_uuid(&chat_id_str, 1)?),
            role,
            content,
            tool_calls,
            artifacts,
            timestamp,
            metadata,
            parent_id,
        },
        chat_title,
        rank,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use hyprcollab_core::types::MessageRole;

    fn open_store() -> MemoryStore {
        MemoryStore::open_in_memory().unwrap()
    }

    fn add_msg(store: &MemoryStore, chat_id: ChatId, role: MessageRole, content: &str) -> MessageId {
        let id = MessageId::new();
        let msg = Message {
            id,
            chat_id,
            role,
            content: content.into(),
            tool_calls: vec![],
            artifacts: vec![],
            timestamp: chrono::Utc::now(),
            metadata: serde_json::Value::Null,
            parent_id: None,
        };
        store.add_message(&msg).unwrap();
        id
    }

    fn seed_chat(store: &MemoryStore, title: &str) -> ChatId {
        let id = ChatId::new();
        store.create_chat(id, title, None, None, None, "gpt-4", false).unwrap();
        id
    }

    #[test]
    fn search_finds_matching_message() {
        let store = open_store();
        let chat = seed_chat(&store, "Rust Chat");
        add_msg(&store, chat, MessageRole::User, "I love Rust programming");
        add_msg(&store, chat, MessageRole::Assistant, "Python is also great");

        let searcher = ConversationSearch::new(&store);
        let results = searcher.search_conversations("Rust", 10, 0).unwrap();
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.message.content.contains("Rust")));
    }

    #[test]
    fn search_returns_empty_for_no_match() {
        let store = open_store();
        let chat = seed_chat(&store, "Chat");
        add_msg(&store, chat, MessageRole::User, "hello world");

        let searcher = ConversationSearch::new(&store);
        let results = searcher.search_conversations("zzzyyyxxx", 10, 0).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_empty_query_returns_empty() {
        let store = open_store();
        let chat = seed_chat(&store, "Chat");
        add_msg(&store, chat, MessageRole::User, "something");

        let searcher = ConversationSearch::new(&store);
        let results = searcher.search_conversations("", 10, 0).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_includes_chat_title() {
        let store = open_store();
        let chat = seed_chat(&store, "My Project Chat");
        add_msg(&store, chat, MessageRole::User, "interesting topic");

        let searcher = ConversationSearch::new(&store);
        let results = searcher.search_conversations("interesting", 10, 0).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].chat_title, "My Project Chat");
    }

    #[test]
    fn search_across_multiple_chats() {
        let store = open_store();
        let chat_a = seed_chat(&store, "Chat A");
        let chat_b = seed_chat(&store, "Chat B");
        add_msg(&store, chat_a, MessageRole::User, "docker build failed");
        add_msg(&store, chat_b, MessageRole::User, "docker compose up");
        add_msg(&store, chat_b, MessageRole::Assistant, "unrelated answer");

        let searcher = ConversationSearch::new(&store);
        let results = searcher.search_conversations("docker", 10, 0).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn search_in_chat_scoped_to_chat() {
        let store = open_store();
        let chat_a = seed_chat(&store, "Chat A");
        let chat_b = seed_chat(&store, "Chat B");
        add_msg(&store, chat_a, MessageRole::User, "target keyword here");
        add_msg(&store, chat_b, MessageRole::User, "target keyword there");

        let searcher = ConversationSearch::new(&store);
        let results = searcher.search_in_chat(chat_a, "target", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].message.chat_id, chat_a);
    }

    #[test]
    fn search_in_chat_empty_query_returns_empty() {
        let store = open_store();
        let chat = seed_chat(&store, "Chat");
        add_msg(&store, chat, MessageRole::User, "content");

        let searcher = ConversationSearch::new(&store);
        let results = searcher.search_in_chat(chat, "   ", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_limit_respected() {
        let store = open_store();
        let chat = seed_chat(&store, "Chat");
        for i in 0..5 {
            add_msg(&store, chat, MessageRole::User, &format!("keyword message {i}"));
        }

        let searcher = ConversationSearch::new(&store);
        let results = searcher.search_conversations("keyword", 3, 0).unwrap();
        assert!(results.len() <= 3);
    }

    #[test]
    fn search_offset_pages_results() {
        let store = open_store();
        let chat = seed_chat(&store, "Chat");
        for i in 0..4 {
            add_msg(&store, chat, MessageRole::User, &format!("paging keyword {i}"));
        }

        let searcher = ConversationSearch::new(&store);
        let page1 = searcher.search_conversations("keyword", 2, 0).unwrap();
        let page2 = searcher.search_conversations("keyword", 2, 2).unwrap();
        assert_eq!(page1.len(), 2);
        assert_eq!(page2.len(), 2);
        // No overlap: message IDs should be distinct.
        let ids1: std::collections::HashSet<_> = page1.iter().map(|r| r.message.id).collect();
        let ids2: std::collections::HashSet<_> = page2.iter().map(|r| r.message.id).collect();
        assert!(ids1.is_disjoint(&ids2));
    }
}
