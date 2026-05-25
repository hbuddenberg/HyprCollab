use std::collections::HashMap;
use std::path::Path;
use crate::Result;

mod markdown;
mod html;
mod pdf;
mod code;
mod text;

pub use code::CodeParser;
pub use html::HtmlParser;
pub use markdown::MarkdownParser;
pub use pdf::PdfParser;
pub use text::TextParser;

/// A logical section extracted from a document.
#[derive(Debug, Clone)]
pub struct Section {
    pub heading: Option<String>,
    pub content: String,
    pub level: u8,
    pub start_line: usize,
    pub end_line: usize,
}

/// Result of parsing a document into structured content.
#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub title: Option<String>,
    pub content: String,
    pub mime_type: String,
    pub sections: Vec<Section>,
    pub metadata: HashMap<String, String>,
}

/// Implemented by every format-specific parser.
pub trait DocumentParser: Send + Sync {
    fn mime_types(&self) -> &[&str];
    fn extensions(&self) -> &[&str];
    fn parse(&self, input: &[u8], filename: Option<&str>) -> Result<ParsedDocument>;
}

/// Registry that dispatches to the correct parser by file extension.
pub struct ParserRegistry;

impl Default for ParserRegistry {
    fn default() -> Self {
        Self
    }
}

impl ParserRegistry {
    pub fn new() -> Self {
        Self
    }

    pub fn parse(&self, input: &[u8], filename: Option<&str>) -> crate::Result<ParsedDocument> {
        parse(input, filename)
    }
}

/// Dispatch to the right parser based on file extension.
pub fn parse(input: &[u8], filename: Option<&str>) -> Result<ParsedDocument> {
    let ext = filename
        .and_then(|f| Path::new(f).extension())
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    match ext.as_deref() {
        Some("md") | Some("markdown") | Some("mkd") => MarkdownParser.parse(input, filename),
        Some("pdf") => PdfParser.parse(input, filename),
        Some("html") | Some("htm") | Some("xhtml") => HtmlParser.parse(input, filename),
        Some("csv") | Some("json") | Some("txt") | Some("text") => {
            TextParser.parse(input, filename)
        }
        Some("rs") | Some("py") | Some("js") | Some("ts") | Some("go")
        | Some("java") | Some("c") | Some("cpp") | Some("cc") | Some("h")
        | Some("hpp") | Some("toml") | Some("yaml") | Some("yml") => {
            CodeParser.parse(input, filename)
        }
        _ => TextParser.parse(input, filename),
    }
}
