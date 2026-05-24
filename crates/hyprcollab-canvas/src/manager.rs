use indexmap::IndexMap;

use hyprcollab_artifacts::{Artifact, ArtifactId};

use crate::diff::unified_diff;

/// Events emitted by the canvas as artifacts are opened, modified, or closed.
#[derive(Debug, Clone)]
pub enum CanvasEvent {
    ArtifactOpened(ArtifactId),
    ArtifactModified(ArtifactId),
    ArtifactClosed(ArtifactId),
}

struct ArtifactEntry {
    artifact: Artifact,
    /// Each entry is the text content snapshot after a save.
    history: Vec<String>,
}

/// Manages the set of currently open artifacts (tabs) in the canvas.
///
/// Tracks the active artifact, edit history per artifact, and emits
/// `CanvasEvent`s that the UI layer can consume.
pub struct CanvasManager {
    open: IndexMap<ArtifactId, ArtifactEntry>,
    active: Option<ArtifactId>,
    events: Vec<CanvasEvent>,
}

impl CanvasManager {
    pub fn new() -> Self {
        Self {
            open: IndexMap::new(),
            active: None,
            events: Vec::new(),
        }
    }

    /// Open an artifact as a new tab.  If no tab is currently active, this
    /// artifact becomes the active one automatically.
    pub fn open_artifact(&mut self, id: ArtifactId, artifact: Artifact) {
        let content = artifact.content_str().unwrap_or("").to_string();
        self.open.insert(
            id.clone(),
            ArtifactEntry {
                artifact,
                history: vec![content],
            },
        );
        if self.active.is_none() {
            self.active = Some(id.clone());
        }
        self.events.push(CanvasEvent::ArtifactOpened(id));
    }

    /// Close an artifact tab.  Returns `true` if the tab was open.
    pub fn close_artifact(&mut self, id: &ArtifactId) -> bool {
        if self.open.shift_remove(id).is_some() {
            if self.active.as_ref() == Some(id) {
                self.active = self.open.keys().next().cloned();
            }
            self.events.push(CanvasEvent::ArtifactClosed(id.clone()));
            true
        } else {
            false
        }
    }

    /// Switch the active (focused) tab.  Returns `false` if `id` is not open.
    pub fn set_active(&mut self, id: ArtifactId) -> bool {
        if self.open.contains_key(&id) {
            self.active = Some(id);
            true
        } else {
            false
        }
    }

    pub fn active_id(&self) -> Option<&ArtifactId> {
        self.active.as_ref()
    }

    pub fn get(&self, id: &ArtifactId) -> Option<&Artifact> {
        self.open.get(id).map(|e| &e.artifact)
    }

    /// Replace the artifact content, record a history snapshot, and return
    /// the unified diff between old and new text content.
    ///
    /// Returns `None` if `id` is not open.
    pub fn update(&mut self, id: &ArtifactId, new_artifact: Artifact) -> Option<String> {
        let entry = self.open.get_mut(id)?;
        let old_text = entry.artifact.content_str().unwrap_or("").to_string();
        let new_text = new_artifact.content_str().unwrap_or("").to_string();
        let diff = unified_diff(&old_text, &new_text);
        entry.history.push(new_text);
        entry.artifact = new_artifact;
        self.events.push(CanvasEvent::ArtifactModified(id.clone()));
        Some(diff)
    }

    pub fn tab_count(&self) -> usize {
        self.open.len()
    }

    pub fn is_open(&self, id: &ArtifactId) -> bool {
        self.open.contains_key(id)
    }

    pub fn open_ids(&self) -> Vec<ArtifactId> {
        self.open.keys().cloned().collect()
    }

    pub fn history(&self, id: &ArtifactId) -> Option<&[String]> {
        self.open.get(id).map(|e| e.history.as_slice())
    }

    /// Drain and return all accumulated events.
    pub fn take_events(&mut self) -> Vec<CanvasEvent> {
        std::mem::take(&mut self.events)
    }
}

