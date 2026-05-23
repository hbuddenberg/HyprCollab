pub mod chunker;
pub mod embedder;
pub mod index;

use crate::state::Message;
use crate::storage::RagConfig;
use embedder::Embedder;
use index::RagEntry;

/// Index a message into the folder's index AND (when scope != "folder") into
/// every other existing folder's index entry is NOT duplicated — we only write
/// to the owning folder. Global search is achieved by searching all folder
/// indexes at query time, so no separate global index is needed.
///
/// If you want a dedicated global index file, set `scope = "global"` and this
/// function writes to `global/rag/index.jsonl` in addition to the folder index.
pub async fn index_message(
    folder_id: &str,
    chat_id: &str,
    msg: &Message,
    embedder: &Embedder,
    config: &RagConfig,
) -> anyhow::Result<()> {
    let chunks = chunker::chunk(&msg.content, config.chunk_size, config.chunk_overlap);
    if chunks.is_empty() {
        return Ok(());
    }

    let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
    let vectors = embedder.embed_batch(&texts).await?;

    for (chunk, vector) in chunks.into_iter().zip(vectors.into_iter()) {
        // Always write to the folder index
        let entry = RagEntry {
            id: index::entry_id(folder_id, chat_id, chunk.index),
            folder_id: folder_id.to_string(),
            chat_id: chat_id.to_string(),
            chunk_index: chunk.index,
            text: chunk.text.clone(),
            message_id: format!("{}_{}_{}", folder_id, chat_id, msg.timestamp),
            created_at: index::now_secs(),
            vector: vector.clone(),
        };
        index::insert(folder_id, &entry)?;

        // When scope is "global", also mirror into the shared global index
        // so that "global"-only queries don't need to scan each folder.
        if config.is_global() {
            let global_entry = RagEntry {
                id: format!("global_{}", entry.id),
                folder_id: folder_id.to_string(),
                ..entry
            };
            index::insert("__global__", &global_entry)?;
        }
    }

    Ok(())
}

/// Search for relevant context and format it for the system prompt.
/// Scope from config:
///   "folder" — only the active folder index
///   "global" — the shared __global__ index (populated when scope="global")
///   "both"   — all folder indexes merged; active folder results score higher (penalty 0.9)
pub async fn search_context(
    folder_id: &str,
    query: &str,
    embedder: &Embedder,
    config: &RagConfig,
) -> anyhow::Result<String> {
    let query_vec = embedder.embed_one(query).await?;

    let results = if config.is_folder() {
        index::search(folder_id, &query_vec, config.top_k, 0.7)?
    } else if config.is_global() {
        // Use dedicated global index if it exists; fall back to all-folders scan
        if index::list_folder_ids().iter().any(|id| id == "__global__") {
            index::search("__global__", &query_vec, config.top_k, 0.7)?
        } else {
            index::search_all_folders(Some(folder_id), &query_vec, config.top_k, 0.7, 1.0)?
        }
    } else {
        // "both": search all folder indexes; active folder gets full score, others penalised 10%
        index::search_all_folders(Some(folder_id), &query_vec, config.top_k, 0.7, 0.9)?
    };

    if results.is_empty() {
        return Ok(String::new());
    }

    let mut ctx = "Contexto relevante de conversaciones anteriores:\n".to_string();
    for r in &results {
        ctx.push_str(&format!("- {}\n", r.text.trim()));
    }
    Ok(ctx)
}

/// Clear the RAG index for a specific folder. Returns chunks removed.
pub fn clear_index(folder_id: &str) -> anyhow::Result<usize> {
    index::clear(folder_id)
}

/// Clear the global mirror index (only meaningful when scope="global").
pub fn clear_global_index() -> anyhow::Result<usize> {
    index::clear("__global__")
}
