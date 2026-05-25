//! SQLite-backed memory store with CRUD operations.
//!
//! Provides [`MemoryStore`] — the primary interface for persisting chats, messages,
//! and settings in a local SQLite database.

use std::path::Path;
use std::sync::{Arc, Mutex};

use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::types::*;

use crate::migrations;

// ── Store ──────────────────────────────────────────────────────────────

/// SQLite-backed memory store.
///
/// Wraps the connection in `Arc<Mutex<>>` so the store is `Send + Sync`
/// and can be shared across Axum handlers.
pub struct MemoryStore {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl MemoryStore {
    // ── Lifecycle ──────────────────────────────────────────────────────

    /// Open (or create) a database at the given filesystem path and run migrations.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = rusqlite::Connection::open(path)
            .map_err(|e| CoreError::Memory(format!("failed to open database: {e}")))?;

        // Enable WAL mode and foreign keys.
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| CoreError::Memory(format!("pragma setup failed: {e}")))?;

        migrations::run(&conn)?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    /// Open an in-memory database (useful for tests).
    pub fn open_in_memory() -> Result<Self> {
        let conn = rusqlite::Connection::open_in_memory()
            .map_err(|e| CoreError::Memory(format!("failed to open in-memory db: {e}")))?;

        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(|e| CoreError::Memory(format!("pragma setup failed: {e}")))?;

        migrations::run(&conn)?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    /// Clone the underlying connection `Arc` for sharing with other store layers.
    pub fn connection_arc(&self) -> std::sync::Arc<std::sync::Mutex<rusqlite::Connection>> {
        std::sync::Arc::clone(&self.conn)
    }

    pub(crate) fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, rusqlite::Connection>> {
        self.conn
            .lock()
            .map_err(|_| CoreError::Memory("database mutex poisoned".to_string()))
    }

    // ── Chat CRUD ──────────────────────────────────────────────────────

    /// Insert a new chat record. Returns the same [`ChatId`] for convenience.
    #[allow(clippy::too_many_arguments)]
    pub fn create_chat(
        &self,
        id: ChatId,
        title: &str,
        workspace_id: Option<WorkspaceId>,
        persona_id: Option<PersonaId>,
        agent_role_id: Option<AgentRoleId>,
        model: &str,
        pinned: bool,
    ) -> Result<ChatId> {
        let conn = self.lock_conn()?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
                "INSERT INTO chats (id, title, workspace_id, persona_id, agent_role_id, model, created_at, updated_at, pinned)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    id.to_string(),
                    title,
                    workspace_id.map(|w| w.to_string()),
                    persona_id.map(|p| p.to_string()),
                    agent_role_id.map(|a| a.to_string()),
                    model,
                    now,
                    now,
                    pinned as i64,
                ],
            )
            .map_err(|e| CoreError::Memory(format!("create_chat failed: {e}")))?;
        Ok(id)
    }

    /// Retrieve a single chat by its ID.
    pub fn get_chat(&self, id: ChatId) -> Result<Option<ChatRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, title, workspace_id, persona_id, agent_role_id, model, created_at, updated_at, pinned, last_read_message_id
                 FROM chats WHERE id = ?1",
            )
            .map_err(|e| CoreError::Memory(format!("get_chat prepare: {e}")))?;

        let id_str = id.to_string();
        let mut rows = stmt
            .query(rusqlite::params![id_str])
            .map_err(|e| CoreError::Memory(format!("get_chat query: {e}")))?;

        match rows
            .next()
            .map_err(|e| CoreError::Memory(format!("get_chat next: {e}")))?
        {
            Some(row) => Ok(Some(ChatRecord::from_row(row)?)),
            None => Ok(None),
        }
    }

    /// List all chats, ordered by most recently updated first.
    pub fn list_chats(&self) -> Result<Vec<ChatRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, title, workspace_id, persona_id, agent_role_id, model, created_at, updated_at, pinned, last_read_message_id
                 FROM chats ORDER BY updated_at DESC",
            )
            .map_err(|e| CoreError::Memory(format!("list_chats prepare: {e}")))?;

        let rows = stmt
            .query_map([], |row| {
                ChatRecord::from_row(row)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
            })
            .map_err(|e| CoreError::Memory(format!("list_chats query: {e}")))?;

        let mut chats = Vec::new();
        for row in rows {
            chats.push(row.map_err(|e| CoreError::Memory(format!("list_chats row: {e}")))?);
        }
        Ok(chats)
    }

    /// Delete a chat (cascades to messages and settings).
    pub fn delete_chat(&self, id: ChatId) -> Result<bool> {
        let conn = self.lock_conn()?;
        let affected = conn
            .execute(
                "DELETE FROM chats WHERE id = ?1",
                rusqlite::params![id.to_string()],
            )
            .map_err(|e| CoreError::Memory(format!("delete_chat: {e}")))?;
        Ok(affected > 0)
    }

    /// Pin or unpin a chat. Returns `true` if the chat was found and updated.
    pub fn pin_chat(&self, id: ChatId, pinned: bool) -> Result<bool> {
        let conn = self.lock_conn()?;
        let rows = conn
            .execute(
                "UPDATE chats SET pinned = ?1 WHERE id = ?2",
                rusqlite::params![pinned as i64, id.to_string()],
            )
            .map_err(|e| CoreError::Memory(format!("pin_chat: {e}")))?;
        Ok(rows > 0)
    }

    /// Return all pinned chats ordered by updated_at DESC.
    pub fn list_pinned_chats(&self) -> Result<Vec<ChatRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, title, workspace_id, persona_id, agent_role_id, model, created_at, updated_at, pinned, last_read_message_id
                 FROM chats WHERE pinned = 1 ORDER BY updated_at DESC",
            )
            .map_err(|e| CoreError::Memory(format!("list_pinned_chats prepare: {e}")))?;

        let rows = stmt
            .query_map([], |row| {
                ChatRecord::from_row(row)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
            })
            .map_err(|e| CoreError::Memory(format!("list_pinned_chats query: {e}")))?;

        let mut chats = Vec::new();
        for row in rows {
            chats.push(row.map_err(|e| CoreError::Memory(format!("list_pinned_chats row: {e}")))?);
        }
        Ok(chats)
    }

    // ── Session persistence ────────────────────────────────────────────────

    /// Mark a chat as read up to `message_id`. Returns `true` if the chat was found.
    pub fn mark_chat_read(&self, chat_id: ChatId, message_id: MessageId) -> Result<bool> {
        let conn = self.lock_conn()?;
        let rows = conn
            .execute(
                "UPDATE chats SET last_read_message_id = ?1 WHERE id = ?2",
                rusqlite::params![message_id.to_string(), chat_id.to_string()],
            )
            .map_err(|e| CoreError::Memory(format!("mark_chat_read: {e}")))?;
        Ok(rows > 0)
    }

    /// Get a chat together with the count of messages newer than `last_read_message_id`.
    /// Returns `None` if the chat does not exist.
    pub fn get_chat_with_unread(
        &self,
        chat_id: ChatId,
    ) -> Result<Option<(ChatRecord, usize)>> {
        let chat = match self.get_chat(chat_id)? {
            Some(c) => c,
            None => return Ok(None),
        };

        let conn = self.lock_conn()?;
        let unread: i64 = if let Some(last_read_id) = chat.last_read_message_id {
            conn.query_row(
                "SELECT COUNT(*) FROM messages
                 WHERE chat_id = ?1
                   AND timestamp > (SELECT timestamp FROM messages WHERE id = ?2)",
                rusqlite::params![chat_id.to_string(), last_read_id.to_string()],
                |row| row.get(0),
            )
            .unwrap_or(0)
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM messages WHERE chat_id = ?1",
                rusqlite::params![chat_id.to_string()],
                |row| row.get(0),
            )
            .unwrap_or(0)
        };

        Ok(Some((chat, unread as usize)))
    }

    // ── Message CRUD ───────────────────────────────────────────────────

    /// Add a message to a chat.
    pub fn add_message(&self, msg: &Message) -> Result<()> {
        let tool_calls_json = if msg.tool_calls.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&msg.tool_calls)?)
        };
        let artifacts_json = if msg.artifacts.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&msg.artifacts)?)
        };
        let metadata_json = if msg.metadata.is_null() {
            None
        } else {
            Some(serde_json::to_string(&msg.metadata)?)
        };

        let conn = self.lock_conn()?;
        conn.execute(
                "INSERT INTO messages (id, chat_id, role, content, tool_calls, artifacts, timestamp, metadata, parent_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    msg.id.to_string(),
                    msg.chat_id.to_string(),
                    msg.role.to_string(),
                    msg.content,
                    tool_calls_json,
                    artifacts_json,
                    msg.timestamp.to_rfc3339(),
                    metadata_json,
                    msg.parent_id.map(|p| p.to_string()),
                ],
            )
            .map_err(|e| CoreError::Memory(format!("add_message failed: {e}")))?;

        // Bump the chat's updated_at.
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
                "UPDATE chats SET updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now, msg.chat_id.to_string()],
            )
            .map_err(|e| CoreError::Memory(format!("update chat timestamp: {e}")))?;

        Ok(())
    }

    /// Get all messages for a chat, ordered chronologically.
    pub fn get_messages(&self, chat_id: ChatId) -> Result<Vec<Message>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, chat_id, role, content, tool_calls, artifacts, timestamp, metadata, parent_id
                 FROM messages WHERE chat_id = ?1 ORDER BY timestamp ASC",
            )
            .map_err(|e| CoreError::Memory(format!("get_messages prepare: {e}")))?;

        let chat_id_str = chat_id.to_string();
        let rows = stmt
            .query_map(rusqlite::params![chat_id_str], message_from_row)
            .map_err(|e| CoreError::Memory(format!("get_messages query: {e}")))?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row.map_err(|e| CoreError::Memory(format!("get_messages row: {e}")))?);
        }
        Ok(messages)
    }

    /// Get the *N* most recent messages for a chat.
    pub fn get_recent(&self, chat_id: ChatId, limit: usize) -> Result<Vec<Message>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, chat_id, role, content, tool_calls, artifacts, timestamp, metadata, parent_id
                 FROM messages WHERE chat_id = ?1 ORDER BY timestamp DESC LIMIT ?2",
            )
            .map_err(|e| CoreError::Memory(format!("get_recent prepare: {e}")))?;

        let chat_id_str = chat_id.to_string();
        let rows = stmt
            .query_map(
                rusqlite::params![chat_id_str, limit as i64],
                message_from_row,
            )
            .map_err(|e| CoreError::Memory(format!("get_recent query: {e}")))?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row.map_err(|e| CoreError::Memory(format!("get_recent row: {e}")))?);
        }
        // Reverse so they're in chronological order.
        messages.reverse();
        Ok(messages)
    }

    // ── Conversation Branching ─────────────────────────────────────────

    /// Fork a chat at a given message: create a new chat with all messages up to and
    /// including `parent_msg_id`, preserving content and roles.
    ///
    /// Returns the ID of the newly created chat.
    pub fn fork_from_message(
        &self,
        original_chat_id: ChatId,
        parent_msg_id: MessageId,
    ) -> Result<ChatId> {
        // Load original chat to clone title/model.
        let original = self
            .get_chat(original_chat_id)?
            .ok_or_else(|| CoreError::Memory(format!("chat '{original_chat_id}' not found")))?;

        // Get all messages in the original chat chronologically, up to the fork point.
        let all_msgs = self.get_messages(original_chat_id)?;
        let fork_pos = all_msgs.iter().position(|m| m.id == parent_msg_id).ok_or_else(|| {
            CoreError::Memory(format!("message '{parent_msg_id}' not found in chat"))
        })?;
        let msgs_to_copy = &all_msgs[..=fork_pos];

        // Create the new chat.
        let new_chat_id = ChatId::new();
        self.create_chat(
            new_chat_id,
            &format!("{} (fork)", original.title),
            original.workspace_id,
            original.persona_id,
            original.agent_role_id,
            &original.model,
            false,
        )?;

        // Copy messages with fresh IDs into the new chat.
        for orig_msg in msgs_to_copy {
            let new_msg = Message {
                id: MessageId::new(),
                chat_id: new_chat_id,
                role: orig_msg.role,
                content: orig_msg.content.clone(),
                tool_calls: orig_msg.tool_calls.clone(),
                artifacts: orig_msg.artifacts.clone(),
                timestamp: orig_msg.timestamp,
                metadata: orig_msg.metadata.clone(),
                parent_id: None,
            };
            self.add_message(&new_msg)?;
        }

        Ok(new_chat_id)
    }

    /// Return direct children of a message (messages whose parent_id == msg_id).
    pub fn get_message_children(&self, msg_id: MessageId) -> Result<Vec<Message>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, chat_id, role, content, tool_calls, artifacts, timestamp, metadata, parent_id
                 FROM messages WHERE parent_id = ?1 ORDER BY timestamp ASC",
            )
            .map_err(|e| CoreError::Memory(format!("get_message_children prepare: {e}")))?;

        let rows = stmt
            .query_map(rusqlite::params![msg_id.to_string()], message_from_row)
            .map_err(|e| CoreError::Memory(format!("get_message_children query: {e}")))?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(
                row.map_err(|e| CoreError::Memory(format!("get_message_children row: {e}")))?,
            );
        }
        Ok(messages)
    }

    /// Build the full message tree for a chat.
    ///
    /// The tree's root nodes are messages with no parent (or a parent outside this chat).
    /// Children are nested recursively.
    pub fn get_message_tree(&self, chat_id: ChatId) -> Result<Vec<MessageTreeNode>> {
        let all_msgs = self.get_messages(chat_id)?;
        Ok(build_tree(&all_msgs, None))
    }

    // ── Chat Settings ──────────────────────────────────────────────────

    /// Get a setting value for a chat.
    pub fn get_setting(&self, chat_id: ChatId, key: &str) -> Result<Option<String>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn
            .prepare("SELECT value FROM chat_settings WHERE chat_id = ?1 AND key = ?2")
            .map_err(|e| CoreError::Memory(format!("get_setting prepare: {e}")))?;

        let chat_id_str = chat_id.to_string();
        let mut rows = stmt
            .query(rusqlite::params![chat_id_str, key])
            .map_err(|e| CoreError::Memory(format!("get_setting query: {e}")))?;

        match rows
            .next()
            .map_err(|e| CoreError::Memory(format!("get_setting next: {e}")))?
        {
            Some(row) => {
                Ok(Some(row.get(0).map_err(|e| {
                    CoreError::Memory(format!("get_setting get: {e}"))
                })?))
            }
            None => Ok(None),
        }
    }

    /// Set (upsert) a setting value for a chat.
    pub fn set_setting(&self, chat_id: ChatId, key: &str, value: &str) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
                "INSERT INTO chat_settings (chat_id, key, value) VALUES (?1, ?2, ?3)
                 ON CONFLICT(chat_id, key) DO UPDATE SET value = excluded.value",
                rusqlite::params![chat_id.to_string(), key, value],
            )
            .map_err(|e| CoreError::Memory(format!("set_setting: {e}")))?;
        Ok(())
    }
}