impl Default for CanvasManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> ArtifactId {
        ArtifactId(s.to_string())
    }

    fn md(content: &str) -> Artifact {
        Artifact::Markdown { content: content.into() }
    }

    fn code(content: &str) -> Artifact {
        Artifact::Code { language: "rust".into(), content: content.into(), filename: None }
    }

    #[test]
    fn open_and_retrieve_artifact() {
        let mut cm = CanvasManager::new();
        cm.open_artifact(id("a1"), md("hello"));
        let a = cm.get(&id("a1")).unwrap();
        match a {
            Artifact::Markdown { content } => assert_eq!(content, "hello"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn first_opened_becomes_active() {
        let mut cm = CanvasManager::new();
        cm.open_artifact(id("x"), md("x"));
        assert_eq!(cm.active_id(), Some(&id("x")));
    }

    #[test]
    fn second_open_does_not_change_active() {
        let mut cm = CanvasManager::new();
        cm.open_artifact(id("a"), md("a"));
        cm.open_artifact(id("b"), md("b"));
        assert_eq!(cm.active_id(), Some(&id("a")));
    }

    #[test]
    fn set_active_switches_tab() {
        let mut cm = CanvasManager::new();
        cm.open_artifact(id("a"), md("a"));
        cm.open_artifact(id("b"), md("b"));
        assert!(cm.set_active(id("b")));
        assert_eq!(cm.active_id(), Some(&id("b")));
    }

    #[test]
    fn set_active_unknown_returns_false() {
        let mut cm = CanvasManager::new();
        assert!(!cm.set_active(id("ghost")));
    }

    #[test]
    fn close_artifact_decrements_count() {
        let mut cm = CanvasManager::new();
        cm.open_artifact(id("a"), md("a"));
        cm.open_artifact(id("b"), md("b"));
        assert_eq!(cm.tab_count(), 2);
        assert!(cm.close_artifact(&id("a")));
        assert_eq!(cm.tab_count(), 1);
    }

    #[test]
    fn close_active_tab_reassigns_active() {
        let mut cm = CanvasManager::new();
        cm.open_artifact(id("a"), md("a"));
        cm.open_artifact(id("b"), md("b"));
        cm.set_active(id("a"));
        cm.close_artifact(&id("a"));
        // active is now some other open tab (or None if no tabs remain)
        let active = cm.active_id().cloned();
        assert!(active == Some(id("b")) || active.is_none());
    }

    #[test]
    fn close_nonexistent_returns_false() {
        let mut cm = CanvasManager::new();
        assert!(!cm.close_artifact(&id("nope")));
    }

    #[test]
    fn update_returns_diff() {
        let mut cm = CanvasManager::new();
        cm.open_artifact(id("a"), md("line1\n"));
        let diff = cm.update(&id("a"), md("line1\nline2\n")).unwrap();
        assert!(diff.contains("+line2"), "diff: {diff}");
    }

    #[test]
    fn update_same_content_returns_empty_diff() {
        let mut cm = CanvasManager::new();
        cm.open_artifact(id("a"), md("same\n"));
        let diff = cm.update(&id("a"), md("same\n")).unwrap();
        assert!(diff.is_empty());
    }

    #[test]
    fn update_grows_history() {
        let mut cm = CanvasManager::new();
        cm.open_artifact(id("a"), md("v1"));
        cm.update(&id("a"), md("v2"));
        cm.update(&id("a"), md("v3"));
        let hist = cm.history(&id("a")).unwrap();
        assert_eq!(hist.len(), 3, "v1 snapshot + 2 updates");
    }

    #[test]
    fn update_nonexistent_returns_none() {
        let mut cm = CanvasManager::new();
        assert!(cm.update(&id("nope"), md("x")).is_none());
    }

    #[test]
    fn events_emitted_on_open_close_modify() {
        let mut cm = CanvasManager::new();
        cm.open_artifact(id("a"), code("fn f(){}"));
        cm.update(&id("a"), code("fn g(){}"));
        cm.close_artifact(&id("a"));

        let events = cm.take_events();
        let kinds: Vec<&str> = events
            .iter()
            .map(|e| match e {
                CanvasEvent::ArtifactOpened(_) => "open",
                CanvasEvent::ArtifactModified(_) => "modify",
                CanvasEvent::ArtifactClosed(_) => "close",
            })
            .collect();
        assert_eq!(kinds, vec!["open", "modify", "close"]);
    }

    #[test]
    fn take_events_drains_queue() {
        let mut cm = CanvasManager::new();
        cm.open_artifact(id("a"), md("x"));
        let _ = cm.take_events();
        assert!(cm.take_events().is_empty());
    }
}
