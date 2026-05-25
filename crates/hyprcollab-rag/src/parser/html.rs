use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use html5ever::tendril::StrTendril;
use html5ever::tokenizer::{
    BufferQueue, CharacterTokens, EndTag, StartTag, TagToken, Token, TokenSink,
    TokenSinkResult, Tokenizer, TokenizerOpts,
};
use crate::{RagError, Result};
use super::{DocumentParser, ParsedDocument, Section};

pub struct HtmlParser;

impl DocumentParser for HtmlParser {
    fn mime_types(&self) -> &[&str] {
        &["text/html", "application/xhtml+xml"]
    }

    fn extensions(&self) -> &[&str] {
        &["html", "htm", "xhtml"]
    }

    fn parse(&self, input: &[u8], filename: Option<&str>) -> Result<ParsedDocument> {
        let text = std::str::from_utf8(input)
            .map_err(|e| RagError::Encoding(e.to_string()))?;

        // Share state between the sink and this function via Rc<RefCell>.
        // We borrow the state after tokenization rather than try_unwrap, so the
        // Tokenizer lifetime (whether end() takes self or &mut self) doesn't matter.
        let shared = Rc::new(RefCell::new(HtmlState::new()));

        {
            let sink = HtmlSink { state: Rc::clone(&shared) };
            let input_buf = BufferQueue::default();
            input_buf.push_back(StrTendril::from(text));
            let tok = Tokenizer::new(sink, TokenizerOpts::default());
            let _ = tok.feed(&input_buf);
            tok.end();
        } // tok, sink, input_buf all dropped here

        shared.borrow_mut().flush_section();
        build_document_ref(&shared.borrow(), filename)
    }
}

// ── internal state ────────────────────────────────────────────────────────────

struct HtmlState {
    title: Option<String>,
    sections: Vec<Section>,
    metadata: HashMap<String, String>,
    in_title: bool,
    skip_depth: u32,
    heading_level: Option<u8>,
    heading_text: String,
    pending_heading: Option<(String, u8)>,
    section_content: String,
}

impl HtmlState {
    fn new() -> Self {
        Self {
            title: None,
            sections: Vec::new(),
            metadata: HashMap::new(),
            in_title: false,
            skip_depth: 0,
            heading_level: None,
            heading_text: String::new(),
            pending_heading: None,
            section_content: String::new(),
        }
    }

    fn flush_section(&mut self) {
        let content = self.section_content.trim().to_string();
        if content.is_empty() && self.pending_heading.is_none() {
            return;
        }
        let (heading, level) = match self.pending_heading.take() {
            Some((h, l)) => (Some(h), l),
            None => (None, 0),
        };
        self.sections.push(Section {
            heading,
            content,
            level,
            start_line: 0,
            end_line: 0,
        });
        self.section_content.clear();
    }
}

// ── tokenizer sink ────────────────────────────────────────────────────────────

struct HtmlSink {
    state: Rc<RefCell<HtmlState>>,
}

impl TokenSink for HtmlSink {
    type Handle = ();

