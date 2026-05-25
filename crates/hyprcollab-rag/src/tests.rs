use crate::parser::{self, DocumentParser};
use crate::parser::{CodeParser, HtmlParser, MarkdownParser, PdfParser, TextParser};

// ── dispatcher ────────────────────────────────────────────────────────────────

#[test]
fn dispatcher_md_extension() {
    let md = b"# Title\n\nContent.";
    let doc = parser::parse(md, Some("readme.md")).unwrap();
    assert_eq!(doc.mime_type, "text/markdown");
}

#[test]
fn dispatcher_markdown_extension() {
    let md = b"# Title\n\nContent.";
    let doc = parser::parse(md, Some("readme.markdown")).unwrap();
    assert_eq!(doc.mime_type, "text/markdown");
}

#[test]
fn dispatcher_html_extension() {
    let html = b"<html><body><p>hi</p></body></html>";
    let doc = parser::parse(html, Some("page.html")).unwrap();
    assert_eq!(doc.mime_type, "text/html");
}

#[test]
fn dispatcher_htm_extension() {
    let html = b"<p>hi</p>";
    let doc = parser::parse(html, Some("old.htm")).unwrap();
    assert_eq!(doc.mime_type, "text/html");
}

#[test]
fn dispatcher_rs_extension() {
    let code = b"fn main() {}";
    let doc = parser::parse(code, Some("main.rs")).unwrap();
    assert_eq!(doc.metadata.get("language").map(String::as_str), Some("rust"));
}

#[test]
fn dispatcher_py_extension() {
    let code = b"print('hello')";
    let doc = parser::parse(code, Some("hello.py")).unwrap();
    assert_eq!(doc.metadata.get("language").map(String::as_str), Some("python"));
}

#[test]
fn dispatcher_ts_extension() {
    let code = b"const x: number = 1;";
    let doc = parser::parse(code, Some("app.ts")).unwrap();
    assert_eq!(doc.metadata.get("language").map(String::as_str), Some("typescript"));
}

#[test]
fn dispatcher_toml_extension() {
    let code = b"[package]\nname = \"foo\"";
    let doc = parser::parse(code, Some("Cargo.toml")).unwrap();
    assert_eq!(doc.metadata.get("language").map(String::as_str), Some("toml"));
}

#[test]
fn dispatcher_csv_extension() {
    let csv = b"a,b\n1,2\n";
    let doc = parser::parse(csv, Some("data.csv")).unwrap();
    assert_eq!(doc.mime_type, "text/csv");
}

#[test]
fn dispatcher_json_extension() {
    let json = b"{\"key\":\"value\"}";
    let doc = parser::parse(json, Some("config.json")).unwrap();
    assert_eq!(doc.mime_type, "application/json");
}

#[test]
fn dispatcher_txt_extension() {
    let text = b"Hello world.";
    let doc = parser::parse(text, Some("notes.txt")).unwrap();
    assert_eq!(doc.mime_type, "text/plain");
}

#[test]
fn dispatcher_unknown_extension_falls_back_to_text() {
    let text = b"some content";
    let doc = parser::parse(text, Some("file.xyz")).unwrap();
    assert_eq!(doc.mime_type, "text/plain");
}

#[test]
fn dispatcher_no_filename_falls_back_to_text() {
    let doc = parser::parse(b"something", None).unwrap();
    assert_eq!(doc.mime_type, "text/plain");
}

// ── MarkdownParser integration ────────────────────────────────────────────────

#[test]
fn markdown_full_document() {
    let md = b"# Introduction\n\nFirst para.\n\n## Details\n\nSecond para.\n\n```rust\nfn foo() {}\n```\n";
    let doc = MarkdownParser.parse(md, Some("doc.md")).unwrap();
    assert_eq!(doc.title.as_deref(), Some("Introduction"));
    assert!(doc.sections.len() >= 2);
    assert!(doc.content.contains("First para"));
    assert!(doc.content.contains("Second para"));
}

#[test]
fn markdown_nested_headings() {
    let md = b"# H1\n## H2\n### H3";
    let doc = MarkdownParser.parse(md, None).unwrap();
    let levels: Vec<u8> = doc.sections.iter().map(|s| s.level).collect();
    assert!(levels.contains(&1));
    assert!(levels.contains(&2));
    assert!(levels.contains(&3));
}

// ── HtmlParser integration ────────────────────────────────────────────────────

#[test]
fn html_full_document() {
    let html = b"<!DOCTYPE html><html><head><title>Test</title><meta name=\"description\" content=\"hello\"></head><body><h1>Main</h1><p>Body text here.</p><h2>Sub</h2><p>More text.</p></body></html>";
    let doc = HtmlParser.parse(html, None).unwrap();
    assert_eq!(doc.title.as_deref(), Some("Test"));
    assert!(doc.sections.len() >= 2);
    assert!(doc.metadata.contains_key("description"));
}

#[test]
fn html_strips_scripts_completely() {
    let html = b"<html><body><script>document.write('x')</script><p>clean</p></body></html>";
    let doc = HtmlParser.parse(html, None).unwrap();
    assert!(!doc.content.contains("document.write"));
}

// ── TextParser integration ────────────────────────────────────────────────────

#[test]
fn text_multiline_paragraphs() {
    let text = b"First paragraph\nstill first.\n\nSecond paragraph.";
    let doc = TextParser.parse(text, None).unwrap();
    assert_eq!(doc.sections.len(), 2);
}

#[test]
fn csv_full_parse() {
    let csv = b"id,name,score\n1,Alice,95\n2,Bob,87\n3,Carol,92\n";
    let doc = TextParser.parse(csv, Some("results.csv")).unwrap();
    assert_eq!(doc.title.as_deref(), Some("results"));
    assert!(doc.metadata.get("row_count").map(String::as_str) == Some("3"));
    assert!(doc.content.contains("Alice"));
}

#[test]
fn json_nested_object() {
    let json = b"{\"user\":{\"name\":\"Alice\",\"age\":30},\"active\":true}";
    let doc = TextParser.parse(json, Some("data.json")).unwrap();
    assert!(doc.sections.iter().any(|s| s.heading.as_deref() == Some("user")));
    assert!(doc.sections.iter().any(|s| s.heading.as_deref() == Some("active")));
}

// ── CodeParser integration ────────────────────────────────────────────────────

#[test]
fn code_rust_file() {
    let code = b"use std::collections::HashMap;\n\nfn main() {\n    let mut m = HashMap::new();\n    m.insert(\"key\", 1);\n}\n";
    let doc = CodeParser.parse(code, Some("main.rs")).unwrap();
    assert_eq!(doc.metadata.get("language").map(String::as_str), Some("rust"));
    assert!(doc.content.contains("HashMap"));
}

#[test]
fn code_yaml_file() {
    let yaml = b"version: \"3\"\nservices:\n  web:\n    image: nginx\n";
    let doc = CodeParser.parse(yaml, Some("docker-compose.yml")).unwrap();
    assert_eq!(doc.metadata.get("language").map(String::as_str), Some("yaml"));
}

// ── PdfParser integration ─────────────────────────────────────────────────────

#[test]
fn pdf_error_on_random_bytes() {
    let result = PdfParser.parse(b"\x00\x01\x02\x03garbage", None);
    assert!(result.is_err());
}
