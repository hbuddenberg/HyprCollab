use crate::parser::ParsedDocument;
use super::{Chunk, ChunkMetadata, ChunkerConfig};

/// Split a [`ParsedDocument`] into [`Chunk`]s using a recursive character splitter.
pub fn chunk_document(doc: &ParsedDocument, config: &ChunkerConfig) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut idx = 0usize;

    if !doc.sections.is_empty() {
        let source = doc.metadata.get("source_file").map(String::as_str);
        for section in &doc.sections {
            let texts = split_text(&section.content, config);
            for content in texts {
                if content.trim().is_empty() {
                    continue;
                }
                chunks.push(Chunk {
                    id: uuid::Uuid::new_v4().to_string(),
                    content,
                    metadata: ChunkMetadata {
                        source_file: source.map(String::from),
                        section_heading: section.heading.clone(),
                        start_line: section.start_line,
                        end_line: section.end_line,
                        chunk_index: idx,
                        mime_type: doc.mime_type.clone(),
                    },
                });
                idx += 1;
            }
        }
    } else {
        let source = doc.metadata.get("source_file").map(String::as_str);
        let texts = split_text(&doc.content, config);
        let line_count = doc.content.lines().count();
        for content in texts {
            if content.trim().is_empty() {
                continue;
            }
            chunks.push(Chunk {
                id: uuid::Uuid::new_v4().to_string(),
                content,
                metadata: ChunkMetadata {
                    source_file: source.map(String::from),
                    section_heading: doc.title.clone(),
                    start_line: 0,
                    end_line: line_count,
                    chunk_index: idx,
                    mime_type: doc.mime_type.clone(),
                },
            });
            idx += 1;
        }
    }

    chunks
}

fn split_text(text: &str, config: &ChunkerConfig) -> Vec<String> {
    if text.trim().is_empty() {
        return vec![];
    }
    // Build separator hierarchy: configured separator first, then fallbacks.
    let mut seps: Vec<&str> = vec![config.separator.as_str()];
    for s in ["\n\n", "\n", ". ", " "] {
        if !seps.contains(&s) {
            seps.push(s);
        }
    }
    recursive_split(text, &seps, config.chunk_size, config.overlap)
}

fn recursive_split(text: &str, separators: &[&str], chunk_size: usize, overlap: usize) -> Vec<String> {
    if text.trim().is_empty() {
        return vec![];
    }
    if text.len() <= chunk_size {
        return vec![text.to_string()];
    }

    // Find first separator present in text.
    let mut chosen_sep: Option<&str> = None;
    let mut tail_seps: &[&str] = &[];
    for (i, &sep) in separators.iter().enumerate() {
        if text.contains(sep) {
            chosen_sep = Some(sep);
            tail_seps = &separators[i + 1..];
            break;
        }
    }

    let sep = match chosen_sep {
        Some(s) => s,
        None => {
            // No separator found; force-split by bytes at chunk_size boundaries.
            return force_split(text, chunk_size);
        }
    };

    let raw: Vec<&str> = text.split(sep).filter(|s| !s.trim().is_empty()).collect();
    let mut good: Vec<String> = Vec::new();
    let mut final_chunks: Vec<String> = Vec::new();

    for piece in raw {
        if piece.len() < chunk_size {
            good.push(piece.to_string());
        } else {
            if !good.is_empty() {
                final_chunks.extend(merge_splits(&good, sep, chunk_size, overlap));
                good.clear();
            }
            if tail_seps.is_empty() {
                final_chunks.extend(force_split(piece, chunk_size));
            } else {
                final_chunks.extend(recursive_split(piece, tail_seps, chunk_size, overlap));
            }
        }
    }

    if !good.is_empty() {
        final_chunks.extend(merge_splits(&good, sep, chunk_size, overlap));
    }

    final_chunks
}

/// Merge splits into chunks of at most `chunk_size` bytes, keeping `overlap` bytes of context.
fn merge_splits(splits: &[String], sep: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    let mut docs: Vec<String> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut total: usize = 0;

    for d in splits {
        let d_len = d.len();
        let sep_cost = if current.is_empty() { 0 } else { sep.len() };

        if !current.is_empty() && total + sep_cost + d_len > chunk_size {
            let joined = current.join(sep);
            if !joined.trim().is_empty() {
                docs.push(joined);
            }
            // Drop from front until total <= overlap.
            while total > overlap && !current.is_empty() {
                let removed = current[0].len();
                total = total.saturating_sub(removed);
                if current.len() > 1 {
                    total = total.saturating_sub(sep.len());
                }
                current.remove(0);
            }
        }

        let add_sep = if current.is_empty() { 0 } else { sep.len() };
        total += d_len + add_sep;
        current.push(d.clone());
    }

    if !current.is_empty() {
        let joined = current.join(sep);
        if !joined.trim().is_empty() {
            docs.push(joined);
        }
    }

    docs
}