    fn process_token(&self, token: Token, _line_number: u64) -> TokenSinkResult<()> {
        let mut st = self.state.borrow_mut();

        match token {
            TagToken(tag) => {
                let name = tag.name.as_ref().to_lowercase();
                match tag.kind {
                    StartTag => match name.as_str() {
                        "script" | "style" => st.skip_depth += 1,
                        "title" => st.in_title = true,
                        "meta" => {
                            let mut n = None;
                            let mut c = None;
                            for attr in &tag.attrs {
                                match attr.name.local.as_ref() {
                                    "name" => n = Some(attr.value.as_ref().to_string()),
                                    "content" => c = Some(attr.value.as_ref().to_string()),
                                    _ => {}
                                }
                            }
                            if let (Some(k), Some(v)) = (n, c) {
                                st.metadata.insert(k, v);
                            }
                        }
                        h @ ("h1" | "h2" | "h3" | "h4" | "h5" | "h6") => {
                            st.flush_section();
                            let lvl = h.chars().nth(1)
                                .and_then(|c| c.to_digit(10))
                                .unwrap_or(1) as u8;
                            st.heading_level = Some(lvl);
                            st.heading_text.clear();
                        }
                        "p" | "div" | "li" | "br"
                            if st.skip_depth == 0 && st.heading_level.is_none() =>
                        {
                            st.section_content.push('\n');
                        }
                        _ => {}
                    },
                    EndTag => match name.as_str() {
                        "script" | "style" => {
                            st.skip_depth = st.skip_depth.saturating_sub(1);
                        }
                        "title" => st.in_title = false,
                        h @ ("h1" | "h2" | "h3" | "h4" | "h5" | "h6") => {
                            let _ = h;
                            if let Some(level) = st.heading_level.take() {
                                let heading = st.heading_text.trim().to_string();
                                if st.title.is_none() && level == 1 {
                                    st.title = Some(heading.clone());
                                }
                                st.pending_heading = Some((heading, level));
                            }
                        }
                        _ => {}
                    },
                }
            }
            CharacterTokens(s) => {
                if st.skip_depth > 0 {
                    return TokenSinkResult::Continue;
                }
                let text = s.as_ref();
                if st.in_title {
                    if st.title.is_none() {
                        let t = text.trim().to_string();
                        if !t.is_empty() {
                            st.title = Some(t);
                        }
                    }
                } else if st.heading_level.is_some() {
                    st.heading_text.push_str(text);
                } else {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        if !st.section_content.is_empty()
                            && !st.section_content.ends_with('\n')
                        {
                            st.section_content.push(' ');
                        }
                        st.section_content.push_str(trimmed);
                    }
                }
            }
            _ => {}
        }

        TokenSinkResult::Continue
    }
}

// ── document assembly ─────────────────────────────────────────────────────────

fn build_document_ref(state: &HtmlState, filename: Option<&str>) -> Result<ParsedDocument> {
    let content = state
        .sections
        .iter()
        .map(|s| match &s.heading {
            Some(h) => format!("{}\n{}", h, s.content),
            None => s.content.clone(),
        })
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    let title = state.title.clone().or_else(|| {
        filename.and_then(|f| {
            std::path::Path::new(f)
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        })
    });

    let mut metadata = state.metadata.clone();
    metadata.insert("parser".to_string(), "html".to_string());

    Ok(ParsedDocument {
        title,
        content,
        mime_type: "text/html".to_string(),
        sections: state.sections.clone(),
        metadata,
    })
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::DocumentParser;

    fn parser() -> HtmlParser { HtmlParser }

    #[test]
    fn title_tag_extracted() {
        let html = b"<html><head><title>My Page</title></head><body></body></html>";
        let doc = parser().parse(html, None).unwrap();
        assert_eq!(doc.title.as_deref(), Some("My Page"));
    }

    #[test]
    fn h1_becomes_title_fallback() {
        let html = b"<html><body><h1>Main Heading</h1><p>content</p></body></html>";
        let doc = parser().parse(html, None).unwrap();
        assert_eq!(doc.title.as_deref(), Some("Main Heading"));
    }

    #[test]
    fn headings_create_sections() {
        let html = b"<html><body><h1>A</h1><p>para1</p><h2>B</h2><p>para2</p></body></html>";
        let doc = parser().parse(html, None).unwrap();
        assert!(doc.sections.len() >= 2);
    }

    #[test]
    fn meta_tags_in_metadata() {
        let html =
            b"<html><head><meta name=\"author\" content=\"Alice\"></head><body></body></html>";
        let doc = parser().parse(html, None).unwrap();
        assert_eq!(doc.metadata.get("author").map(String::as_str), Some("Alice"));
    }

    #[test]
    fn script_content_stripped() {
        let html =
            b"<html><body><script>evil()</script><p>Clean text.</p></body></html>";
        let doc = parser().parse(html, None).unwrap();
        assert!(!doc.content.contains("evil"));
        assert!(doc.content.contains("Clean text"));
    }

    #[test]
    fn style_content_stripped() {
        let html = b"<html><head><style>.x{color:red}</style></head><body><p>Hi</p></body></html>";
        let doc = parser().parse(html, None).unwrap();
        assert!(!doc.content.contains("color"));
    }

    #[test]
    fn mime_type_is_html() {
        let doc = parser().parse(b"<p>hi</p>", None).unwrap();
        assert_eq!(doc.mime_type, "text/html");
    }

    #[test]
    fn filename_fallback_when_no_title() {
        let doc = parser().parse(b"<p>no title</p>", Some("about.html")).unwrap();
        assert_eq!(doc.title.as_deref(), Some("about"));
    }
}
