//! # hyprcollab-artifacts
//!
//! Artifact types, storage (SQLite + filesystem), and an agent tool for
//! creating artifacts during conversations.

pub mod error;
pub mod store;
pub mod tool;
pub mod types;

pub use error::{ArtifactError, Result};
pub use store::ArtifactStore;
pub use tool::ArtifactCreateTool;
pub use types::{Artifact, ArtifactId, ArtifactMeta};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    async fn temp_store() -> ArtifactStore {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.keep();
        ArtifactStore::new(base.join("artifacts.db"), base.join("artifacts"))
            .await
            .expect("store init")
    }

    // ── Artifact::type_name ──────────────────────────────────────────

    #[test]
    fn type_names() {
        assert_eq!(Artifact::Markdown { content: "x".into() }.type_name(), "markdown");
        assert_eq!(
            Artifact::Code { language: "rust".into(), content: "x".into(), filename: None }.type_name(),
            "code"
        );
        assert_eq!(Artifact::Html { content: "x".into(), sandboxed: true }.type_name(), "html");
        assert_eq!(Artifact::Svg { content: "x".into() }.type_name(), "svg");
        assert_eq!(Artifact::Mermaid { content: "x".into() }.type_name(), "mermaid");
        assert_eq!(
            Artifact::React { code: "x".into(), dependencies: HashMap::new() }.type_name(),
            "react"
        );
        assert_eq!(Artifact::Latex { content: "x".into() }.type_name(), "latex");
        assert_eq!(
            Artifact::Image { url: "u".into(), alt: "a".into(), width: None, height: None }.type_name(),
            "image"
        );
        assert_eq!(Artifact::Pdf { url: "u".into(), page_count: None }.type_name(), "pdf");
    }

    // ── Artifact::extension ──────────────────────────────────────────

    #[test]
    fn extensions() {
        assert_eq!(
            Artifact::Code { language: "rust".into(), content: "x".into(), filename: None }.extension(),
            "rs"
        );
        assert_eq!(
            Artifact::Code { language: "python".into(), content: "x".into(), filename: None }.extension(),
            "py"
        );
        assert_eq!(Artifact::Markdown { content: "x".into() }.extension(), "md");
        assert_eq!(Artifact::Html { content: "x".into(), sandboxed: false }.extension(), "html");
        assert_eq!(Artifact::Svg { content: "x".into() }.extension(), "svg");
        assert_eq!(Artifact::Mermaid { content: "x".into() }.extension(), "mmd");
        assert_eq!(
            Artifact::React { code: "x".into(), dependencies: HashMap::new() }.extension(),
            "jsx"
        );
        assert_eq!(Artifact::Latex { content: "x".into() }.extension(), "tex");
    }

    // ── Artifact::content_str ────────────────────────────────────────

    #[test]
    fn content_str_present_for_text_variants() {
        assert!(Artifact::Markdown { content: "hello".into() }.content_str().is_some());
        assert!(Artifact::Code { language: "rs".into(), content: "fn main(){}".into(), filename: None }.content_str().is_some());
        assert!(Artifact::Html { content: "<p/>".into(), sandboxed: true }.content_str().is_some());
    }

    #[test]
    fn content_str_none_for_binary_variants() {
        assert!(Artifact::Image { url: "u".into(), alt: "a".into(), width: None, height: None }.content_str().is_none());
        assert!(Artifact::Pdf { url: "u".into(), page_count: None }.content_str().is_none());
    }

    // ── ArtifactStore CRUD ───────────────────────────────────────────

    #[tokio::test]
    async fn create_and_get_markdown() {
        let store = temp_store().await;
        let artifact = Artifact::Markdown { content: "# Hello World".into() };
        let id = store.create("chat-1", artifact.clone()).await.expect("create");

        let retrieved = store.get(&id).await.expect("get").expect("should exist");
        match retrieved {
            Artifact::Markdown { content } => assert_eq!(content, "# Hello World"),
            _ => panic!("expected Markdown"),
        }
    }

    #[tokio::test]
    async fn create_and_get_code() {
        let store = temp_store().await;
        let artifact = Artifact::Code {
            language: "rust".into(),
            content: "fn main() {}".into(),
            filename: Some("main.rs".into()),
        };
        let id = store.create("chat-2", artifact).await.expect("create");

        let retrieved = store.get(&id).await.expect("get").expect("should exist");
        match retrieved {
            Artifact::Code { language, content, filename } => {
                assert_eq!(language, "rust");
                assert_eq!(content, "fn main() {}");
                assert_eq!(filename.as_deref(), Some("main.rs"));
            }
            _ => panic!("expected Code"),
        }
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let store = temp_store().await;
        let id = ArtifactId("nonexistent-id".into());
        let result = store.get(&id).await.expect("query ok");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn update_artifact() {
        let store = temp_store().await;
        let id = store
            .create("chat-3", Artifact::Markdown { content: "old".into() })
            .await
            .expect("create");

        store
            .update(&id, Artifact::Markdown { content: "updated".into() })
            .await
            .expect("update");

        let retrieved = store.get(&id).await.expect("get").expect("exists");
        match retrieved {
            Artifact::Markdown { content } => assert_eq!(content, "updated"),
            _ => panic!("expected Markdown"),
        }
    }

    #[tokio::test]
    async fn update_nonexistent_returns_error() {
        let store = temp_store().await;
        let id = ArtifactId("no-such-id".into());
        let result = store.update(&id, Artifact::Markdown { content: "x".into() }).await;
        assert!(result.is_err());
        matches!(result.unwrap_err(), ArtifactError::NotFound(_));
    }

    #[tokio::test]
    async fn list_for_chat_empty() {
        let store = temp_store().await;
        let list = store.list_for_chat("empty-chat").await.expect("list");
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn list_for_chat_returns_correct_items() {
        let store = temp_store().await;
        store.create("chat-a", Artifact::Markdown { content: "1".into() }).await.unwrap();
        store.create("chat-a", Artifact::Svg { content: "<svg/>".into() }).await.unwrap();
        store.create("chat-b", Artifact::Markdown { content: "other".into() }).await.unwrap();

        let list = store.list_for_chat("chat-a").await.expect("list");
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|m| m.chat_id == "chat-a"));
    }

    #[tokio::test]
    async fn list_for_chat_preserves_types() {
        let store = temp_store().await;
        store.create("chat-x", Artifact::Markdown { content: "md".into() }).await.unwrap();
        store.create("chat-x", Artifact::Html { content: "<p/>".into(), sandboxed: true }).await.unwrap();
        store.create("chat-x", Artifact::Mermaid { content: "graph TB".into() }).await.unwrap();

        let list = store.list_for_chat("chat-x").await.expect("list");
        assert_eq!(list.len(), 3);
        let types: Vec<&str> = list.iter().map(|m| m.artifact_type.as_str()).collect();
        assert!(types.contains(&"markdown"));
        assert!(types.contains(&"html"));
        assert!(types.contains(&"mermaid"));
    }

    #[tokio::test]
    async fn delete_existing_returns_true() {
        let store = temp_store().await;
        let id = store
            .create("chat-d", Artifact::Markdown { content: "bye".into() })
            .await
            .expect("create");

        let deleted = store.delete(&id).await.expect("delete");
        assert!(deleted);
        assert!(store.get(&id).await.expect("get").is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent_returns_false() {
        let store = temp_store().await;
        let id = ArtifactId("phantom".into());
        let deleted = store.delete(&id).await.expect("delete");
        assert!(!deleted);
    }

    #[tokio::test]
    async fn filesystem_content_written_for_text_artifacts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.keep();
        let store = ArtifactStore::new(base.join("a.db"), base.join("arts"))
            .await
            .expect("init");

        let id = store
            .create("c", Artifact::Markdown { content: "# Test".into() })
            .await
            .expect("create");

        let file_path = base.join("arts").join(format!("{}.md", id.0));
        assert!(file_path.exists());
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "# Test");
    }

    // ── Serde round-trip ─────────────────────────────────────────────

    #[test]
    fn serde_code_round_trip() {
        let a = Artifact::Code {
            language: "python".into(),
            content: "print('hi')".into(),
            filename: Some("hello.py".into()),
        };
        let json = serde_json::to_string(&a).unwrap();
        let b: Artifact = serde_json::from_str(&json).unwrap();
        match b {
            Artifact::Code { language, .. } => assert_eq!(language, "python"),
            _ => panic!("round-trip failed"),
        }
    }

    #[test]
    fn serde_image_round_trip() {
        let a = Artifact::Image {
            url: "https://example.com/img.png".into(),
            alt: "A picture".into(),
            width: Some(800),
            height: Some(600),
        };
        let json = serde_json::to_string(&a).unwrap();
        let b: Artifact = serde_json::from_str(&json).unwrap();
        match b {
            Artifact::Image { width, height, .. } => {
                assert_eq!(width, Some(800));
                assert_eq!(height, Some(600));
            }
            _ => panic!("round-trip failed"),
        }
    }

    #[test]
    fn serde_react_with_dependencies() {
        let mut deps = HashMap::new();
        deps.insert("react".into(), "18.0.0".into());
        let a = Artifact::React { code: "export default () => <div/>;".into(), dependencies: deps };
        let json = serde_json::to_string(&a).unwrap();
        let b: Artifact = serde_json::from_str(&json).unwrap();
        match b {
            Artifact::React { dependencies, .. } => {
                assert_eq!(dependencies.get("react").map(|s| s.as_str()), Some("18.0.0"));
            }
            _ => panic!("round-trip failed"),
        }
    }
}
