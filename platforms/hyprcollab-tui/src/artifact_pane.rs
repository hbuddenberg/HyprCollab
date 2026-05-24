use hyprcollab_artifacts::ArtifactId;
use hyprcollab_canvas::RenderedArtifact;

/// State for the artifact preview panel.
#[derive(Debug, Default)]
pub struct ArtifactPane {
    /// Currently displayed artifact id.
    current_id: Option<ArtifactId>,
    /// The rendered content ready for display.
    rendered: Option<RenderedArtifact>,
    scroll: u16,
}

impl ArtifactPane {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_artifact(&mut self, id: ArtifactId, rendered: RenderedArtifact) {
        self.current_id = Some(id);
        self.rendered = Some(rendered);
        self.scroll = 0;
    }

    pub fn clear(&mut self) {
        self.current_id = None;
        self.rendered = None;
        self.scroll = 0;
    }

    pub fn current_id(&self) -> Option<&ArtifactId> {
        self.current_id.as_ref()
    }

    pub fn is_empty(&self) -> bool {
        self.rendered.is_none()
    }

    /// Returns the text content for display (None for image artifacts).
    pub fn current_text(&self) -> Option<&str> {
        self.rendered.as_ref().and_then(|r| r.as_str())
    }

    pub fn rendered(&self) -> Option<&RenderedArtifact> {
        self.rendered.as_ref()
    }

    pub fn scroll_offset(&self) -> u16 {
        self.scroll
    }

    pub fn scroll_down(&mut self, lines: u16) {
        let content_lines = self
            .current_text()
            .map(|t| t.lines().count() as u16)
            .unwrap_or(0);
        let max = content_lines.saturating_sub(1);
        self.scroll = (self.scroll + lines).min(max);
    }

    pub fn scroll_up(&mut self, lines: u16) {
        self.scroll = self.scroll.saturating_sub(lines);
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aid(s: &str) -> ArtifactId {
        ArtifactId(s.to_string())
    }

    #[test]
    fn initial_state_is_empty() {
        let pane = ArtifactPane::new();
        assert!(pane.is_empty());
        assert!(pane.current_id().is_none());
    }

    #[test]
    fn set_artifact_stores_text() {
        let mut pane = ArtifactPane::new();
        pane.set_artifact(aid("a1"), RenderedArtifact::Text("hello\nworld".into()));
        assert!(!pane.is_empty());
        assert_eq!(pane.current_text(), Some("hello\nworld"));
    }

    #[test]
    fn set_artifact_updates_id() {
        let mut pane = ArtifactPane::new();
        pane.set_artifact(aid("abc"), RenderedArtifact::Html("<p/>".into()));
        assert_eq!(pane.current_id(), Some(&aid("abc")));
    }

    #[test]
    fn clear_resets_state() {
        let mut pane = ArtifactPane::new();
        pane.set_artifact(aid("x"), RenderedArtifact::Text("content".into()));
        pane.clear();
        assert!(pane.is_empty());
        assert!(pane.current_id().is_none());
    }

    #[test]
    fn image_artifact_returns_no_text() {
        let mut pane = ArtifactPane::new();
        pane.set_artifact(aid("img"), RenderedArtifact::Image(vec![0u8; 10]));
        assert!(pane.current_text().is_none());
        assert!(!pane.is_empty());
    }

    #[test]
    fn scroll_resets_on_new_artifact() {
        let mut pane = ArtifactPane::new();
        let content = (0..20).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        pane.set_artifact(aid("a"), RenderedArtifact::Text(content.clone()));
        pane.scroll_down(5);
        assert_eq!(pane.scroll_offset(), 5);
        pane.set_artifact(aid("b"), RenderedArtifact::Text(content));
        assert_eq!(pane.scroll_offset(), 0, "scroll should reset on new artifact");
    }
}
