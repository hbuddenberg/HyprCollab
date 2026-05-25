use std::collections::HashMap;
use std::path::Path;
use crate::{RagError, Result};
use super::{DocumentParser, ParsedDocument, Section};

pub struct TextParser;

impl DocumentParser for TextParser {
    fn mime_types(&self) -> &[&str] {
        &["text/plain", "text/csv", "application/json", "application/csv"]
    }

    fn extensions(&self) -> &[&str] {
        &["txt", "text", "csv", "json"]
    }

    fn parse(&self, input: &[u8], filename: Option<&str>) -> Result<ParsedDocument> {
        let ext = filename
            .and_then(|f| Path::new(f).extension())
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        match ext.as_deref() {
            Some("csv") => parse_csv(input, filename),
            Some("json") => parse_json(input, filename),
            _ => parse_plain(input, filename),
        }
    }
}

// ── plain text ────────────────────────────────────────────────────────────────

fn parse_plain(input: &[u8], filename: Option<&str>) -> Result<ParsedDocument> {
    let text = std::str::from_utf8(input)
        .map_err(|e| RagError::Encoding(e.to_string()))?;

    let mut sections: Vec<Section> = Vec::new();
    let mut line_num = 0usize;

    for para in text.split("\n\n") {
        let content = para.trim().to_string();
        if content.is_empty() {
            line_num += 2;
            continue;
        }
        let para_lines = content.chars().filter(|&c| c == '\n').count() + 1;
        sections.push(Section {
            heading: None,
            content,
            level: 0,
            start_line: line_num,
            end_line: line_num + para_lines,
        });
        line_num += para_lines + 1;
    }

    let title = stem(filename);
    let mut metadata = HashMap::new();
    metadata.insert("parser".to_string(), "text".to_string());

    Ok(ParsedDocument {
        title,
        content: text.to_string(),
        mime_type: "text/plain".to_string(),
        sections,
        metadata,
    })
}

// ── CSV ───────────────────────────────────────────────────────────────────────

