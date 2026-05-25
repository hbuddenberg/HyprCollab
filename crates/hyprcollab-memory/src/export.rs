//! Conversation export — Markdown and JSON formats.

use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::types::{ChatId, Message, MessageId, MessageRole};
use serde::{Deserialize, Serialize};

use crate::search::SearchResult;
use crate::store::{ChatRecord, MemoryStore};

// ── ExportFormat ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Markdown,
    Json,
}

// ── ExportOptions ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    pub format: ExportFormat,
    pub include_metadata: bool,
    pub include_tool_calls: bool,
    pub include_artifacts: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: ExportFormat::Markdown,
            include_metadata: false,
            include_tool_calls: false,
            include_artifacts: false,
        }
    }
}

// ── ChatExport ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatExport {
    pub id: String,
    pub title: String,
    pub model: String,
    pub created_at: String,
    pub messages: Vec<MessageExport>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageExport {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

// ── Exporter ──────────────────────────────────────────────────────────────────

/// Export conversations in Markdown or JSON format.
pub struct Exporter<'a> {
    store: &'a MemoryStore,
}

impl<'a> Exporter<'a> {
    pub fn new(store: &'a MemoryStore) -> Self {
        Self { store }
    }

    /// Export an entire chat.
    pub fn export_chat(&self, chat_id: ChatId, options: &ExportOptions) -> Result<String> {
        let chat = self
            .store
            .get_chat(chat_id)?
            .ok_or_else(|| CoreError::Memory(format!("chat '{chat_id}' not found")))?;
        let messages = self.store.get_messages(chat_id)?;
        render(&chat, &messages, options)
    }

    /// Export a specific conversation thread rooted at `root_message_id`.
    pub fn export_conversation_thread(
        &self,
        chat_id: ChatId,
        root_message_id: MessageId,
        options: &ExportOptions,
    ) -> Result<String> {
        let chat = self
            .store
            .get_chat(chat_id)?
            .ok_or_else(|| CoreError::Memory(format!("chat '{chat_id}' not found")))?;
        let all_messages = self.store.get_messages(chat_id)?;
        let thread = collect_thread(&all_messages, root_message_id);
        render(&chat, &thread, options)
    }

