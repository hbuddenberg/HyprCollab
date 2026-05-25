pub mod recursive;

pub use recursive::chunk_document;

/// A chunk of text ready for embedding.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub id: String,
    pub content: String,
    pub metadata: ChunkMetadata,
}

/// Metadata attached to each chunk.
#[derive(Debug, Clone)]
pub struct ChunkMetadata {
    pub source_file: Option<String>,
    pub section_heading: Option<String>,
    pub start_line: usize,
    pub end_line: usize,
    pub chunk_index: usize,
    pub mime_type: String,
}

/// Configuration for the text chunker.
#[derive(Debug, Clone)]
pub struct ChunkerConfig {
    /// Target bytes per chunk.
    pub chunk_size: usize,
    /// Overlap bytes between consecutive chunks.
    pub overlap: usize,
    /// Primary separator (first tried before fallbacks).
    pub separator: String,
}

impl Default for ChunkerConfig {
    fn default() -> Self {
        Self {
            chunk_size: 1000,
            overlap: 200,
            separator: "\n\n".to_string(),
        }
    }
}
