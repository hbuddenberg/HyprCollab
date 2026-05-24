use anyhow::Result;
use async_trait::async_trait;
use hyprcollab_artifacts::Artifact;

/// How the artifact should be rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    /// ANSI 24-bit colored output for terminal display.
    Terminal,
    /// Full HTML with syntax highlighting via inline styles.
    Html,
    /// Truncated preview (first N lines / characters).
    Preview,
}

/// The output of a render operation.
#[derive(Debug, Clone)]
pub enum RenderedArtifact {
    /// ANSI-escaped text for terminal display.
    Text(String),
    /// Full HTML fragment or document.
    Html(String),
    /// Raw PNG bytes (e.g. rendered SVG).
    Image(Vec<u8>),
    /// Unformatted raw content (fallback).
    Raw(String),
}

impl RenderedArtifact {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            RenderedArtifact::Text(s)
            | RenderedArtifact::Html(s)
            | RenderedArtifact::Raw(s) => Some(s.as_str()),
            RenderedArtifact::Image(_) => None,
        }
    }

    pub fn is_image(&self) -> bool {
        matches!(self, RenderedArtifact::Image(_))
    }
}

/// Core rendering trait implemented by each artifact-type renderer.
#[async_trait]
pub trait ArtifactRenderer: Send + Sync {
    async fn render(&self, artifact: &Artifact, mode: RenderMode) -> Result<RenderedArtifact>;
}
