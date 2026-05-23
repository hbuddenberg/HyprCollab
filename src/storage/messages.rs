use crate::state::Message;
use crate::utils::paths::data_dir;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

/// Path to the JSONL file for a specific chat.
/// ~/.local/share/hypr-collab/data/folders/{folder_id}/chats/{chat_id}/messages.jsonl
pub fn messages_path(folder_id: &str, chat_id: &str) -> PathBuf {
    data_dir()
        .join("folders")
        .join(folder_id)
        .join("chats")
        .join(chat_id)
        .join("messages.jsonl")
}

/// Append a single message to the chat's JSONL file.
/// Creates parent directories on first call. Errors are propagated to the caller.
pub fn append_message(folder_id: &str, chat_id: &str, msg: &Message) -> anyhow::Result<()> {
    let path = messages_path(folder_id, chat_id);
    fs::create_dir_all(path.parent().unwrap())?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(file, "{}", serde_json::to_string(msg)?)?;
    Ok(())
}

/// Load all messages for a chat from its JSONL file.
/// Returns Ok(vec![]) if the file does not exist (first-run / empty chat).
/// Malformed lines are silently skipped.
pub fn load_messages(folder_id: &str, chat_id: &str) -> anyhow::Result<Vec<Message>> {
    let path = messages_path(folder_id, chat_id);
    if !path.exists() {
        return Ok(vec![]);
    }
    let reader = BufReader::new(File::open(&path)?);
    let msgs = reader
        .lines()
        .filter_map(|l| l.ok())
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Message>(&l).ok())
        .collect();
    Ok(msgs)
}
