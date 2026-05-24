use anyhow::Result;
use async_trait::async_trait;
use hyprcollab_artifacts::Artifact;
use syntect::{
    easy::HighlightLines,
    html::{styled_line_to_highlighted_html, IncludeBackground},
    highlighting::ThemeSet,
    parsing::SyntaxSet,
    util::{as_24_bit_terminal_escaped, LinesWithEndings},
};

use crate::renderer::{ArtifactRenderer, RenderMode, RenderedArtifact};

const DARK_THEME: &str = "base16-ocean.dark";
const PREVIEW_LINES: usize = 30;

pub struct CodeRenderer {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl CodeRenderer {
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    fn find_syntax<'a>(&'a self, language: &str) -> &'a syntect::parsing::SyntaxReference {
        let lower = language.to_lowercase();
        self.syntax_set
            .find_syntax_by_name(language)
            .or_else(|| self.syntax_set.find_syntax_by_name(&lower))
            .or_else(|| self.syntax_set.find_syntax_by_extension(&lower))
            .or_else(|| self.syntax_set.find_syntax_by_extension(language))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text())
    }

    /// Highlights `code` with 24-bit ANSI escape sequences.
    pub fn highlight_terminal(&self, code: &str, language: &str) -> String {
        let syntax = self.find_syntax(language);
        let theme = &self.theme_set.themes[DARK_THEME];
        let mut h = HighlightLines::new(syntax, theme);
        let mut out = String::new();
        for line in LinesWithEndings::from(code) {
            if let Ok(ranges) = h.highlight_line(line, &self.syntax_set) {
                out.push_str(&as_24_bit_terminal_escaped(&ranges, false));
            }
        }
        out.push_str("\x1b[0m");
        out
    }

    /// Highlights `code` and returns an HTML fragment with inline styles.
    pub fn highlight_html(&self, code: &str, language: &str) -> Result<String> {
        let syntax = self.find_syntax(language);
        let theme = &self.theme_set.themes[DARK_THEME];
        let mut h = HighlightLines::new(syntax, theme);
        let mut html = String::from("<pre><code>");
        for line in LinesWithEndings::from(code) {
            let ranges = h.highlight_line(line, &self.syntax_set)
                .map_err(|e| anyhow::anyhow!("syntect highlight error: {e}"))?;
            html.push_str(
                &styled_line_to_highlighted_html(&ranges, IncludeBackground::No)
                    .map_err(|e| anyhow::anyhow!("syntect html error: {e}"))?,
            );
        }
        html.push_str("</code></pre>");
        Ok(html)
    }
}

impl Default for CodeRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ArtifactRenderer for CodeRenderer {
    async fn render(&self, artifact: &Artifact, mode: RenderMode) -> Result<RenderedArtifact> {
        let (language, content) = match artifact {
            Artifact::Code { language, content, .. } => (language.as_str(), content.as_str()),
            _ => return Err(anyhow::anyhow!("CodeRenderer only handles Code artifacts")),
        };

        match mode {
            RenderMode::Terminal => Ok(RenderedArtifact::Text(
                self.highlight_terminal(content, language),
            )),
            RenderMode::Html => Ok(RenderedArtifact::Html(
                self.highlight_html(content, language)?,
            )),
            RenderMode::Preview => {
                let preview: String = content
                    .lines()
                    .take(PREVIEW_LINES)
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok(RenderedArtifact::Text(
                    self.highlight_terminal(&preview, language),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rust_artifact(code: &str) -> Artifact {
        Artifact::Code {
            language: "rust".into(),
            content: code.into(),
            filename: None,
        }
    }

    #[test]
    fn highlight_terminal_contains_ansi() {
        let r = CodeRenderer::new();
        let out = r.highlight_terminal("fn main() {}", "rust");
        assert!(out.contains("\x1b["), "should contain ANSI escapes");
        assert!(out.ends_with("\x1b[0m"), "should reset at end");
    }

    #[test]
    fn highlight_html_contains_pre_code() {
        let r = CodeRenderer::new();
        let html = r.highlight_html("fn main() {}", "rust").unwrap();
        assert!(html.contains("<pre>"), "should have <pre> tag");
        assert!(html.contains("</code></pre>"), "should close tags");
    }

    #[test]
    fn unknown_language_falls_back_to_plain() {
        let r = CodeRenderer::new();
        let out = r.highlight_terminal("hello world", "xyzzy-unknown");
        assert!(!out.is_empty());
    }

    #[tokio::test]
    async fn render_terminal_mode() {
        let r = CodeRenderer::new();
        let a = rust_artifact("fn foo() -> u32 { 42 }");
        let result = r.render(&a, RenderMode::Terminal).await.unwrap();
        assert!(matches!(result, RenderedArtifact::Text(_)));
        assert!(result.as_str().unwrap().contains("\x1b["));
    }

    #[tokio::test]
    async fn render_html_mode() {
        let r = CodeRenderer::new();
        let a = rust_artifact("fn foo() {}");
        let result = r.render(&a, RenderMode::Html).await.unwrap();
        assert!(matches!(result, RenderedArtifact::Html(_)));
        assert!(result.as_str().unwrap().contains("<pre>"));
    }

    #[tokio::test]
    async fn render_preview_truncates_at_30_lines() {
        let r = CodeRenderer::new();
        let big_code: String = (0..60).map(|i| format!("let x{i} = {i};\n")).collect();
        let a = Artifact::Code {
            language: "rust".into(),
            content: big_code,
            filename: None,
        };
        let result = r.render(&a, RenderMode::Preview).await.unwrap();
        let text = result.as_str().unwrap();
        let lines: Vec<_> = text
            .split('\n')
            .filter(|l| l.contains("let x"))
            .collect();
        assert!(lines.len() <= 30, "preview should be at most 30 code lines");
    }

    #[tokio::test]
    async fn render_wrong_type_returns_error() {
        let r = CodeRenderer::new();
        let a = Artifact::Markdown { content: "# hi".into() };
        assert!(r.render(&a, RenderMode::Terminal).await.is_err());
    }

    #[test]
    fn python_syntax_detected() {
        let r = CodeRenderer::new();
        let out = r.highlight_terminal("def hello():\n    print('hi')\n", "python");
        assert!(out.contains("\x1b["));
    }
}
