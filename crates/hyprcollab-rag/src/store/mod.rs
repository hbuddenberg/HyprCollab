use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection};

use crate::chunker::Chunk;
use crate::RagError;
use crate::Result;

/// A chunk paired with its embedding vector.
pub struct ChunkWithEmbedding {
    pub chunk: Chunk,
    pub embedding: Vec<f32>,
}

/// A search result from the vector store.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub chunk: Chunk,
    /// Cosine similarity score in [-1, 1].
    pub score: f32,
}

/// SQLite-backed vector store using in-process cosine similarity.
#[derive(Clone)]
pub struct VectorStore {
    conn: Arc<Mutex<Connection>>,
    table_name: String,
}

impl VectorStore {
    pub async fn open(path: &Path) -> Result<Self> {
        let conn = if path == Path::new(":memory:") {
            Connection::open_in_memory()
        } else {
            Connection::open(path)
        }
        .map_err(|e| RagError::Store(e.to_string()))?;

        let table_name = "rag_chunks".to_string();
        conn.execute_batch(&format!(
            "CREATE TABLE IF NOT EXISTS {table_name} (
                id TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                source_file TEXT,
                section_heading TEXT,
                start_line INTEGER NOT NULL DEFAULT 0,
                end_line INTEGER NOT NULL DEFAULT 0,
                chunk_index INTEGER NOT NULL DEFAULT 0,
                mime_type TEXT NOT NULL DEFAULT '',
                embedding BLOB NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_{table_name}_source ON {table_name}(source_file);"
        ))
        .map_err(|e| RagError::Store(e.to_string()))?;

        Ok(Self { conn: Arc::new(Mutex::new(conn)), table_name })
    }

    /// Insert or replace chunks with their embeddings.
    pub async fn upsert(&self, chunks: &[ChunkWithEmbedding]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let sql = format!(
            "INSERT OR REPLACE INTO {} \
             (id, content, source_file, section_heading, start_line, end_line, chunk_index, mime_type, embedding) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            self.table_name
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| RagError::Store(e.to_string()))?;

        for cwe in chunks {
            let emb_bytes = f32_to_bytes(&cwe.embedding);
            stmt.execute(params![
                cwe.chunk.id,
                cwe.chunk.content,
                cwe.chunk.metadata.source_file,
                cwe.chunk.metadata.section_heading,
                cwe.chunk.metadata.start_line as i64,
                cwe.chunk.metadata.end_line as i64,
                cwe.chunk.metadata.chunk_index as i64,
                cwe.chunk.metadata.mime_type,
                emb_bytes,
            ])
            .map_err(|e| RagError::Store(e.to_string()))?;
        }
        Ok(())
    }

    /// Brute-force cosine similarity search; returns top-`limit` results.
    pub async fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<SearchResult>> {
        let conn = self.conn.lock().unwrap();
        let sql = format!(
            "SELECT id, content, source_file, section_heading, start_line, end_line, chunk_index, mime_type, embedding \
             FROM {}",
            self.table_name
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| RagError::Store(e.to_string()))?;

        let mut rows: Vec<(Chunk, f32)> = stmt
            .query_map([], |row| {
                let emb_bytes: Vec<u8> = row.get(8)?;
                Ok((
                    Chunk {
                        id: row.get(0)?,
                        content: row.get(1)?,
                        metadata: crate::chunker::ChunkMetadata {
                            source_file: row.get(2)?,
                            section_heading: row.get(3)?,
                            start_line: row.get::<_, i64>(4)? as usize,
                            end_line: row.get::<_, i64>(5)? as usize,
                            chunk_index: row.get::<_, i64>(6)? as usize,
                            mime_type: row.get(7)?,
                        },
                    },
                    emb_bytes,
                ))
            })
            .map_err(|e| RagError::Store(e.to_string()))?
            .filter_map(|r| r.ok())
            .map(|(chunk, bytes)| {
                let embedding = bytes_to_f32(&bytes);
                let score = cosine_similarity(query_embedding, &embedding);
                (chunk, score)
            })
            .collect();

        rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        rows.truncate(limit);

        Ok(rows
            .into_iter()
            .map(|(chunk, score)| SearchResult { chunk, score })
            .collect())
    }

