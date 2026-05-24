use anyhow::Result;
use async_trait::async_trait;
use hyprcollab_artifacts::Artifact;

use crate::renderer::{ArtifactRenderer, RenderMode, RenderedArtifact};

/// Known Mermaid diagram-type keywords.
const MERMAID_KEYWORDS: &[&str] = &[
    "graph",
    "flowchart",
    "sequenceDiagram",
    "classDiagram",
    "stateDiagram",
    "erDiagram",
    "gantt",
    "pie",
    "gitGraph",
    "mindmap",
    "timeline",
    "journey",
    "quadrantChart",
    "xychart-beta",
    "block-beta",
];

pub struct MermaidRenderer;

impl MermaidRenderer {
    pub fn new() -> Self {
        Self
    }

    /// Returns `true` when `content` starts with a recognised Mermaid keyword.
    pub fn is_valid_syntax(content: &str) -> bool {
        let trimmed = content.trim();
        MERMAID_KEYWORDS
            .iter()
            .any(|kw| trimmed.starts_with(kw))
    }
}

impl Default for MermaidRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ArtifactRenderer for MermaidRenderer {
    async fn render(&self, artifact: &Artifact, mode: RenderMode) -> Result<RenderedArtifact> {
        let content = match artifact {
            Artifact::Mermaid { content } => content.as_str(),
            _ => return Err(anyhow::anyhow!("MermaidRenderer only handles Mermaid artifacts")),
        };

        match mode {
            RenderMode::Html => {
                // Return source wrapped for client-side Mermaid.js rendering.
                let html = format!(r#"<div class="mermaid">{content}</div>"#);
                Ok(RenderedArtifact::Html(html))
            }
            RenderMode::Terminal | RenderMode::Preview => {
                let label = if Self::is_valid_syntax(content) {
                    "[mermaid diagram]"
                } else {
                    "[mermaid diagram — possibly invalid syntax]"
                };
                Ok(RenderedArtifact::Raw(format!("{label}\n\n{content}")))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mermaid(content: &str) -> Artifact {
        Artifact::Mermaid { content: content.into() }
    }

    #[tokio::test]
    async fn html_mode_wraps_in_mermaid_div() {
        let r = MermaidRenderer::new();
        let result = r
            .render(&mermaid("graph TD\n  A-->B"), RenderMode::Html)
            .await
            .unwrap();
        let html = result.as_str().unwrap();
        assert!(html.starts_with(r#"<div class="mermaid">"#));
        assert!(html.contains("graph TD"));
    }

    #[tokio::test]
    async fn terminal_mode_includes_source() {
        let r = MermaidRenderer::new();
        let content = "sequenceDiagram\n  A->>B: hello";
        let result = r.render(&mermaid(content), RenderMode::Terminal).await.unwrap();
        let out = result.as_str().unwrap();
        assert!(out.contains("sequenceDiagram"));
        assert!(out.contains("[mermaid diagram]"));
    }

    #[tokio::test]
    async fn preview_mode_includes_source() {
        let r = MermaidRenderer::new();
        let result = r
            .render(&mermaid("pie\n  title Pets\n  Dogs: 45"), RenderMode::Preview)
            .await
            .unwrap();
        assert!(result.as_str().unwrap().contains("pie"));
    }

    #[test]
    fn validates_graph_keyword() {
        assert!(MermaidRenderer::is_valid_syntax("graph TD\n  A-->B"));
        assert!(MermaidRenderer::is_valid_syntax("flowchart LR\n  X-->Y"));
        assert!(MermaidRenderer::is_valid_syntax("sequenceDiagram\n  A->>B: msg"));
        assert!(MermaidRenderer::is_valid_syntax("pie\n  title Pets"));
    }

    #[test]
    fn rejects_invalid_syntax() {
        assert!(!MermaidRenderer::is_valid_syntax("random text"));
        assert!(!MermaidRenderer::is_valid_syntax("<svg/>"));
        assert!(!MermaidRenderer::is_valid_syntax(""));
    }

    #[tokio::test]
    async fn invalid_syntax_gets_warning_label() {
        let r = MermaidRenderer::new();
        let result = r
            .render(&mermaid("garbage data"), RenderMode::Terminal)
            .await
            .unwrap();
        assert!(result.as_str().unwrap().contains("possibly invalid"));
    }

    #[tokio::test]
    async fn wrong_type_errors() {
        let r = MermaidRenderer::new();
        let a = Artifact::Code {
            language: "rust".into(),
            content: "fn main(){}".into(),
            filename: None,
        };
        assert!(r.render(&a, RenderMode::Html).await.is_err());
    }
}