fn parse_csv(input: &[u8], filename: Option<&str>) -> Result<ParsedDocument> {
    let mut rdr = csv::Reader::from_reader(input);

    let headers: Vec<String> = rdr
        .headers()?
        .iter()
        .map(|s| s.to_string())
        .collect();

    let mut rows: Vec<Vec<String>> = Vec::new();
    for result in rdr.records() {
        let record = result?;
        rows.push(record.iter().map(|s| s.to_string()).collect());
    }

    let header_line = headers.join(",");
    let data_lines: Vec<String> = rows.iter().map(|r| r.join(",")).collect();
    let content = std::iter::once(header_line.as_str())
        .chain(data_lines.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join("\n");

    let mut sections = vec![Section {
        heading: Some("Headers".to_string()),
        content: header_line,
        level: 1,
        start_line: 0,
        end_line: 1,
    }];

    if !rows.is_empty() {
        sections.push(Section {
            heading: Some("Data".to_string()),
            content: data_lines.join("\n"),
            level: 1,
            start_line: 1,
            end_line: rows.len() + 1,
        });
    }

    let mut metadata = HashMap::new();
    metadata.insert("parser".to_string(), "csv".to_string());
    metadata.insert("columns".to_string(), headers.join(", "));
    metadata.insert("row_count".to_string(), rows.len().to_string());

    Ok(ParsedDocument {
        title: stem(filename),
        content,
        mime_type: "text/csv".to_string(),
        sections,
        metadata,
    })
}

// ── JSON ──────────────────────────────────────────────────────────────────────

fn parse_json(input: &[u8], filename: Option<&str>) -> Result<ParsedDocument> {
    let value: serde_json::Value = serde_json::from_slice(input)?;
    let content = serde_json::to_string_pretty(&value)?;

    let sections = if let serde_json::Value::Object(ref map) = value {
        let mut out = Vec::new();
        let mut line_num = 0usize;
        for (key, val) in map {
            let val_str = serde_json::to_string_pretty(val)?;
            let val_lines = val_str.chars().filter(|&c| c == '\n').count() + 1;
            out.push(Section {
                heading: Some(key.clone()),
                content: val_str,
                level: 1,
                start_line: line_num,
                end_line: line_num + val_lines,
            });
            line_num += val_lines + 1;
        }
        out
    } else {
        vec![Section {
            heading: None,
            content: content.clone(),
            level: 0,
            start_line: 0,
            end_line: content.lines().count(),
        }]
    };

    let mut metadata = HashMap::new();
    metadata.insert("parser".to_string(), "json".to_string());

    Ok(ParsedDocument {
        title: stem(filename),
        content,
        mime_type: "application/json".to_string(),
        sections,
        metadata,
    })
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn stem(filename: Option<&str>) -> Option<String> {
    filename.and_then(|f| {
        Path::new(f)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
    })
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::DocumentParser;

    fn parser() -> TextParser { TextParser }

    // ── plain text ───────────────────────────────────────────────────────

    #[test]
    fn plain_splits_on_double_newline() {
        let text = b"Para one.\n\nPara two.\n\nPara three.";
        let doc = parser().parse(text, Some("notes.txt")).unwrap();
        assert_eq!(doc.sections.len(), 3);
    }

    #[test]
    fn plain_single_paragraph() {
        let text = b"Just one paragraph.";
        let doc = parser().parse(text, None).unwrap();
        assert_eq!(doc.sections.len(), 1);
        assert_eq!(doc.sections[0].content, "Just one paragraph.");
    }

    #[test]
    fn plain_empty_input() {
        let doc = parser().parse(b"", None).unwrap();
        assert!(doc.sections.is_empty());
    }

    #[test]
    fn plain_mime_type() {
        let doc = parser().parse(b"hello", None).unwrap();
        assert_eq!(doc.mime_type, "text/plain");
    }

    // ── CSV ──────────────────────────────────────────────────────────────

    #[test]
    fn csv_headers_extracted() {
        let csv = b"name,age,city\nAlice,30,Berlin\nBob,25,London\n";
        let doc = parser().parse(csv, Some("data.csv")).unwrap();
        assert_eq!(doc.metadata.get("columns").map(String::as_str), Some("name, age, city"));
        assert_eq!(doc.metadata.get("row_count").map(String::as_str), Some("2"));
    }

    #[test]
    fn csv_sections_headers_and_data() {
        let csv = b"x,y\n1,2\n3,4\n";
        let doc = parser().parse(csv, Some("points.csv")).unwrap();
        assert_eq!(doc.sections.len(), 2);
        assert_eq!(doc.sections[0].heading.as_deref(), Some("Headers"));
        assert_eq!(doc.sections[1].heading.as_deref(), Some("Data"));
    }

    #[test]
    fn csv_mime_type() {
        let doc = parser().parse(b"a,b\n1,2\n", Some("f.csv")).unwrap();
        assert_eq!(doc.mime_type, "text/csv");
    }

    #[test]
    fn csv_headers_only_no_data_section() {
        let csv = b"col1,col2\n";
        let doc = parser().parse(csv, Some("empty.csv")).unwrap();
        // Only the Headers section, no Data section.
        assert_eq!(doc.sections.len(), 1);
    }

    // ── JSON ─────────────────────────────────────────────────────────────

    #[test]
    fn json_object_sections_per_key() {
        let json = b"{\"name\":\"Alice\",\"age\":30}";
        let doc = parser().parse(json, Some("user.json")).unwrap();
        assert!(doc.sections.iter().any(|s| s.heading.as_deref() == Some("name")));
        assert!(doc.sections.iter().any(|s| s.heading.as_deref() == Some("age")));
    }

    #[test]
    fn json_array_single_section() {
        let json = b"[1,2,3]";
        let doc = parser().parse(json, Some("nums.json")).unwrap();
        assert_eq!(doc.sections.len(), 1);
        assert!(doc.sections[0].heading.is_none());
    }

    #[test]
    fn json_invalid_returns_error() {
        let result = parser().parse(b"{invalid}", Some("bad.json"));
        assert!(result.is_err());
    }

    #[test]
    fn json_mime_type() {
        let doc = parser().parse(b"{}", Some("f.json")).unwrap();
        assert_eq!(doc.mime_type, "application/json");
    }
}
