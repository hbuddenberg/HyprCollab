//! hyprcollab-canvas — Artifact rendering and canvas management.
//!
//! Provides renderers for every artifact type supported by HyprCollab
//! (code with syntax highlighting, Markdown, SVG, Mermaid) as well as a
//! `CanvasManager` that tracks open artifact tabs and diffs between versions.

pub mod code;
pub mod diff;
pub mod manager;
pub mod markdown;
pub mod mermaid;
pub mod renderer;
pub mod svg;

pub use code::CodeRenderer;
pub use diff::unified_diff;
pub use manager::{CanvasEvent, CanvasManager};
pub use markdown::MarkdownRenderer;
pub use mermaid::MermaidRenderer;
pub use renderer::{ArtifactRenderer, RenderMode, RenderedArtifact};
pub use svg::SvgRenderer;
