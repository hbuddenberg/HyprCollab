use std::collections::HashMap;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use crate::{RagError, Result};
use super::{DocumentParser, ParsedDocument, Section};

pub struct MarkdownParser;

impl DocumentParser for MarkdownParser {
    fn mime_types(&self) -> &[&str] {
        &["text/markdown", "text/x-markdown"]
    }

    fn extensions(&self) -> &[&str] {
        &["md", "markdown", "mkd"]
    }

    fn parse(&self, input: &[u8], filename: Option<&str>) -> Result<ParsedDocument> {
        let text = std::str::from_utf8(input)
            .map_err(|e| RagError::Encoding(e.to_string()))?;
        parse_markdown(text, filename)
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn heading_u8(l: HeadingLevel) -> u8 {
    match l {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Accumulates a single section until it is flushed.
struct SectionAcc {
    heading: Option<(String, u8)>,
    content: String,
    start_line: usize,
}

impl SectionAcc {
    fn new(start: usize) -> Self {
        Self { heading: None, content: String::new(), start_line: start }
    }

    fn flush(
        &mut self,
        sections: &mut Vec<Section>,
        all_content: &mut Vec<String>,
        end_line: usize,
    ) {
        let content = self.content.trim().to_string();
        if content.is_empty() && self.heading.is_none() {
            return;
        }
        let (heading, level) = match self.heading.take() {
            Some((h, l)) => (Some(h), l),
            None => (None, 0),
        };
        if let Some(ref h) = heading {
            all_content.push(format!("{}\n{}", h, content));
        } else if !content.is_empty() {
            all_content.push(content.clone());
        }
        sections.push(Section {
            heading,
            content,
            level,
            start_line: self.start_line,
            end_line,
        });
        self.content.clear();
        self.start_line = end_line;
    }
}

// ── core parser ───────────────────────────────────────────────────────────────

fn parse_markdown(text: &str, filename: Option<&str>) -> Result<ParsedDocument> {
    let options = Options::all();

    let mut title: Option<String> = None;
    let mut sections: Vec<Section> = Vec::new();
    let mut all_content: Vec<String> = Vec::new();

    let mut acc = SectionAcc::new(0);

    let mut in_heading: Option<HeadingLevel> = None;
    let mut heading_text = String::new();
    let mut in_code_block = false;
    let mut code_lang: Option<String> = None;
    let mut code_buf = String::new();
    let mut line_num: usize = 0;

    for event in Parser::new_ext(text, options) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                acc.flush(&mut sections, &mut all_content, line_num);
                in_heading = Some(level);
                heading_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                let lvl = in_heading.take().map_or(1, heading_u8);
                let h = heading_text.trim().to_string();
                if title.is_none() && lvl == 1 {
                    title = Some(h.clone());
                }
                acc.heading = Some((h, lvl));
                acc.start_line = line_num;
                heading_text.clear();
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                code_lang = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        let s = lang.as_ref().to_string();
                        if s.is_empty() { None } else { Some(s) }
                    }
                    CodeBlockKind::Indented => None,
                };
                code_buf.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                let fence = code_lang.as_deref().unwrap_or("");
                acc.content.push_str(&format!("```{}\n{}```\n", fence, code_buf));
                code_buf.clear();
                code_lang = None;
            }
            Event::Text(t) => {
                let s = t.as_ref();
                line_num += s.chars().filter(|&c| c == '\n').count();
                if in_heading.is_some() {
                    heading_text.push_str(s);
                } else if in_code_block {
                    code_buf.push_str(s);
                } else {
                    acc.content.push_str(s);
                }
            }
            Event::Code(c) => {
                if in_heading.is_some() {
                    heading_text.push_str(c.as_ref());
                } else {
                    acc.content.push('`');
                    acc.content.push_str(c.as_ref());
                    acc.content.push('`');
                }
            }
            Event::SoftBreak | Event::HardBreak
                if in_heading.is_none() && !in_code_block =>
            {
                acc.content.push('\n');
            }
            _ => {}
        }
    }

    acc.flush(&mut sections, &mut all_content, line_num);

    if title.is_none() {
        title = filename.and_then(|f| {
            std::path::Path::new(f)
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        });
    }

    let mut metadata = HashMap::new();
    metadata.insert("parser".to_string(), "markdown".to_string());

    Ok(ParsedDocument {
        title,
        content: all_content.join("\n\n"),
        mime_type: "text/markdown".to_string(),
        sections,
        metadata,
    })
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::DocumentParser;

    fn parser() -> MarkdownParser { MarkdownParser }

    #[test]
    fn title_extracted_from_h1() {
        let md = b"# Hello World\n\nSome paragraph.";
        let doc = parser().parse(md, None).unwrap();
        assert_eq!(doc.title.as_deref(), Some("Hello World"));
    }

    #[test]
    fn fallback_title_from_filename() {
        let md = b"No heading here.";
        let doc = parser().parse(md, Some("notes.md")).unwrap();
        assert_eq!(doc.title.as_deref(), Some("notes"));
    }

    #[test]
    fn headings_create_sections() {
        let md = b"# Section A\n\nContent A.\n\n## Section B\n\nContent B.";
        let doc = parser().parse(md, None).unwrap();
        assert!(doc.sections.len() >= 2);
        let h1 = doc.sections.iter().find(|s| s.level == 1).unwrap();
        assert_eq!(h1.heading.as_deref(), Some("Section A"));
        let h2 = doc.sections.iter().find(|s| s.level == 2).unwrap();
        assert_eq!(h2.heading.as_deref(), Some("Section B"));
    }

    #[test]
    fn code_block_with_language_preserved() {
        let md = b"# Title\n\n```rust\nfn main() {}\n```\n";
        let doc = parser().parse(md, None).unwrap();
        let code_section = doc.sections.iter().find(|s| s.content.contains("fn main")).unwrap();
        assert!(code_section.content.contains("```rust"));
    }

    #[test]
    fn inline_code_backtick_preserved() {
        let md = b"Use `cargo build` to compile.";
        let doc = parser().parse(md, None).unwrap();
        assert!(doc.content.contains("`cargo build`"));
    }

    #[test]
    fn multiple_heading_levels() {
        let md = b"# H1\n\n## H2\n\n### H3\n\n#### H4\n\n##### H5\n\n###### H6";
        let doc = parser().parse(md, None).unwrap();
        let levels: Vec<u8> = doc.sections.iter().map(|s| s.level).collect();
        assert!(levels.contains(&1));
        assert!(levels.contains(&2));
        assert!(levels.contains(&3));
    }

    #[test]
    fn empty_input_returns_empty_doc() {
        let doc = parser().parse(b"", None).unwrap();
        assert!(doc.sections.is_empty());
        assert!(doc.content.is_empty());
    }

    #[test]
    fn mime_type_is_markdown() {
        let doc = parser().parse(b"hello", None).unwrap();
        assert_eq!(doc.mime_type, "text/markdown");
    }
}
