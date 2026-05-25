//! Bookmark store — pin specific messages for quick retrieval.

use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::types::{ChatId, MessageId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::store::MemoryStore;

// ── BookmarkId ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BookmarkId(pub Uuid);

impl BookmarkId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for BookmarkId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for BookmarkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── Bookmark ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: BookmarkId,
    pub chat_id: ChatId,
    pub message_id: MessageId,
    pub label: Option<String>,
    pub created_at: String,
}

// ── BookmarkStore ─────────────────────────────────────────────────────────────

/// Bookmark operations backed by the shared [`MemoryStore`].
pub struct BookmarkStore<'a> {
    store: &'a MemoryStore,
}

impl<'a> BookmarkStore<'a> {
    pub fn new(store: &'a MemoryStore) -> Self {
        Self { store }
    }

    /// Add a bookmark for a message. Returns the new [`Bookmark`].
    pub fn add_bookmark(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        label: Option<String>,
    ) -> Result<Bookmark> {
        let id = BookmarkId::new();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.store.lock_conn()?;
        conn.execute(
            "INSERT INTO bookmarks (id, chat_id, message_id, label, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                id.to_string(),
                chat_id.to_string(),
                message_id.to_string(),
                label,
                now,
            ],
        )
        .map_err(|e| CoreError::Memory(format!("add_bookmark: {e}")))?;

        Ok(Bookmark { id, chat_id, message_id, label, created_at: now })
    }

    /// Remove a bookmark by ID. Returns `true` if it existed.
    pub fn remove_bookmark(&self, id: BookmarkId) -> Result<bool> {
        let conn = self.store.lock_conn()?;
        let rows = conn
            .execute(
                "DELETE FROM bookmarks WHERE id = ?1",
                rusqlite::params![id.to_string()],
            )
            .map_err(|e| CoreError::Memory(format!("remove_bookmark: {e}")))?;
        Ok(rows > 0)
    }

    /// List all bookmarks for a chat, newest first.
    pub fn list_bookmarks(&self, chat_id: ChatId) -> Result<Vec<Bookmark>> {
        let conn = self.store.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, chat_id, message_id, label, created_at
                 FROM bookmarks WHERE chat_id = ?1 ORDER BY created_at DESC",
            )
            .map_err(|e| CoreError::Memory(format!("list_bookmarks prepare: {e}")))?;

        let rows = stmt
            .query_map(rusqlite::params![chat_id.to_string()], bookmark_from_row)
            .map_err(|e| CoreError::Memory(format!("list_bookmarks query: {e}")))?;

        let mut bookmarks = Vec::new();
        for row in rows {
            bookmarks
                .push(row.map_err(|e| CoreError::Memory(format!("list_bookmarks row: {e}")))?);
        }
        Ok(bookmarks)
    }

    /// Get a single bookmark by ID.
    pub fn get_bookmark(&self, id: BookmarkId) -> Result<Option<Bookmark>> {
        let conn = self.store.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, chat_id, message_id, label, created_at
                 FROM bookmarks WHERE id = ?1",
            )
            .map_err(|e| CoreError::Memory(format!("get_bookmark prepare: {e}")))?;

        let mut rows = stmt
            .query(rusqlite::params![id.to_string()])
            .map_err(|e| CoreError::Memory(format!("get_bookmark query: {e}")))?;

        match rows.next().map_err(|e| CoreError::Memory(format!("get_bookmark next: {e}")))? {
            Some(row) => Ok(Some(
                bookmark_from_row(row).map_err(|e| CoreError::Memory(format!("bookmark row: {e}")))?,
            )),
            None => Ok(None),
        }
    }
}

// ── Row helper ────────────────────────────────────────────────────────────────

