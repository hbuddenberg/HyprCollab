pub mod parser;

pub use parser::{parse, DocumentParser, ParsedDocument, Section};

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
}

pub type Result<T> = std::result::Result<T, RagError>;

#[cfg(test)]
mod tests;
