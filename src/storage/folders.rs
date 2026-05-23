use crate::state::{Chat, Folder};
use crate::utils::paths::data_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::warn;

// ── On-disk schemas (no messages / no chats array) ───────────────────────────

#[derive(Serialize, Deserialize)]
struct FolderMeta {
    id: String,
    name: String,
    icon: String,
    git_branch: Option<String>,
    workdir: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ChatMeta {
    id: String,
    title: String,
    agent: String,
    model: String,
    tokens_used: u64,
    created_at: i64,
    #[serde(default)]
    system_prompt: Option<String>,
}

// ── Path helpers ─────────────────────────────────────────────────────────────

pub fn folder_dir(folder_id: &str) -> PathBuf {
    data_dir().join("folders").join(folder_id)
}

pub fn chat_dir(folder_id: &str, chat_id: &str) -> PathBuf {
    folder_dir(folder_id).join("chats").join(chat_id)
}

// ── Writers ──────────────────────────────────────────────────────────────────

/// Write folder metadata to ~/.local/share/hypr-collab/data/folders/{id}/metadata.json
pub fn save_folder_metadata(folder: &Folder) -> anyhow::Result<()> {
    let dir = folder_dir(&folder.id);
    fs::create_dir_all(&dir)?;
    let meta = FolderMeta {
        id: folder.id.clone(),
        name: folder.name.clone(),
        icon: folder.icon.clone(),
        git_branch: folder.git_branch.clone(),
        workdir: folder.workdir.clone(),
    };
    fs::write(dir.join("metadata.json"), serde_json::to_string_pretty(&meta)?)?;
    Ok(())
}

/// Write chat metadata to .../folders/{folder_id}/chats/{chat_id}/metadata.json
pub fn save_chat_metadata(folder_id: &str, chat: &Chat) -> anyhow::Result<()> {
    let dir = chat_dir(folder_id, &chat.id);
    fs::create_dir_all(&dir)?;
    let meta = ChatMeta {
        id: chat.id.clone(),
        title: chat.title.clone(),
        agent: chat.agent.clone(),
        model: chat.model.clone(),
        tokens_used: chat.tokens_used,
        created_at: chat.created_at,
        system_prompt: chat.system_prompt.clone(),
    };
    fs::write(dir.join("metadata.json"), serde_json::to_string_pretty(&meta)?)?;
    Ok(())
}

// ── Loader ───────────────────────────────────────────────────────────────────

/// Load all folders (and their chats + messages) from disk.
/// Returns Ok(vec![]) if the folders directory doesn't exist yet (first run).
/// Entries with missing or malformed metadata.json are skipped with a warning.
pub fn load_all_folders() -> anyhow::Result<Vec<Folder>> {
    let base = data_dir().join("folders");
    if !base.exists() {
        return Ok(vec![]);
    }

    let mut folders = Vec::new();

    for entry in fs::read_dir(&base)? {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => { warn!("read_dir entry error: {}", e); continue; }
        };
        let folder_path = entry.path();
        if !folder_path.is_dir() {
            continue;
        }

        let meta_path = folder_path.join("metadata.json");
        let folder_meta: FolderMeta = match read_json(&meta_path) {
            Ok(m) => m,
            Err(e) => {
                warn!("skip folder {:?}: {}", folder_path, e);
                continue;
            }
        };

        let chats = load_chats_for_folder(&folder_meta.id, &folder_path);

        folders.push(Folder {
            id: folder_meta.id,
            name: folder_meta.name,
            icon: folder_meta.icon,
            chats,
            git_branch: folder_meta.git_branch,
            workdir: folder_meta.workdir,
        });
    }

    // Sort folders by name for consistent ordering
    folders.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(folders)
}

fn load_chats_for_folder(folder_id: &str, folder_path: &std::path::Path) -> Vec<Chat> {
    let chats_dir = folder_path.join("chats");
    if !chats_dir.exists() {
        return vec![];
    }

    let entries = match fs::read_dir(&chats_dir) {
        Ok(e) => e,
        Err(e) => { warn!("read_dir chats for {}: {}", folder_id, e); return vec![]; }
    };

    let mut chats = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => { warn!("read_dir chat entry: {}", e); continue; }
        };
        let chat_path = entry.path();
        if !chat_path.is_dir() {
            continue;
        }

        let meta_path = chat_path.join("metadata.json");
        let chat_meta: ChatMeta = match read_json(&meta_path) {
            Ok(m) => m,
            Err(e) => {
                warn!("skip chat {:?}: {}", chat_path, e);
                continue;
            }
        };

        let messages = super::messages::load_messages(folder_id, &chat_meta.id)
            .unwrap_or_else(|e| {
                warn!("load messages for chat {}: {}", chat_meta.id, e);
                vec![]
            });

        chats.push(Chat {
            id: chat_meta.id,
            title: chat_meta.title,
            messages,
            agent: chat_meta.agent,
            model: chat_meta.model,
            tokens_used: chat_meta.tokens_used,
            created_at: chat_meta.created_at,
            system_prompt: chat_meta.system_prompt,
        });
    }

    // Sort by creation time for consistent ordering
    chats.sort_by_key(|c| c.created_at);

    chats
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &std::path::Path) -> anyhow::Result<T> {
    if !path.exists() {
        return Err(anyhow::anyhow!("missing: {}", path.display()));
    }
    let content = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content)?)
}