// ── Helper types ───────────────────────────────────────────────────────

/// A flat representation of a chat row from the database.
#[derive(Debug, Clone)]
pub struct ChatRecord {
    pub id: ChatId,
    pub title: String,
    pub workspace_id: Option<WorkspaceId>,
    pub persona_id: Option<PersonaId>,
    pub agent_role_id: Option<AgentRoleId>,
    pub model: String,
    pub created_at: String,
    pub updated_at: String,
    pub pinned: bool,
    pub last_read_message_id: Option<MessageId>,
}

impl ChatRecord {
    fn from_row(row: &rusqlite::Row<'_>) -> Result<Self> {
        let id_str: String = row
            .get(0)
            .map_err(|e| CoreError::Memory(format!("chat id: {e}")))?;
        let title: String = row.get(1).unwrap_or_default();
        let ws: Option<String> = row.get(2).unwrap_or(None);
        let pid: Option<String> = row.get(3).unwrap_or(None);
        let arid: Option<String> = row.get(4).unwrap_or(None);
        let model: String = row.get(5).unwrap_or_default();
        let created_at: String = row.get(6).unwrap_or_default();
        let updated_at: String = row.get(7).unwrap_or_default();
        let pinned_int: i64 = row.get(8).unwrap_or(0);
        let last_read_str: Option<String> = row.get(9).unwrap_or(None);

        Ok(Self {
            id: ChatId(
                uuid::Uuid::parse_str(&id_str)
                    .map_err(|e| CoreError::Memory(format!("invalid chat id '{id_str}': {e}")))?,
            ),
            title,
            workspace_id: ws
                .as_deref()
                .map(|s| {
                    uuid::Uuid::parse_str(s)
                        .map(WorkspaceId)
                        .map_err(|e| CoreError::Memory(format!("invalid workspace_id '{s}': {e}")))
                })
                .transpose()?,
            persona_id: pid
                .as_deref()
                .map(|s| {
                    uuid::Uuid::parse_str(s)
                        .map(PersonaId)
                        .map_err(|e| CoreError::Memory(format!("invalid persona_id '{s}': {e}")))
                })
                .transpose()?,
            agent_role_id: arid
                .as_deref()
                .map(|s| {
                    uuid::Uuid::parse_str(s)
                        .map(AgentRoleId)
                        .map_err(|e| CoreError::Memory(format!("invalid agent_role_id '{s}': {e}")))
                })
                .transpose()?,
            model,
            created_at,
            updated_at,
            pinned: pinned_int != 0,
            last_read_message_id: last_read_str
                .as_deref()
                .map(|s| {
                    uuid::Uuid::parse_str(s)
                        .map(MessageId)
                        .map_err(|e| {
                            CoreError::Memory(format!("invalid last_read_message_id '{s}': {e}"))
                        })
                })
                .transpose()?,
        })
    }
}

