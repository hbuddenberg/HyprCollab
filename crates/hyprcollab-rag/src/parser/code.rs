use std::collections::HashMap;
use crate::{RagError, Result};
use super::{DocumentParser, ParsedDocument, Section};

const CHUNK_LINES: usize = 50;
const OVERLAP_LINES: usize = 5;

pub struct CodeParser;

impl DocumentParser for CodeParser {
    fn mime_types(&self) -> &[&str] {
        &[
            "text/x-rust",
            "text/x-python",
            "application/javascript",
            "text/javascript",
            "text/typescript",
            "text/x-go",
            "text/x-java-source",
            "text/x-csrc",
            "text/x-c++src",
        ]
    }

    fn extensions(&self) -> &[&str] {
        &[
            "rs", "py", "js", "ts", "go", "java", "c", "cpp", "cc", "h", "hpp",
            "toml", "yaml", "yml",
        ]
    }

    fn parse(&self, input: &[u8], filename: Option<&str>) -> Result<ParsedDocument> {
        let text = std::str::from_utf8(input)
            .map_err(|e| RagError::Encoding(e.to_string()))?;

        let lang = detect_language(filename);
        let lines: Vec<&str> = text.lines().collect();
        let total = lines.len();

        let sections = if total <= CHUNK_LINES {
            vec![Section {
                heading: None,
                content: text.to_string(),
                level: 0,
                start_line: 0,
                end_line: total,
            }]
        } else {
            build_chunks(&lines)
        };

        let title = filename.and_then(|f| {
            std::path::Path::new(f)
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        });

        let mime = filename
            .map(|f| mime_guess::from_path(f).first_or_octet_stream().to_string())
            .unwrap_or_else(|| "text/plain".to_string());

        let mut metadata = HashMap::new();
        metadata.insert("language".to_string(), lang.to_string());
        if let Some(f) = filename {
            metadata.insert("filename".to_string(), f.to_string());
        }

        Ok(ParsedDocument {
            title,
            content: text.to_string(),
            mime_type: mime,
            sections,
            metadata,
        })
    }
}

fn detect_language(filename: Option<&str>) -> &'static str {
    let ext = filename
        .and_then(|f| std::path::Path::new(f).extension())
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "ts" => "typescript",
        "go" => "go",
        "java" => "java",
        "c" => "c",
        "cpp" | "cc" | "cxx" => "cpp",
        "h" | "hpp" => "c_or_cpp",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "json" => "json",
        _ => "text",
    }
}

fn build_chunks(lines: &[&str]) -> Vec<Section> {
    let mut sections = Vec::new();
    let total = lines.len();
    let mut start = 0;
    let mut chunk = 1;

    while start < total {
        let end = (start + CHUNK_LINES).min(total);
        sections.push(Section {
            heading: Some(format!("Chunk {}", chunk)),
            content: lines[start..end].join("\n"),
            level: 1,
            start_line: start,
            end_line: end,
        });
        chunk += 1;
        if end >= total {
            break;
        }
        start = end.saturating_sub(OVERLAP_LINES);
    }

    sections
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::DocumentParser;

    fn parser() -> CodeParser { CodeParser }

    #[test]
    fn language_rust_detected() {
        assert_eq!(detect_language(Some("main.rs")), "rust");
    }

    #[test]
    fn language_python_detected() {
        assert_eq!(detect_language(Some("script.py")), "python");
    }

    #[test]
    fn language_go_detected() {
        assert_eq!(detect_language(Some("server.go")), "go");
    }

    #[test]
    fn short_file_single_section() {
        let code = b"fn main() {\n    println!(\"hello\");\n}";
        let doc = parser().parse(code, Some("main.rs")).unwrap();
        assert_eq!(doc.sections.len(), 1);
        assert!(doc.sections[0].heading.is_none());
    }

    #[test]
    fn long_file_multiple_chunks() {
        let line = "let x = 1;\n";
        let code: Vec<u8> = line.repeat(200).into_bytes();
        let doc = parser().parse(&code, Some("many.rs")).unwrap();
        assert!(doc.sections.len() > 1, "expected multiple chunks");
    }

    #[test]
    fn chunks_have_overlap() {
        let lines: Vec<String> = (0..120).map(|i| format!("line{}", i)).collect();
        let code = lines.join("\n").into_bytes();
        let doc = parser().parse(&code, Some("big.rs")).unwrap();
        // Second chunk should start before line 50 (overlap)
        if doc.sections.len() >= 2 {
            assert!(doc.sections[1].start_line < CHUNK_LINES);
        }
    }

    #[test]
    fn metadata_contains_language() {
        let doc = parser().parse(b"x = 1", Some("script.py")).unwrap();
        assert_eq!(doc.metadata.get("language").map(String::as_str), Some("python"));
    }

    #[test]
    fn title_is_filename() {
        let doc = parser().parse(b"fn foo() {}", Some("lib.rs")).unwrap();
        assert_eq!(doc.title.as_deref(), Some("lib.rs"));
    }

    #[test]
    fn invalid_utf8_returns_error() {
        let result = parser().parse(b"\xff\xfe invalid", Some("bad.rs"));
        assert!(result.is_err());
    }
}
