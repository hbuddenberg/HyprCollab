use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use crate::utils::paths::data_dir;

use super::embedder::cosine_similarity;

#[derive(Serialize, Deserialize)]
pub struct RagEntry {
    pub id: String,
    pub folder_id: String,
    pub chat_id: String,
    pub chunk_index: u32,
    pub text: String,
    pub message_id: String,
    pub created_at: i64,
    pub vector: Vec<f32>,
}

pub struct SearchResult {
    pub text: String,
    pub score: f32,
    pub chat_id: String,
}

fn index_path(folder_id: &str) -> PathBuf {
    data_dir()
        .join("folders")
        .join(folder_id)
        .join("rag")
        .join("index.jsonl")
}

pub fn insert(folder_id: &str, entry: &RagEntry) -> anyhow::Result<()> {
    let path = index_path(folder_id);
    fs::create_dir_all(path.parent().unwrap())?;
    let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
    let line = serde_json::to_string(entry)? + "\n";
    f.write_all(line.as_bytes())?;
    Ok(())
}

pub fn search(folder_id: &str, query_vec: &[f32], top_k: usize, min_score: f32) -> anyhow::Result<Vec<SearchResult>> {
    let path = index_path(folder_id);
    if !path.exists() {
        return Ok(vec![]);
    }

    let file = File::open(&path)?;
    let reader = BufReader::new(file);
    let mut scored: Vec<(f32, String, String)> = Vec::new(); // (score, text, chat_id)

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: RagEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let score = cosine_similarity(query_vec, &entry.vector);
        if score >= min_score {
            scored.push((score, entry.text, entry.chat_id));
        }
    }

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_k);

    Ok(scored
        .into_iter()
        .map(|(score, text, chat_id)| SearchResult { text, score, chat_id })
        .collect())
}

pub fn clear(folder_id: &str) -> anyhow::Result<usize> {
    let path = index_path(folder_id);
    if !path.exists() {
        return Ok(0);
    }
    // Count before clearing
    let count = {
        let file = File::open(&path)?;
        BufReader::new(file).lines().filter(|l| l.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false)).count()
    };
    fs::write(&path, "")?;
    Ok(count)
}

/// List all folder IDs that have a RAG index on disk.
pub fn list_folder_ids() -> Vec<String> {
    let folders_dir = data_dir().join("folders");
    let Ok(entries) = fs::read_dir(&folders_dir) else { return vec![] };
    entries
        .flatten()
        .filter(|e| {
            e.path().join("rag").join("index.jsonl").exists()
        })
        .filter_map(|e| e.file_name().into_string().ok())
        .collect()
}

/// Search across every folder index and merge results by score.
/// `penalty` (0.0–1.0) is multiplied into scores from non-active folders
/// so the active folder's results rank higher when mixed.
pub fn search_all_folders(
    active_folder_id: Option<&str>,
    query_vec: &[f32],
    top_k: usize,
    min_score: f32,
    penalty: f32,
) -> anyhow::Result<Vec<SearchResult>> {
    let folder_ids = list_folder_ids();
    let mut all: Vec<(f32, String, String)> = Vec::new(); // (score, text, chat_id)

    for fid in &folder_ids {
        let path = index_path(fid);
        if !path.exists() { continue; }

        let is_active = active_folder_id.map(|a| a == fid).unwrap_or(false);
        let factor = if is_active { 1.0 } else { penalty };

        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() { continue; }
            let entry: RagEntry = match serde_json::from_str(&line) {
                Ok(e) => e,
                Err(_) => continue,
            };
            let raw_score = cosine_similarity(query_vec, &entry.vector);
            let score = raw_score * factor;
            if score >= min_score {
                all.push((score, entry.text, entry.chat_id));
            }
        }
    }

    all.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    // Deduplicate by text (same chunk can appear in multiple folder indexes if cross-linked)
    let mut seen = std::collections::HashSet::new();
    let deduped: Vec<SearchResult> = all
        .into_iter()
        .filter(|(_, text, _)| seen.insert(text.clone()))
        .take(top_k)
        .map(|(score, text, chat_id)| SearchResult { text, score, chat_id })
        .collect();

    Ok(deduped)
}

pub fn entry_id(folder_id: &str, chat_id: &str, chunk_index: u32) -> String {
    format!("{}_{}_{}", folder_id, chat_id, chunk_index)
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