/// A node in a message tree (for branching conversations).
#[derive(Debug, Clone)]
pub struct MessageTreeNode {
    pub message: Message,
    pub children: Vec<MessageTreeNode>,
}

// ── Row mapping helper for Message ─────────────────────────────────────

/// Parse a SQLite row into a [`Message`]. Used as a `query_map` callback.
///
/// Column order: id, chat_id, role, content, tool_calls, artifacts, timestamp, metadata, parent_id
fn message_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    let id_str: String = row.get(0)?;
    let chat_id_str: String = row.get(1)?;
    let role_str: String = row.get(2)?;
    let content: String = row.get(3).unwrap_or_default();
    let tool_calls_json: Option<String> = row.get(4).unwrap_or(None);
    let artifacts_json: Option<String> = row.get(5).unwrap_or(None);
    let timestamp_str: String = row.get(6)?;
    let metadata_json: Option<String> = row.get(7).unwrap_or(None);
    let parent_id_str: Option<String> = row.get(8).unwrap_or(None);

    let role = match role_str.as_str() {
        "user" => MessageRole::User,
        "assistant" => MessageRole::Assistant,
        "system" => MessageRole::System,
        "tool" => MessageRole::Tool,
        _ => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                2,
                rusqlite::types::Type::Text,
                Box::from(format!("unknown message role: {role_str}")),
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

    let parent_id = parent_id_str
        .as_deref()
        .map(|s| {
            uuid::Uuid::parse_str(s).map(MessageId).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    8,
                    rusqlite::types::Type::Text,
                    Box::from(e),
                )
            })
        })
        .transpose()?;

    Ok(Message {
        id: MessageId(uuid::Uuid::parse_str(&id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::from(e))
        })?),
        chat_id: ChatId(uuid::Uuid::parse_str(&chat_id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::from(e))
        })?),
        role,
        content,
        tool_calls,
        artifacts,
        timestamp,
        metadata,
        parent_id,
    })
}