    /// Export a list of search results.
    pub fn export_search_results(
        &self,
        results: Vec<SearchResult>,
        options: &ExportOptions,
    ) -> Result<String> {
        match options.format {
            ExportFormat::Markdown => {
                let mut out = String::from("# Search Results\n\n");
                for (i, r) in results.iter().enumerate() {
                    out.push_str(&format!("## Result {}\n\n", i + 1));
                    out.push_str(&format!("**Chat**: {}\n\n", r.chat_title));
                    out.push_str(&format!("**Score**: {:.4}\n\n", r.rank));
                    let role_label = role_label(r.message.role);
                    out.push_str(&format!("**{role_label}**: {}\n\n", r.message.content));
                    out.push_str("---\n\n");
                }
                Ok(out)
            }
            ExportFormat::Json => {
                #[derive(Serialize)]
                struct SearchExport {
                    results: Vec<ResultItem>,
                }
                #[derive(Serialize)]
                struct ResultItem {
                    chat_title: String,
                    rank: f64,
                    message_id: String,
                    role: String,
                    content: String,
                    timestamp: String,
                }
                let export = SearchExport {
                    results: results
                        .iter()
                        .map(|r| ResultItem {
                            chat_title: r.chat_title.clone(),
                            rank: r.rank,
                            message_id: r.message.id.to_string(),
                            role: r.message.role.to_string(),
                            content: r.message.content.clone(),
                            timestamp: r.message.timestamp.to_rfc3339(),
                        })
                        .collect(),
                };
                serde_json::to_string_pretty(&export)
                    .map_err(|e| CoreError::Memory(format!("json export search: {e}")))
            }
        }
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn render(chat: &ChatRecord, messages: &[Message], options: &ExportOptions) -> Result<String> {
    match options.format {
        ExportFormat::Markdown => Ok(render_markdown(chat, messages, options)),
        ExportFormat::Json => {
            let export = ChatExport {
                id: chat.id.to_string(),
                title: chat.title.clone(),
                model: chat.model.clone(),
                created_at: chat.created_at.clone(),
                messages: messages.iter().map(|m| msg_to_export(m, options)).collect(),
            };
            serde_json::to_string_pretty(&export)
                .map_err(|e| CoreError::Memory(format!("json export: {e}")))
        }
    }
}

fn render_markdown(chat: &ChatRecord, messages: &[Message], options: &ExportOptions) -> String {
    let mut out = format!("# {}\n\n", chat.title);
    out.push_str(&format!("**Model**: {}\n", chat.model));
    out.push_str(&format!("**Created**: {}\n\n", chat.created_at));
    out.push_str("## Messages\n\n");

    for msg in messages {
        let label = role_label(msg.role);
        out.push_str(&format!(
            "**{}** _{}_\n\n",
            label,
            msg.timestamp.to_rfc3339()
        ));
        out.push_str(&msg.content);
        out.push_str("\n\n");

        if options.include_tool_calls && !msg.tool_calls.is_empty() {
            let tc_json = serde_json::to_string_pretty(&msg.tool_calls).unwrap_or_default();
            out.push_str("```json\n");
            out.push_str(&tc_json);
            out.push_str("\n```\n\n");
        }

        if options.include_artifacts && !msg.artifacts.is_empty() {
            let a_json = serde_json::to_string_pretty(&msg.artifacts).unwrap_or_default();
            out.push_str("```json\n");
            out.push_str(&a_json);
            out.push_str("\n```\n\n");
        }

        if options.include_metadata && !msg.metadata.is_null() {
            out.push_str(&format!("<!-- metadata: {} -->\n\n", msg.metadata));
        }

        out.push_str("---\n\n");
    }

    out
}

fn msg_to_export(msg: &Message, options: &ExportOptions) -> MessageExport {
    let tool_calls = if options.include_tool_calls && !msg.tool_calls.is_empty() {
        serde_json::to_value(&msg.tool_calls).ok()
    } else {
        None
    };

    let artifacts = if options.include_artifacts && !msg.artifacts.is_empty() {
        serde_json::to_value(&msg.artifacts).ok()
    } else {
        None
    };

    let metadata = if options.include_metadata && !msg.metadata.is_null() {
        Some(msg.metadata.clone())
    } else {
        None
    };

    MessageExport {
        id: msg.id.to_string(),
        role: msg.role.to_string(),
        content: msg.content.clone(),
        timestamp: msg.timestamp.to_rfc3339(),
        parent_id: msg.parent_id.map(|p| p.to_string()),
        tool_calls,
        artifacts,
        metadata,
    }
}

fn role_label(role: MessageRole) -> &'static str {
    match role {
        MessageRole::User => "User",
        MessageRole::Assistant => "Assistant",
        MessageRole::System => "System",
        MessageRole::Tool => "Tool",
    }
}

/// Collect the thread starting at `root_id` (BFS over parent→children).
fn collect_thread(all: &[Message], root_id: MessageId) -> Vec<Message> {
    use std::collections::HashMap;

    let mut children: HashMap<MessageId, Vec<usize>> = HashMap::new();
    let mut root_idx = None;

    for (i, msg) in all.iter().enumerate() {
        if msg.id == root_id {
            root_idx = Some(i);
        }
        if let Some(parent) = msg.parent_id {
            children.entry(parent).or_default().push(i);
        }
    }

    let Some(_) = root_idx else {
        return Vec::new();
    };

    let mut result = Vec::new();
    let mut queue = vec![root_id];
    while let Some(current_id) = queue.pop() {
        if let Some(msg) = all.iter().find(|m| m.id == current_id) {
            result.push(msg.clone());
            if let Some(kids) = children.get(&current_id) {
                for &idx in kids {
                    queue.push(all[idx].id);
                }
            }
        }
    }
    result
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use hyprcollab_core::types::MessageRole;

    fn open_store() -> MemoryStore {
        MemoryStore::open_in_memory().unwrap()
    }

    fn seed_chat(store: &MemoryStore, title: &str) -> ChatId {
        let id = ChatId::new();
        store.create_chat(id, title, None, None, None, "gpt-4", false).unwrap();
        id
    }

    fn add_msg(store: &MemoryStore, chat_id: ChatId, role: MessageRole, content: &str) -> Message {
        let msg = Message {
            id: MessageId::new(),
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
        msg
    }

    #[test]
    fn export_chat_markdown_contains_title_and_content() {
        let store = open_store();
        let chat_id = seed_chat(&store, "My Project Chat");
        add_msg(&store, chat_id, MessageRole::User, "Hello there!");
        add_msg(&store, chat_id, MessageRole::Assistant, "Hi! How can I help?");

        let exporter = Exporter::new(&store);
        let md = exporter.export_chat(chat_id, &ExportOptions::default()).unwrap();

        assert!(md.contains("# My Project Chat"));
        assert!(md.contains("Hello there!"));
        assert!(md.contains("Hi! How can I help?"));
        assert!(md.contains("**User**"));
        assert!(md.contains("**Assistant**"));
    }

    #[test]
    fn export_chat_json_is_valid() {
        let store = open_store();
        let chat_id = seed_chat(&store, "JSON Test");
        add_msg(&store, chat_id, MessageRole::User, "test content");

        let exporter = Exporter::new(&store);
        let opts = ExportOptions { format: ExportFormat::Json, ..Default::default() };
        let json_str = exporter.export_chat(chat_id, &opts).unwrap();

        let parsed: ChatExport = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.title, "JSON Test");
        assert_eq!(parsed.messages.len(), 1);
        assert_eq!(parsed.messages[0].content, "test content");
    }

    #[test]
    fn export_chat_not_found() {
        let store = open_store();
        let exporter = Exporter::new(&store);
        let result = exporter.export_chat(ChatId::new(), &ExportOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn export_conversation_thread_returns_only_thread() {
        let store = open_store();
        let chat_id = seed_chat(&store, "Thread Chat");
        let root = add_msg(&store, chat_id, MessageRole::User, "root message");

        let child = Message {
            id: MessageId::new(),
            chat_id,
            role: MessageRole::Assistant,
            content: "child reply".into(),
            tool_calls: vec![],
            artifacts: vec![],
            timestamp: chrono::Utc::now(),
            metadata: serde_json::Value::Null,
            parent_id: Some(root.id),
        };
        store.add_message(&child).unwrap();
        add_msg(&store, chat_id, MessageRole::User, "separate message");

        let exporter = Exporter::new(&store);
        let opts = ExportOptions { format: ExportFormat::Json, ..Default::default() };
        let json_str = exporter.export_conversation_thread(chat_id, root.id, &opts).unwrap();
        let parsed: ChatExport = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.messages.len(), 2);
    }

    #[test]
    fn export_search_results_markdown() {
        let store = open_store();
        let chat_id = seed_chat(&store, "Search Chat");
        let msg = add_msg(&store, chat_id, MessageRole::User, "important keyword");

        let results = vec![SearchResult {
            message: msg,
            chat_title: "Search Chat".to_string(),
            rank: 1.5,
        }];

        let exporter = Exporter::new(&store);
        let opts = ExportOptions::default();
        let md = exporter.export_search_results(results, &opts).unwrap();
        assert!(md.contains("# Search Results"));
        assert!(md.contains("important keyword"));
        assert!(md.contains("Search Chat"));
    }

    #[test]
    fn export_search_results_json() {
        let store = open_store();
        let chat_id = seed_chat(&store, "Search Chat");
        let msg = add_msg(&store, chat_id, MessageRole::User, "keyword");

        let results = vec![SearchResult {
            message: msg,
            chat_title: "Search Chat".to_string(),
            rank: 0.9,
        }];

        let exporter = Exporter::new(&store);
        let opts = ExportOptions { format: ExportFormat::Json, ..Default::default() };
        let json_str = exporter.export_search_results(results, &opts).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(val["results"].as_array().unwrap().len(), 1);
        assert_eq!(val["results"][0]["content"].as_str(), Some("keyword"));
    }

    #[test]
    fn export_with_metadata_includes_comment() {
        let store = open_store();
        let chat_id = seed_chat(&store, "Meta Chat");
        let msg = Message {
            id: MessageId::new(),
            chat_id,
            role: MessageRole::User,
            content: "meta message".into(),
            tool_calls: vec![],
            artifacts: vec![],
            timestamp: chrono::Utc::now(),
            metadata: serde_json::json!({"key": "value"}),
            parent_id: None,
        };
        store.add_message(&msg).unwrap();

        let exporter = Exporter::new(&store);
        let opts = ExportOptions { include_metadata: true, ..Default::default() };
        let md = exporter.export_chat(chat_id, &opts).unwrap();
        assert!(md.contains("<!-- metadata:"));
    }
}
