use std::collections::HashMap;
use crate::{RagError, Result};
use super::{DocumentParser, ParsedDocument, Section};

pub struct PdfParser;

impl DocumentParser for PdfParser {
    fn mime_types(&self) -> &[&str] {
        &["application/pdf"]
    }

    fn extensions(&self) -> &[&str] {
        &["pdf"]
    }

    fn parse(&self, input: &[u8], filename: Option<&str>) -> Result<ParsedDocument> {
        let doc = lopdf::Document::load_mem(input)
            .map_err(|e| RagError::Pdf(e.to_string()))?;

        let mut page_nums: Vec<u32> = doc.get_pages().keys().cloned().collect();
        page_nums.sort_unstable();

        let chunks = doc.extract_text_chunks(&page_nums);

        let mut sections: Vec<Section> = Vec::new();
        let mut all_text: Vec<String> = Vec::new();

        for (i, result) in chunks.iter().enumerate() {
            let page = page_nums.get(i).copied().unwrap_or((i + 1) as u32);
            let content = match result {
                Ok(text) => text.trim().to_string(),
                Err(e) => {
                    tracing::warn!("PDF page {} extraction failed: {}", page, e);
                    String::new()
                }
            };
            if !content.is_empty() {
                all_text.push(content.clone());
                sections.push(Section {
                    heading: Some(format!("Page {}", page)),
                    content,
                    level: 1,
                    start_line: 0,
                    end_line: 0,
                });
            }
        }

        let title = filename.and_then(|f| {
            std::path::Path::new(f)
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        });

        let mut metadata = HashMap::new();
        metadata.insert("parser".to_string(), "pdf".to_string());
        metadata.insert("pages".to_string(), page_nums.len().to_string());

        Ok(ParsedDocument {
            title,
            content: all_text.join("\n\n"),
            mime_type: "application/pdf".to_string(),
            sections,
            metadata,
        })
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::DocumentParser;

    fn parser() -> PdfParser { PdfParser }

    #[test]
    fn invalid_bytes_return_error() {
        let result = parser().parse(b"not a pdf", None);
        assert!(result.is_err());
    }

    #[test]
    fn empty_bytes_return_error() {
        let result = parser().parse(b"", None);
        assert!(result.is_err());
    }

    #[test]
    fn mime_type_is_pdf() {
        // Can't easily create a real PDF in a unit test; just verify the
        // parser advertises the right mime type.
        assert!(parser().mime_types().contains(&"application/pdf"));
    }

    #[test]
    fn extension_is_pdf() {
        assert!(parser().extensions().contains(&"pdf"));
    }

    #[test]
    fn error_message_is_non_empty_on_bad_input() {
        let err = parser().parse(b"%not-pdf", None).unwrap_err();
        assert!(!err.to_string().is_empty());
    }
}
