use anyhow::Result;
use async_trait::async_trait;
use hyprcollab_artifacts::Artifact;
use pulldown_cmark::{html, Options, Parser};

use crate::renderer::{ArtifactRenderer, RenderMode, RenderedArtifact};

const PREVIEW_CHARS: usize = 500;

pub struct MarkdownRenderer;

impl MarkdownRenderer {
    pub fn new() -> Self {
        Self
    }

    pub fn to_html(content: &str) -> String {
        let parser = Parser::new_ext(content, Options::all());
        let mut out = String::new();
        html::push_html(&mut out, parser);
        out
    }

    fn to_text(content: &str) -> String {
        content.to_string()
    }
}

impl Default for MarkdownRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ArtifactRenderer for MarkdownRenderer {
    async fn render(&self, artifact: &Artifact, mode: RenderMode) -> Result<RenderedArtifact> {
        let content = match artifact {
            Artifact::Markdown { content } => content.as_str(),
            _ => return Err(anyhow::anyhow!("MarkdownRenderer only handles Markdown artifacts")),
        };

        match mode {
            RenderMode::Html => Ok(RenderedArtifact::Html(Self::to_html(content))),
            RenderMode::Terminal => Ok(RenderedArtifact::Text(Self::to_text(content))),
            RenderMode::Preview => {
                let preview = if content.chars().count() > PREVIEW_CHARS {
                    let end = content
                        .char_indices()
                        .nth(PREVIEW_CHARS)
                        .map(|(i, _)| i)
                        .unwrap_or(content.len());
                    format!("{}…", &content[..end])
                } else {
                    content.to_string()
                };
                Ok(RenderedArtifact::Text(preview))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md(content: &str) -> Artifact {
        Artifact::Markdown { content: content.into() }
    }

    #[test]
    fn to_html_basic_paragraph() {
        let html = MarkdownRenderer::to_html("hello world");
        assert!(html.contains("<p>"), "should produce <p>");
        assert!(html.contains("hello world"));
    }

    #[test]
    fn to_html_heading() {
        let html = MarkdownRenderer::to_html("# Title\n\nBody");
        assert!(html.contains("<h1>") || html.contains("<h1 "), "should have h1");
    }

    #[test]
    fn to_html_table_enabled() {
        let html = MarkdownRenderer::to_html("| a | b |\n|---|---|\n| 1 | 2 |");
        assert!(html.contains("<table>") || html.contains("<thead>"), "should have table");
    }

    #[test]
    fn to_html_inline_code() {
        let html = MarkdownRenderer::to_html("use `cargo test`");
        assert!(html.contains("<code>"));
    }

    #[tokio::test]
    async fn render_html_mode_produces_html() {
        let r = MarkdownRenderer::new();
        let result = r.render(&md("# Hello\n\nworld"), RenderMode::Html).await.unwrap();
        assert!(matches!(result, RenderedArtifact::Html(_)));
        let s = result.as_str().unwrap();
        assert!(s.contains("<h1>") || s.contains("<h1 "));
    }

    #[tokio::test]
    async fn render_terminal_returns_raw_text() {
        let r = MarkdownRenderer::new();
        let result = r.render(&md("hello"), RenderMode::Terminal).await.unwrap();
        assert!(matches!(result, RenderedArtifact::Text(_)));
    }

    #[tokio::test]
    async fn render_preview_truncates_long_content() {
        let r = MarkdownRenderer::new();
        let long = "x".repeat(1000);
        let result = r.render(&md(&long), RenderMode::Preview).await.unwrap();
        let s = result.as_str().unwrap();
        assert!(s.len() <= 510, "preview should be truncated");
        assert!(s.contains('…'), "should contain ellipsis");
    }

    #[tokio::test]
    async fn render_preview_short_not_truncated() {
        let r = MarkdownRenderer::new();
        let short = "Hello, world!";
        let result = r.render(&md(short), RenderMode::Preview).await.unwrap();
        assert_eq!(result.as_str().unwrap(), short);
    }

    #[tokio::test]
    async fn render_wrong_type_errors() {
        let r = MarkdownRenderer::new();
        let a = Artifact::Svg { content: "<svg/>".into() };
        assert!(r.render(&a, RenderMode::Terminal).await.is_err());
    }
}
