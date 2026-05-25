pub mod chunker;
pub mod embedder;
pub mod parser;
pub mod pipeline;
pub mod store;

pub use chunker::{chunk_document, Chunk, ChunkMetadata, ChunkerConfig};
pub use embedder::{Embedder, OpenAiEmbedder};
pub use parser::{parse, DocumentParser, ParsedDocument, ParserRegistry, Section};
pub use pipeline::{IngestResult, RagPipeline};
pub use store::{ChunkWithEmbedding, SearchResult, VectorStore};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RagError {
    #[error("parse: {0}")]
    Parse(String),
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("PDF: {0}")]
    Pdf(String),
    #[error("CSV: {0}")]
    Csv(#[from] csv::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("encoding: {0}")]
    Encoding(String),
    #[error("embed: {0}")]
    Embed(String),
    #[error("store: {0}")]
    Store(String),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("pipeline: {0}")]
    Pipeline(String),
}

pub type Result<T> = std::result::Result<T, RagError>;

#[cfg(test)]
mod tests;
