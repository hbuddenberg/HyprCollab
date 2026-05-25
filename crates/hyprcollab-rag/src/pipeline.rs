use crate::chunker::ChunkerConfig;
use crate::embedder::Embedder;
use crate::parser::ParserRegistry;
use crate::store::{ChunkWithEmbedding, SearchResult, VectorStore};
use crate::RagError;
use crate::Result;

/// Result of ingesting a single document.
pub struct IngestResult {
    pub chunks_created: usize,
    pub document_title: Option<String>,
}

/// End-to-end RAG pipeline: parse → chunk → embed → store → query.
pub struct RagPipeline {
    parser_registry: ParserRegistry,
    chunker_config: ChunkerConfig,
    embedder: Box<dyn Embedder>,
    store: VectorStore,
}

impl RagPipeline {
    pub fn new(
        chunker_config: ChunkerConfig,
        embedder: Box<dyn Embedder>,
        store: VectorStore,
    ) -> Self {
        Self {
            parser_registry: ParserRegistry::new(),
            chunker_config,
            embedder,
            store,
        }
    }

    /// Ingest a document: parse → chunk → embed → store.
    pub async fn ingest(&self, input: &[u8], filename: &str) -> Result<IngestResult> {
        let doc = self.parser_registry.parse(input, Some(filename))?;
        let document_title = doc.title.clone();

        let chunks = crate::chunker::chunk_document(&doc, &self.chunker_config);
        if chunks.is_empty() {
            return Ok(IngestResult { chunks_created: 0, document_title });
        }

        let texts: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();
        let embeddings = self
            .embedder
            .embed_batch(&texts)
            .await
            .map_err(|e| RagError::Pipeline(e.to_string()))?;

        if embeddings.len() != chunks.len() {
            return Err(RagError::Pipeline(format!(
                "embedding count {} != chunk count {}",
                embeddings.len(),
                chunks.len()
            )));
        }

        let cwe: Vec<ChunkWithEmbedding> = chunks
            .into_iter()
            .zip(embeddings)
            .map(|(chunk, embedding)| ChunkWithEmbedding { chunk, embedding })
            .collect();

        self.store.upsert(&cwe).await?;
        Ok(IngestResult { chunks_created: cwe.len(), document_title })
    }

    /// Query: embed the query text → vector search → return top-k results.
    pub async fn query(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let embedding = self
            .embedder
            .embed(query)
            .await
            .map_err(|e| RagError::Pipeline(e.to_string()))?;
        self.store.search(&embedding, limit).await
    }

    /// List all source files indexed in the store.
    pub async fn list_sources(&self) -> Result<Vec<String>> {
        self.store.list_sources().await
    }

    /// Remove all chunks for a given source file.
    pub async fn delete_source(&self, source_file: &str) -> Result<()> {
        self.store.delete_by_source(source_file).await
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::embedder::MockEmbedder;

    async fn make_pipeline(dim: usize) -> RagPipeline {
        let store = VectorStore::open(Path::new(":memory:")).await.unwrap();
        RagPipeline::new(
            ChunkerConfig::default(),
            Box::new(MockEmbedder { dim, fail: false }),
            store,
        )
    }

    #[tokio::test]
    async fn pipeline_ingest_empty_content_zero_chunks() {
        let p = make_pipeline(4).await;
        let result = p.ingest(b"", "empty.txt").await.unwrap();
        assert_eq!(result.chunks_created, 0);
    }

    #[tokio::test]
    async fn pipeline_ingest_text_doc() {
        let p = make_pipeline(4).await;
        let result = p.ingest(b"Hello world, this is a test document.", "doc.txt").await.unwrap();
        assert!(result.chunks_created >= 1);
    }

    #[tokio::test]
    async fn pipeline_ingest_markdown() {
        let p = make_pipeline(4).await;
        let md = b"# Title\n\nFirst paragraph content.\n\n## Section\n\nSecond paragraph content.";
        let result = p.ingest(md, "doc.md").await.unwrap();
        assert!(result.chunks_created >= 1);
    }

    #[tokio::test]
    async fn pipeline_ingest_result_title_from_markdown() {
        let p = make_pipeline(4).await;
        let result = p.ingest(b"# My Title\n\nContent here.", "doc.md").await.unwrap();
        assert_eq!(result.document_title.as_deref(), Some("My Title"));
    }

    #[tokio::test]
    async fn pipeline_query_returns_results_after_ingest() {
        let p = make_pipeline(4).await;
        p.ingest(b"Rust is a systems programming language.", "rust.txt").await.unwrap();
        let results = p.query("programming language", 5).await.unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn pipeline_query_empty_store_returns_empty() {
        let p = make_pipeline(4).await;
        let results = p.query("anything", 5).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn pipeline_query_limit_respected() {
        let p = make_pipeline(4).await;
        for i in 0..10u8 {
            p.ingest(format!("doc {i}").as_bytes(), &format!("doc{i}.txt")).await.unwrap();
        }
        let results = p.query("doc", 3).await.unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn pipeline_ingest_chunks_created_count() {
        // Long text that produces multiple chunks with small chunk_size.
        let p_small = {
            let store = VectorStore::open(Path::new(":memory:")).await.unwrap();
            RagPipeline::new(
                ChunkerConfig { chunk_size: 50, overlap: 0, separator: " ".into() },
                Box::new(MockEmbedder { dim: 4, fail: false }),
                store,
            )
        };
        let long_text = "word ".repeat(100);
        let result = p_small.ingest(long_text.as_bytes(), "long.txt").await.unwrap();
        assert!(result.chunks_created > 1, "expected >1 chunks, got {}", result.chunks_created);
    }
}