fn force_split(text: &str, chunk_size: usize) -> Vec<String> {
    // Split at char boundaries near chunk_size.
    let mut result = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    while start < bytes.len() {
        let end = (start + chunk_size).min(bytes.len());
        // Walk back to a char boundary.
        let mut boundary = end;
        while boundary > start && !text.is_char_boundary(boundary) {
            boundary -= 1;
        }
        if boundary == start {
            boundary = end; // fallback: slice anyway
        }
        result.push(text[start..boundary].to_string());
        start = boundary;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{ParsedDocument, Section};
    use std::collections::HashMap;

    fn make_doc(content: &str, sections: Vec<Section>) -> ParsedDocument {
        ParsedDocument {
            title: Some("Test".to_string()),
            content: content.to_string(),
            mime_type: "text/plain".to_string(),
            sections,
            metadata: HashMap::new(),
        }
    }

    fn default_cfg() -> ChunkerConfig {
        ChunkerConfig::default()
    }

    #[test]
    fn chunker_empty_doc_no_chunks() {
        let doc = make_doc("", vec![]);
        let chunks = chunk_document(&doc, &default_cfg());
        assert!(chunks.is_empty());
    }

    #[test]
    fn chunker_small_doc_single_chunk() {
        let doc = make_doc("Hello world.", vec![]);
        let chunks = chunk_document(&doc, &default_cfg());
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, "Hello world.");
    }

    #[test]
    fn chunker_large_doc_splits_into_multiple() {
        let long = "x".repeat(3000);
        let doc = make_doc(&long, vec![]);
        let cfg = ChunkerConfig { chunk_size: 1000, overlap: 0, separator: "\n\n".into() };
        let chunks = chunk_document(&doc, &cfg);
        assert!(chunks.len() >= 2, "expected ≥2 chunks, got {}", chunks.len());
        for c in &chunks {
            assert!(c.content.len() <= 1000);
        }
    }

    #[test]
    fn chunker_overlap_produces_shared_content() {
        // 5 paragraphs of 100 chars each; with chunk_size=250 and overlap=150
        // consecutive chunks should share one paragraph.
        let para: String = "a".repeat(100);
        let text = [para.as_str(); 5].join("\n\n");
        let cfg = ChunkerConfig { chunk_size: 250, overlap: 150, separator: "\n\n".into() };
        let doc = make_doc(&text, vec![]);
        let chunks = chunk_document(&doc, &cfg);
        assert!(chunks.len() >= 2);
        // Verify consecutive chunks share content.
        let shared = chunks[0].content.lines().last().unwrap_or("").to_string();
        assert!(chunks[1].content.contains(shared.trim()));
    }

    #[test]
    fn chunker_double_newline_separator_preferred() {
        let text = "para1\n\npara2\n\npara3";
        let cfg = ChunkerConfig { chunk_size: 20, overlap: 0, separator: "\n\n".into() };
        let doc = make_doc(text, vec![]);
        let chunks = chunk_document(&doc, &cfg);
        // Should split at paragraph boundaries first.
        assert!(chunks.iter().any(|c| c.content.contains("para1")));
        assert!(chunks.iter().any(|c| c.content.contains("para2")));
    }

    #[test]
    fn chunker_uuid_ids_are_unique() {
        let text = "line\n".repeat(200);
        let doc = make_doc(&text, vec![]);
        let cfg = ChunkerConfig { chunk_size: 100, overlap: 0, separator: "\n".into() };
        let chunks = chunk_document(&doc, &cfg);
        let ids: std::collections::HashSet<_> = chunks.iter().map(|c| c.id.clone()).collect();
        assert_eq!(ids.len(), chunks.len(), "chunk IDs must be unique");
    }

    #[test]
    fn chunker_metadata_source_file() {
        let mut doc = make_doc("Content here.", vec![]);
        doc.metadata.insert("source_file".into(), "test.md".into());
        let chunks = chunk_document(&doc, &default_cfg());
        assert_eq!(chunks[0].metadata.source_file.as_deref(), Some("test.md"));
    }

    #[test]
    fn chunker_metadata_section_heading_from_section() {
        let sec = Section {
            heading: Some("Introduction".into()),
            content: "Some content here.".into(),
            level: 1,
            start_line: 0,
            end_line: 5,
        };
        let doc = make_doc("", vec![sec]);
        let chunks = chunk_document(&doc, &default_cfg());
        assert_eq!(chunks[0].metadata.section_heading.as_deref(), Some("Introduction"));
    }

    #[test]
    fn chunker_chunk_indices_are_sequential() {
        let text = "word ".repeat(500);
        let doc = make_doc(&text, vec![]);
        let cfg = ChunkerConfig { chunk_size: 200, overlap: 0, separator: " ".into() };
        let chunks = chunk_document(&doc, &cfg);
        assert!(chunks.len() >= 2);
        for (i, c) in chunks.iter().enumerate() {
            assert_eq!(c.metadata.chunk_index, i);
        }
    }

    #[test]
    fn chunker_respects_custom_separator() {
        let text = "part1|part2|part3";
        let cfg = ChunkerConfig { chunk_size: 10, overlap: 0, separator: "|".into() };
        let doc = make_doc(text, vec![]);
        let chunks = chunk_document(&doc, &cfg);
        assert!(chunks.iter().any(|c| c.content == "part1"));
        assert!(chunks.iter().any(|c| c.content == "part2"));
        assert!(chunks.iter().any(|c| c.content == "part3"));
    }

    #[test]
    fn chunker_sections_produce_per_section_chunks() {
        let sections = vec![
            Section { heading: Some("A".into()), content: "Content A.".into(), level: 1, start_line: 0, end_line: 2 },
            Section { heading: Some("B".into()), content: "Content B.".into(), level: 1, start_line: 2, end_line: 4 },
        ];
        let doc = make_doc("ignored", sections);
        let chunks = chunk_document(&doc, &default_cfg());
        assert_eq!(chunks.len(), 2);
        assert!(chunks.iter().any(|c| c.metadata.section_heading.as_deref() == Some("A")));
        assert!(chunks.iter().any(|c| c.metadata.section_heading.as_deref() == Some("B")));
    }
}