fn bookmark_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Bookmark> {
    let id_str: String = row.get(0)?;
    let chat_id_str: String = row.get(1)?;
    let msg_id_str: String = row.get(2)?;
    let label: Option<String> = row.get(3)?;
    let created_at: String = row.get(4)?;

    let parse_uuid = |s: &str, col: usize| {
        Uuid::parse_str(s).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(col, rusqlite::types::Type::Text, Box::from(e))
        })
    };

    Ok(Bookmark {
        id: BookmarkId(parse_uuid(&id_str, 0)?),
        chat_id: ChatId(parse_uuid(&chat_id_str, 1)?),
        message_id: MessageId(parse_uuid(&msg_id_str, 2)?),
        label,
        created_at,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use hyprcollab_core::types::{Message, MessageRole};

    fn open_store() -> MemoryStore {
        MemoryStore::open_in_memory().unwrap()
    }

    fn seed_chat_with_message(store: &MemoryStore) -> (ChatId, MessageId) {
        let chat_id = ChatId::new();
        let msg_id = MessageId::new();
        store.create_chat(chat_id, "Test", None, None, None, "gpt-4", false).unwrap();
        let msg = Message {
            id: msg_id,
            chat_id,
            role: MessageRole::User,
            content: "hello".into(),
            tool_calls: vec![],
            artifacts: vec![],
            timestamp: chrono::Utc::now(),
            metadata: serde_json::Value::Null,
            parent_id: None,
        };
        store.add_message(&msg).unwrap();
        (chat_id, msg_id)
    }

    #[test]
    fn add_and_get_bookmark() {
        let store = open_store();
        let (chat_id, msg_id) = seed_chat_with_message(&store);
        let bm = BookmarkStore::new(&store);

        let b = bm.add_bookmark(chat_id, msg_id, Some("important".into())).unwrap();
        assert_eq!(b.chat_id, chat_id);
        assert_eq!(b.message_id, msg_id);
        assert_eq!(b.label.as_deref(), Some("important"));

        let fetched = bm.get_bookmark(b.id).unwrap().unwrap();
        assert_eq!(fetched.id, b.id);
        assert_eq!(fetched.label.as_deref(), Some("important"));
    }

    #[test]
    fn get_unknown_bookmark_returns_none() {
        let store = open_store();
        let bm = BookmarkStore::new(&store);
        assert!(bm.get_bookmark(BookmarkId::new()).unwrap().is_none());
    }

    #[test]
    fn remove_bookmark_deletes_it() {
        let store = open_store();
        let (chat_id, msg_id) = seed_chat_with_message(&store);
        let bm = BookmarkStore::new(&store);

        let b = bm.add_bookmark(chat_id, msg_id, None).unwrap();
        assert!(bm.remove_bookmark(b.id).unwrap());
        assert!(!bm.remove_bookmark(b.id).unwrap()); // second remove returns false
    }

    #[test]
    fn list_bookmarks_returns_per_chat() {
        let store = open_store();
        let (chat_a, msg_a) = seed_chat_with_message(&store);
        let (chat_b, msg_b) = seed_chat_with_message(&store);
        let bm = BookmarkStore::new(&store);

        bm.add_bookmark(chat_a, msg_a, Some("A1".into())).unwrap();
        bm.add_bookmark(chat_a, msg_a, Some("A2".into())).unwrap();
        bm.add_bookmark(chat_b, msg_b, Some("B1".into())).unwrap();

        let a_bms = bm.list_bookmarks(chat_a).unwrap();
        assert_eq!(a_bms.len(), 2);
        assert!(a_bms.iter().all(|b| b.chat_id == chat_a));

        let b_bms = bm.list_bookmarks(chat_b).unwrap();
        assert_eq!(b_bms.len(), 1);
    }

    #[test]
    fn list_bookmarks_empty_chat_returns_empty() {
        let store = open_store();
        let chat_id = ChatId::new();
        store.create_chat(chat_id, "Empty", None, None, None, "gpt-4", false).unwrap();
        let bm = BookmarkStore::new(&store);
        assert!(bm.list_bookmarks(chat_id).unwrap().is_empty());
    }

    #[test]
    fn add_bookmark_without_label() {
        let store = open_store();
        let (chat_id, msg_id) = seed_chat_with_message(&store);
        let bm = BookmarkStore::new(&store);
        let b = bm.add_bookmark(chat_id, msg_id, None).unwrap();
        assert!(b.label.is_none());
    }
}