    /// Delete all chunks whose source_file matches.
    pub async fn delete_by_source(&self, source_file: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            &format!("DELETE FROM {} WHERE source_file = ?1", self.table_name),
            params![source_file],
        )
        .map_err(|e| RagError::Store(e.to_string()))?;
        Ok(())
    }

    /// List distinct source files stored in the vector store.
    pub async fn list_sources(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT DISTINCT source_file FROM {} WHERE source_file IS NOT NULL ORDER BY source_file",
                self.table_name
            ))
            .map_err(|e| RagError::Store(e.to_string()))?;

        let sources = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| RagError::Store(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(sources)
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn f32_to_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn bytes_to_f32(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunker::ChunkMetadata;

    fn make_chunk(id: &str, content: &str, source: Option<&str>) -> Chunk {
        Chunk {
            id: id.to_string(),
            content: content.to_string(),
            metadata: ChunkMetadata {
                source_file: source.map(String::from),
                section_heading: None,
                start_line: 0,
                end_line: 1,
                chunk_index: 0,
                mime_type: "text/plain".to_string(),
            },
        }
    }

    fn cwe(id: &str, content: &str, embedding: Vec<f32>, source: Option<&str>) -> ChunkWithEmbedding {
        ChunkWithEmbedding { chunk: make_chunk(id, content, source), embedding }
    }

    async fn mem_store() -> VectorStore {
        VectorStore::open(Path::new(":memory:")).await.unwrap()
    }

    #[tokio::test]
    async fn store_open_creates_tables() {
        let store = mem_store().await;
        let sources = store.list_sources().await.unwrap();
        assert!(sources.is_empty());
    }

    #[tokio::test]
    async fn store_upsert_and_list_sources() {
        let store = mem_store().await;
        store.upsert(&[cwe("c1", "hello", vec![1.0, 0.0], Some("a.txt"))]).await.unwrap();
        let sources = store.list_sources().await.unwrap();
        assert_eq!(sources, vec!["a.txt"]);
    }

    #[tokio::test]
    async fn store_search_returns_results() {
        let store = mem_store().await;
        store.upsert(&[cwe("c1", "doc one", vec![1.0, 0.0], None)]).await.unwrap();
        let results = store.search(&[1.0, 0.0], 5).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].chunk.content, "doc one");
    }

    #[tokio::test]
    async fn store_search_respects_limit() {
        let store = mem_store().await;
        for i in 0..10u32 {
            let v = vec![i as f32, 0.0];
            store.upsert(&[cwe(&format!("c{i}"), "x", v, None)]).await.unwrap();
        }
        let results = store.search(&[1.0, 0.0], 3).await.unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn store_search_empty_returns_empty() {
        let store = mem_store().await;
        let results = store.search(&[1.0, 0.0], 5).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn store_delete_by_source_removes_chunks() {
        let store = mem_store().await;
        store.upsert(&[
            cwe("c1", "from file1", vec![1.0, 0.0], Some("file1.txt")),
            cwe("c2", "from file2", vec![0.0, 1.0], Some("file2.txt")),
        ]).await.unwrap();
        store.delete_by_source("file1.txt").await.unwrap();
        let results = store.search(&[1.0, 0.0], 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].chunk.metadata.source_file.as_deref(), Some("file2.txt"));
    }

    #[tokio::test]
    async fn store_upsert_idempotent_updates_existing() {
        let store = mem_store().await;
        store.upsert(&[cwe("c1", "original", vec![1.0, 0.0], None)]).await.unwrap();
        store.upsert(&[cwe("c1", "updated", vec![1.0, 0.0], None)]).await.unwrap();
        let results = store.search(&[1.0, 0.0], 5).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].chunk.content, "updated");
    }

    #[tokio::test]
    async fn store_cosine_similarity_ordering() {
        let store = mem_store().await;
        // c1 is identical to query (sim=1.0), c2 is orthogonal (sim=0.0)
        store.upsert(&[
            cwe("c1", "identical", vec![1.0, 0.0], None),
            cwe("c2", "orthogonal", vec![0.0, 1.0], None),
        ]).await.unwrap();
        let results = store.search(&[1.0, 0.0], 2).await.unwrap();
        assert_eq!(results[0].chunk.content, "identical");
        assert!(results[0].score > results[1].score);
    }
}
