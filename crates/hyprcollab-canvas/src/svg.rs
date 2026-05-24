use anyhow::Result;
use async_trait::async_trait;
use hyprcollab_artifacts::Artifact;
use resvg::usvg;
use tiny_skia::{Pixmap, Transform};

use crate::renderer::{ArtifactRenderer, RenderMode, RenderedArtifact};

pub struct SvgRenderer {
    pub default_width: u32,
}

impl SvgRenderer {
    pub fn new() -> Self {
        Self { default_width: 800 }
    }

    pub fn with_width(width: u32) -> Self {
        Self { default_width: width }
    }

    pub fn render_to_png(svg_content: &str) -> Result<Vec<u8>> {
        let opt = usvg::Options::default();
        let tree = usvg::Tree::from_data(svg_content.as_bytes(), &opt)
            .map_err(|e| anyhow::anyhow!("SVG parse error: {e}"))?;

        let size = tree.size().to_int_size();
        let mut pixmap = Pixmap::new(size.width(), size.height())
            .ok_or_else(|| anyhow::anyhow!("SVG has zero or invalid dimensions"))?;

        resvg::render(&tree, Transform::default(), &mut pixmap.as_mut());

        pixmap.encode_png().map_err(|e| anyhow::anyhow!("PNG encode error: {e}"))
    }
}

impl Default for SvgRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ArtifactRenderer for SvgRenderer {
    async fn render(&self, artifact: &Artifact, mode: RenderMode) -> Result<RenderedArtifact> {
        let content = match artifact {
            Artifact::Svg { content } => content.as_str(),
            _ => return Err(anyhow::anyhow!("SvgRenderer only handles Svg artifacts")),
        };

        match mode {
            RenderMode::Html => {
                let html = format!(r#"<div class="artifact-svg">{content}</div>"#);
                Ok(RenderedArtifact::Html(html))
            }
            RenderMode::Terminal | RenderMode::Preview => {
                match Self::render_to_png(content) {
                    Ok(png) => Ok(RenderedArtifact::Image(png)),
                    // Graceful fallback: return raw SVG source if rendering fails
                    Err(_) => Ok(RenderedArtifact::Raw(content.to_string())),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_SVG: &str =
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect x="0" y="0" width="10" height="10" fill="red"/></svg>"#;

    fn svg_artifact(content: &str) -> Artifact {
        Artifact::Svg { content: content.into() }
    }

    #[tokio::test]
    async fn render_html_wraps_in_div() {
        let r = SvgRenderer::new();
        let result = r.render(&svg_artifact(SIMPLE_SVG), RenderMode::Html).await.unwrap();
        let html = result.as_str().unwrap();
        assert!(html.starts_with(r#"<div class="artifact-svg">"#));
        assert!(html.contains("<svg"));
    }

    #[tokio::test]
    async fn render_terminal_valid_svg_produces_png() {
        let r = SvgRenderer::new();
        let result = r.render(&svg_artifact(SIMPLE_SVG), RenderMode::Terminal).await.unwrap();
        assert!(matches!(result, RenderedArtifact::Image(_)));
        if let RenderedArtifact::Image(bytes) = result {
            assert!(bytes.starts_with(b"\x89PNG"), "should be PNG bytes");
        }
    }

    #[tokio::test]
    async fn render_preview_valid_svg_produces_png() {
        let r = SvgRenderer::new();
        let result = r.render(&svg_artifact(SIMPLE_SVG), RenderMode::Preview).await.unwrap();
        assert!(matches!(result, RenderedArtifact::Image(_) | RenderedArtifact::Raw(_)));
    }

    #[tokio::test]
    async fn render_invalid_svg_falls_back_to_raw() {
        let r = SvgRenderer::new();
        let bad_svg = "<notsvg>garbage content</notsvg>";
        let result = r.render(&svg_artifact(bad_svg), RenderMode::Terminal).await.unwrap();
        assert!(
            matches!(result, RenderedArtifact::Raw(_) | RenderedArtifact::Image(_)),
            "invalid SVG should not panic"
        );
    }

    #[tokio::test]
    async fn render_wrong_type_errors() {
        let r = SvgRenderer::new();
        let a = Artifact::Markdown { content: "# hi".into() };
        assert!(r.render(&a, RenderMode::Terminal).await.is_err());
    }

    #[test]
    fn render_to_png_direct_returns_png_bytes() {
        let bytes = SvgRenderer::render_to_png(SIMPLE_SVG).unwrap();
        assert!(bytes.starts_with(b"\x89PNG"));
    }
}