// ── Tree builder ───────────────────────────────────────────────────────

fn build_tree(all: &[Message], parent: Option<MessageId>) -> Vec<MessageTreeNode> {
    all.iter()
        .filter(|m| m.parent_id == parent)
        .map(|m| MessageTreeNode {
            message: m.clone(),
            children: build_tree(all, Some(m.id)),
        })
        .collect()
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_message(chat_id: ChatId, role: MessageRole, content: &str) -> Message {
        Message {
            id: MessageId::new(),
            chat_id,
            role,
            content: content.to_string(),
            tool_calls: Vec::new(),
            artifacts: Vec::new(),
            timestamp: chrono::Utc::now(),
            metadata: serde_json::Value::Null,
            parent_id: None,
        }
    }

    fn make_message_with_parent(
        chat_id: ChatId,
        role: MessageRole,
        content: &str,
        parent: MessageId,
    ) -> Message {
        Message { parent_id: Some(parent), ..make_message(chat_id, role, content) }
    }

    #[test]
    fn open_in_memory_works() {
        let store = MemoryStore::open_in_memory().expect("open");
        drop(store);
    }

    #[test]
    fn create_and_get_chat() {
        let store = MemoryStore::open_in_memory().unwrap();
        let id = ChatId::new();
        store
            .create_chat(id, "Test Chat", None, None, None, "gpt-4", false)
            .unwrap();

        let chat = store.get_chat(id).unwrap().expect("chat should exist");
        assert_eq!(chat.id, id);
        assert_eq!(chat.title, "Test Chat");
        assert_eq!(chat.model, "gpt-4");
        assert!(!chat.pinned);
    }

    #[test]
    fn list_chats_returns_created() {
        let store = MemoryStore::open_in_memory().unwrap();
        let id1 = ChatId::new();
        let id2 = ChatId::new();
        store
            .create_chat(id1, "Chat 1", None, None, None, "gpt-4", false)
            .unwrap();
        store
            .create_chat(id2, "Chat 2", None, None, None, "claude-3", false)
            .unwrap();

        let chats = store.list_chats().unwrap();
        assert_eq!(chats.len(), 2);
    }

    #[test]
    fn delete_chat_removes_it() {
        let store = MemoryStore::open_in_memory().unwrap();
        let id = ChatId::new();
        store
            .create_chat(id, "Bye", None, None, None, "gpt-4", false)
            .unwrap();

        assert!(store.delete_chat(id).unwrap());
        assert!(store.get_chat(id).unwrap().is_none());
    }

    #[test]
    fn add_and_get_messages() {
        let store = MemoryStore::open_in_memory().unwrap();
        let chat_id = ChatId::new();
        store
            .create_chat(chat_id, "Msg Test", None, None, None, "gpt-4", false)
            .unwrap();

        let msg1 = make_message(chat_id, MessageRole::User, "Hello");
        let msg2 = make_message(chat_id, MessageRole::Assistant, "Hi there!");
        store.add_message(&msg1).unwrap();
        store.add_message(&msg2).unwrap();

        let messages = store.get_messages(chat_id).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "Hello");
        assert_eq!(messages[1].content, "Hi there!");
    }

    #[test]
    fn get_recent_limits_results() {
        let store = MemoryStore::open_in_memory().unwrap();
        let chat_id = ChatId::new();
        store
            .create_chat(chat_id, "Recent", None, None, None, "gpt-4", false)
            .unwrap();

        for i in 0..5 {
            let msg = make_message(chat_id, MessageRole::User, &format!("msg {i}"));
            store.add_message(&msg).unwrap();
        }

        let recent = store.get_recent(chat_id, 3).unwrap();
        assert_eq!(recent.len(), 3);
        // Should be the last 3 in chronological order.
        assert_eq!(recent[0].content, "msg 2");
        assert_eq!(recent[2].content, "msg 4");
    }

    #[test]
    fn chat_settings_crud() {
        let store = MemoryStore::open_in_memory().unwrap();
        let chat_id = ChatId::new();
        store
            .create_chat(chat_id, "Settings", None, None, None, "gpt-4", false)
            .unwrap();

        // Initially absent.
        assert!(store.get_setting(chat_id, "temperature").unwrap().is_none());

        // Set and retrieve.
        store.set_setting(chat_id, "temperature", "0.7").unwrap();
        assert_eq!(
            store.get_setting(chat_id, "temperature").unwrap(),
            Some("0.7".to_string())
        );

        // Update existing.
        store.set_setting(chat_id, "temperature", "0.9").unwrap();
        assert_eq!(
            store.get_setting(chat_id, "temperature").unwrap(),
            Some("0.9".to_string())
        );
    }

    #[test]
    fn message_with_tool_calls_and_artifacts() {
        let store = MemoryStore::open_in_memory().unwrap();
        let chat_id = ChatId::new();
        store
            .create_chat(chat_id, "Tools", None, None, None, "gpt-4", false)
            .unwrap();

        let msg = Message {
            id: MessageId::new(),
            chat_id,
            role: MessageRole::Assistant,
            content: "Let me run that.".to_string(),
            tool_calls: vec![ToolCall {
                id: "tc_1".to_string(),
                name: "run_code".to_string(),
                arguments: serde_json::json!({"lang": "python"}),
            }],
            artifacts: vec![Artifact {
                id: "art_1".to_string(),
                artifact_type: ArtifactType::Code,
                title: "main.py".to_string(),
                content: "print('hello')".to_string(),
                language: Some("python".to_string()),
            }],
            timestamp: chrono::Utc::now(),
            metadata: serde_json::json!({"token_usage": 42}),
            parent_id: None,
        };

        store.add_message(&msg).unwrap();
        let fetched = store.get_messages(chat_id).unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].tool_calls.len(), 1);
        assert_eq!(fetched[0].tool_calls[0].name, "run_code");
        assert_eq!(fetched[0].artifacts.len(), 1);
        assert_eq!(fetched[0].artifacts[0].title, "main.py");
        assert_eq!(fetched[0].metadata["token_usage"], 42);
    }

    // ── Pinned chats ───────────────────────────────────────────────────

    #[test]
    fn pin_chat_pins_and_unpins() {
        let store = MemoryStore::open_in_memory().unwrap();
        let id = ChatId::new();
        store.create_chat(id, "Pinnable", None, None, None, "gpt-4", false).unwrap();

        assert!(store.pin_chat(id, true).unwrap());
        let chat = store.get_chat(id).unwrap().unwrap();
        assert!(chat.pinned);

        assert!(store.pin_chat(id, false).unwrap());
        let chat = store.get_chat(id).unwrap().unwrap();
        assert!(!chat.pinned);
    }

    #[test]
    fn pin_unknown_chat_returns_false() {
        let store = MemoryStore::open_in_memory().unwrap();
        assert!(!store.pin_chat(ChatId::new(), true).unwrap());
    }

    #[test]
    fn create_chat_pinned_true() {
        let store = MemoryStore::open_in_memory().unwrap();
        let id = ChatId::new();
        store.create_chat(id, "Already Pinned", None, None, None, "gpt-4", true).unwrap();
        let chat = store.get_chat(id).unwrap().unwrap();
        assert!(chat.pinned);
    }

    #[test]
    fn list_pinned_chats_filters_correctly() {
        let store = MemoryStore::open_in_memory().unwrap();
        let id_a = ChatId::new();
        let id_b = ChatId::new();
        let id_c = ChatId::new();
        store.create_chat(id_a, "Pinned A", None, None, None, "gpt-4", true).unwrap();
        store.create_chat(id_b, "Not pinned", None, None, None, "gpt-4", false).unwrap();
        store.create_chat(id_c, "Pinned C", None, None, None, "gpt-4", true).unwrap();

        let pinned = store.list_pinned_chats().unwrap();
        assert_eq!(pinned.len(), 2);
        assert!(pinned.iter().all(|c| c.pinned));
    }

    // ── Session persistence ────────────────────────────────────────────

    #[test]
    fn mark_chat_read_sets_last_read_message_id() {
        let store = MemoryStore::open_in_memory().unwrap();
        let chat_id = ChatId::new();
        store.create_chat(chat_id, "Read Test", None, None, None, "gpt-4", false).unwrap();

        let msg = make_message(chat_id, MessageRole::User, "hello");
        store.add_message(&msg).unwrap();

        assert!(store.mark_chat_read(chat_id, msg.id).unwrap());
        let chat = store.get_chat(chat_id).unwrap().unwrap();
        assert_eq!(chat.last_read_message_id, Some(msg.id));
    }

    #[test]
    fn mark_chat_read_unknown_returns_false() {
        let store = MemoryStore::open_in_memory().unwrap();
        assert!(!store.mark_chat_read(ChatId::new(), MessageId::new()).unwrap());
    }

    #[test]
    fn get_chat_with_unread_all_unread() {
        let store = MemoryStore::open_in_memory().unwrap();
        let chat_id = ChatId::new();
        store.create_chat(chat_id, "Unread", None, None, None, "gpt-4", false).unwrap();

        for i in 0..3 {
            store.add_message(&make_message(chat_id, MessageRole::User, &format!("msg {i}"))).unwrap();
        }

        let (_, unread) = store.get_chat_with_unread(chat_id).unwrap().unwrap();
        assert_eq!(unread, 3);
    }

    #[test]
    fn get_chat_with_unread_after_mark_read() {
        let store = MemoryStore::open_in_memory().unwrap();
        let chat_id = ChatId::new();
        store.create_chat(chat_id, "Partial", None, None, None, "gpt-4", false).unwrap();

        let m1 = make_message(chat_id, MessageRole::User, "first");
        store.add_message(&m1).unwrap();
        // Small sleep to ensure distinct timestamps
        std::thread::sleep(std::time::Duration::from_millis(5));
        let m2 = make_message(chat_id, MessageRole::Assistant, "second");
        store.add_message(&m2).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let m3 = make_message(chat_id, MessageRole::User, "third");
        store.add_message(&m3).unwrap();

        store.mark_chat_read(chat_id, m1.id).unwrap();
        let (_, unread) = store.get_chat_with_unread(chat_id).unwrap().unwrap();
        assert_eq!(unread, 2);
    }

    // ── Branching ─────────────────────────────────────────────────────

    #[test]
    fn parent_id_roundtrips() {
        let store = MemoryStore::open_in_memory().unwrap();
        let chat_id = ChatId::new();
        store.create_chat(chat_id, "Branch", None, None, None, "gpt-4", false).unwrap();

        let root = make_message(chat_id, MessageRole::User, "root");
        let child = make_message_with_parent(chat_id, MessageRole::Assistant, "child", root.id);
        store.add_message(&root).unwrap();
        store.add_message(&child).unwrap();

        let msgs = store.get_messages(chat_id).unwrap();
        assert_eq!(msgs[0].parent_id, None);
        assert_eq!(msgs[1].parent_id, Some(root.id));
    }

    #[test]
    fn get_message_children_returns_direct_children() {
        let store = MemoryStore::open_in_memory().unwrap();
        let chat_id = ChatId::new();
        store.create_chat(chat_id, "Children", None, None, None, "gpt-4", false).unwrap();

        let root = make_message(chat_id, MessageRole::User, "root");
        let c1 = make_message_with_parent(chat_id, MessageRole::Assistant, "c1", root.id);
        let c2 = make_message_with_parent(chat_id, MessageRole::User, "c2", root.id);
        let gc = make_message_with_parent(chat_id, MessageRole::Assistant, "grandchild", c1.id);
        for m in [&root, &c1, &c2, &gc] {
            store.add_message(m).unwrap();
        }

        let children = store.get_message_children(root.id).unwrap();
        assert_eq!(children.len(), 2);

        // grandchild is a child of c1, not root
        let grandchildren = store.get_message_children(c1.id).unwrap();
        assert_eq!(grandchildren.len(), 1);
        assert_eq!(grandchildren[0].content, "grandchild");
    }

    #[test]
    fn get_message_tree_builds_correct_structure() {
        let store = MemoryStore::open_in_memory().unwrap();
        let chat_id = ChatId::new();
        store.create_chat(chat_id, "Tree", None, None, None, "gpt-4", false).unwrap();

        let root = make_message(chat_id, MessageRole::User, "root");
        let child = make_message_with_parent(chat_id, MessageRole::Assistant, "child", root.id);
        let grandchild =
            make_message_with_parent(chat_id, MessageRole::User, "grandchild", child.id);
        for m in [&root, &child, &grandchild] {
            store.add_message(m).unwrap();
        }

        let tree = store.get_message_tree(chat_id).unwrap();
        assert_eq!(tree.len(), 1); // one root
        assert_eq!(tree[0].message.content, "root");
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(tree[0].children[0].message.content, "child");
        assert_eq!(tree[0].children[0].children.len(), 1);
        assert_eq!(tree[0].children[0].children[0].message.content, "grandchild");
    }

    #[test]
    fn fork_from_message_creates_new_chat() {
        let store = MemoryStore::open_in_memory().unwrap();
        let chat_id = ChatId::new();
        store.create_chat(chat_id, "Original", None, None, None, "gpt-4", false).unwrap();

        let m1 = make_message(chat_id, MessageRole::User, "hello");
        let m2 = make_message(chat_id, MessageRole::Assistant, "world");
        let m3 = make_message(chat_id, MessageRole::User, "extra");
        for m in [&m1, &m2, &m3] {
            store.add_message(m).unwrap();
        }

        // Fork at m2 — the new chat should have m1 and m2 only.
        let fork_id = store.fork_from_message(chat_id, m2.id).unwrap();
        assert_ne!(fork_id, chat_id);

        let fork_msgs = store.get_messages(fork_id).unwrap();
        assert_eq!(fork_msgs.len(), 2);
        assert_eq!(fork_msgs[0].content, "hello");
        assert_eq!(fork_msgs[1].content, "world");
    }

    #[test]
    fn fork_preserves_content() {
        let store = MemoryStore::open_in_memory().unwrap();
        let chat_id = ChatId::new();
        store.create_chat(chat_id, "Source", None, None, None, "claude-3", false).unwrap();

        let msg = make_message(chat_id, MessageRole::User, "important context");
        store.add_message(&msg).unwrap();

        let fork_id = store.fork_from_message(chat_id, msg.id).unwrap();
        let fork_msgs = store.get_messages(fork_id).unwrap();
        assert_eq!(fork_msgs[0].content, "important context");
        assert_eq!(fork_msgs[0].role, MessageRole::User);
    }

    #[test]
    fn fork_invalid_message_returns_error() {
        let store = MemoryStore::open_in_memory().unwrap();
        let chat_id = ChatId::new();
        store.create_chat(chat_id, "Chat", None, None, None, "gpt-4", false).unwrap();

        let result = store.fork_from_message(chat_id, MessageId::new());
        assert!(result.is_err());
    }

    #[test]
    fn empty_chat_tree_returns_empty() {
        let store = MemoryStore::open_in_memory().unwrap();
        let chat_id = ChatId::new();
        store.create_chat(chat_id, "Empty", None, None, None, "gpt-4", false).unwrap();
        let tree = store.get_message_tree(chat_id).unwrap();
        assert!(tree.is_empty());
    }
}
