#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{
        commands::{BrowserCommand, WebExtract, WebInteract, WebNavigate, WebScreenshot, WebSearch},
        engine::BrowserEngine,
        errors::BrowserError,
        page::{parse_html, parse_search_results},
        session::{BrowserConfig, BrowserType, Viewport},
    };

    fn make_engine() -> BrowserEngine {
        BrowserEngine::new(BrowserConfig::default()).expect("engine")
    }

    // ── parse_html ────────────────────────────────────────────────────────────

    #[test]
    fn test_parse_html_title() {
        let snap = parse_html(
            "<html><head><title>Test Page</title></head><body><p>Hello</p></body></html>",
            "http://example.com",
        );
        assert_eq!(snap.title, Some("Test Page".to_string()));
    }

    #[test]
    fn test_parse_html_no_title() {
        let snap = parse_html("<html><body><p>Hello</p></body></html>", "http://example.com");
        assert_eq!(snap.title, None);
    }

    #[test]
    fn test_parse_html_empty_title_ignored() {
        let snap =
            parse_html("<html><head><title>   </title></head><body></body></html>", "http://example.com");
        assert_eq!(snap.title, None);
    }

    #[test]
    fn test_parse_html_text_content() {
        let html = "<html><body><h1>Hello</h1><p>World</p></body></html>";
        let snap = parse_html(html, "http://example.com");
        assert!(snap.text_content.contains("Hello"));
        assert!(snap.text_content.contains("World"));
    }

    #[test]
    fn test_parse_html_links_absolute() {
        let html = r#"<html><body><a href="https://external.com">External</a></body></html>"#;
        let snap = parse_html(html, "http://example.com");
        assert_eq!(snap.links.len(), 1);
        assert!(snap.links[0].href.starts_with("https://external.com"));
        assert_eq!(snap.links[0].text, "External");
    }

    #[test]
    fn test_parse_html_links_relative_resolved() {
        let html = r#"<html><body><a href="/page1">Link</a></body></html>"#;
        let snap = parse_html(html, "http://example.com");
        assert_eq!(snap.links.len(), 1);
        assert!(snap.links[0].href.contains("example.com/page1"));
    }

    #[test]
    fn test_parse_html_metadata_name() {
        let html = r#"<html><head><meta name="description" content="Desc"></head><body></body></html>"#;
        let snap = parse_html(html, "http://example.com");
        assert_eq!(snap.metadata.get("description").map(String::as_str), Some("Desc"));
    }

    #[test]
    fn test_parse_html_metadata_property() {
        let html =
            r#"<html><head><meta property="og:title" content="OG"></head><body></body></html>"#;
        let snap = parse_html(html, "http://example.com");
        assert_eq!(snap.metadata.get("og:title").map(String::as_str), Some("OG"));
    }

    #[test]
    fn test_parse_html_stores_html() {
        let html = "<html><body><p>Content</p></body></html>";
        let snap = parse_html(html, "http://example.com");
        assert!(snap.html.is_some());
        assert!(snap.html.unwrap().contains("Content"));
    }

    #[test]
    fn test_parse_html_url_stored() {
        let snap = parse_html("<html><body></body></html>", "http://example.com/path");
        assert_eq!(snap.url, "http://example.com/path");
    }

    #[test]
    fn test_parse_html_screenshot_none() {
        let snap = parse_html("<html><body></body></html>", "http://example.com");
        assert!(snap.screenshot.is_none());
    }

    // ── search result parsing ─────────────────────────────────────────────────

    #[test]
    fn test_parse_search_results_empty() {
        let results = parse_search_results("<html><body></body></html>");
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_search_results_single() {
        let html = r#"<html><body>
            <div class="result">
                <div class="result__title"><a href="https://example.com">Example</a></div>
                <div class="result__snippet">A snippet about example</div>
            </div>
        </body></html>"#;
        let results = parse_search_results(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Example");
        assert_eq!(results[0].url, "https://example.com");
        assert!(results[0].snippet.contains("snippet"));
    }

    #[test]
    fn test_parse_search_results_max_ten() {
        let item = r#"<div class="result"><div class="result__title"><a href="http://x.com">T</a></div><div class="result__snippet">S</div></div>"#;
        let body: String = item.repeat(15);
        let html = format!("<html><body>{body}</body></html>");
        let results = parse_search_results(&html);
        assert!(results.len() <= 10);
    }

    // ── session management ────────────────────────────────────────────────────

    #[test]
    fn test_create_session_returns_id() {
        let engine = make_engine();
        let id = engine.create_session();
        assert!(!id.is_empty());
    }

    #[test]
    fn test_get_session_found() {
        let engine = make_engine();
        let id = engine.create_session();
        let session = engine.get_session(&id);
        assert!(session.is_some());
        assert_eq!(session.unwrap().id, id);
    }

    #[test]
    fn test_get_session_not_found() {
        let engine = make_engine();
        assert!(engine.get_session("nonexistent").is_none());
    }

    #[test]
    fn test_remove_session_existing() {
        let engine = make_engine();
        let id = engine.create_session();
        assert!(engine.remove_session(&id));
        assert!(engine.get_session(&id).is_none());
    }

    #[test]
    fn test_remove_session_missing() {
        let engine = make_engine();
        assert!(!engine.remove_session("ghost"));
    }

    #[test]
    fn test_session_count() {
        let engine = make_engine();
        assert_eq!(engine.session_count(), 0);
        engine.create_session();
        engine.create_session();
        assert_eq!(engine.session_count(), 2);
    }

    #[test]
    fn test_multiple_sessions_unique_ids() {
        let engine = make_engine();
        let id1 = engine.create_session();
        let id2 = engine.create_session();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_session_initial_state() {
        let engine = make_engine();
        let id = engine.create_session();
        let session = engine.get_session(&id).unwrap();
        assert!(session.url.is_none());
        assert!(session.history.is_empty());
        assert!(session.cookies.is_empty());
    }

    // ── config / types ────────────────────────────────────────────────────────

    #[test]
    fn test_default_config_headless() {
        assert!(BrowserConfig::default().headless);
    }

    #[test]
    fn test_default_config_timeout() {
        assert_eq!(BrowserConfig::default().default_timeout_ms, 30_000);
    }

    #[test]
    fn test_default_config_no_user_agent() {
        assert!(BrowserConfig::default().user_agent.is_none());
    }

    #[test]
    fn test_default_viewport_dimensions() {
        let vp = Viewport::default();
        assert_eq!(vp.width, 1280);
        assert_eq!(vp.height, 800);
    }

    #[test]
    fn test_browser_type_default_is_chromium() {
        assert!(matches!(BrowserType::default(), BrowserType::Chromium));
    }

    #[test]
    fn test_playwright_available_returns_bool() {
        let _ = BrowserEngine::playwright_available();
    }

    // ── async: navigate ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_navigate_basic() {
        let mut server = mockito::Server::new_async().await;
        let html =
            "<html><head><title>Mock Page</title></head><body><p>Hello from mock</p></body></html>";
        let _mock = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html)
            .create_async()
            .await;

        let engine = make_engine();
        let session_id = engine.create_session();
        let snapshot = engine.navigate(&session_id, &server.url()).await.expect("navigate");

        assert_eq!(snapshot.title, Some("Mock Page".to_string()));
        assert!(snapshot.text_content.contains("Hello from mock"));
    }

    #[tokio::test]
    async fn test_navigate_updates_session_url() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body("<html><body></body></html>")
            .create_async()
            .await;

        let engine = make_engine();
        let session_id = engine.create_session();
        engine.navigate(&session_id, &server.url()).await.expect("navigate");

        let session = engine.get_session(&session_id).unwrap();
        assert!(session.url.is_some());
        assert_eq!(session.history.len(), 1);
    }

    #[tokio::test]
    async fn test_navigate_invalid_url() {
        let engine = make_engine();
        let session_id = engine.create_session();
        let result = engine.navigate(&session_id, "not-a-url").await;
        assert!(matches!(result, Err(BrowserError::Url(_))));
    }

    #[tokio::test]
    async fn test_navigate_extracts_links() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let html = format!(r#"<html><body><a href="{url}/page1">Page 1</a></body></html>"#);
        let _mock = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html)
            .create_async()
            .await;

        let engine = make_engine();
        let session_id = engine.create_session();
        let snapshot = engine.navigate(&session_id, &url).await.expect("navigate");
        assert!(!snapshot.links.is_empty());
        assert_eq!(snapshot.links[0].text, "Page 1");
    }

    // ── async: extract ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_extract_selector() {
        let mut server = mockito::Server::new_async().await;
        let html = "<html><body><p class='target'>Extracted text</p></body></html>";
        let _m1 = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html)
            .create_async()
            .await;
        let _m2 = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html)
            .create_async()
            .await;

        let engine = make_engine();
        let session_id = engine.create_session();
        engine.navigate(&session_id, &server.url()).await.expect("navigate");
        let result = engine.extract(&session_id, ".target").await.expect("extract");
        assert!(result.contains("Extracted text"));
    }

    #[tokio::test]
    async fn test_extract_session_not_found() {
        let engine = make_engine();
        let result = engine.extract("ghost", "p").await;
        assert!(matches!(result, Err(BrowserError::SessionNotFound(_))));
    }

    #[tokio::test]
    async fn test_extract_no_page_loaded() {
        let engine = make_engine();
        let session_id = engine.create_session();
        let result = engine.extract(&session_id, "p").await;
        assert!(result.is_err());
    }

    // ── async: playwright-gated ops ───────────────────────────────────────────

    #[tokio::test]
    async fn test_screenshot_without_playwright() {
        if BrowserEngine::playwright_available() {
            return;
        }
        let engine = make_engine();
        let session_id = engine.create_session();
        let result = engine.screenshot(&session_id).await;
        assert!(matches!(result, Err(BrowserError::PlaywrightUnavailable)));
    }

    #[tokio::test]
    async fn test_interact_without_playwright() {
        if BrowserEngine::playwright_available() {
            return;
        }
        let engine = make_engine();
        let session_id = engine.create_session();
        let result = engine.interact(&session_id, "click", "#btn", None).await;
        assert!(matches!(result, Err(BrowserError::PlaywrightUnavailable)));
    }

    // ── async: search ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_search_with_mock_server() {
        let mut server = mockito::Server::new_async().await;
        let html = r#"<html><body>
            <div class="result">
                <div class="result__title"><a href="https://result.com">Result</a></div>
                <div class="result__snippet">Snippet text</div>
            </div>
        </body></html>"#;
        let _mock = server
            .mock("GET", mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html)
            .create_async()
            .await;

        let engine = BrowserEngine::new(BrowserConfig::default())
            .expect("engine")
            .with_search_url(server.url());

        let results = engine.search("test query").await.expect("search");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Result");
    }

    // ── commands ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_web_navigate_command() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body("<html><head><title>Cmd Page</title></head><body></body></html>")
            .create_async()
            .await;

        let engine = make_engine();
        let session_id = engine.create_session();
        let cmd = WebNavigate;
        let args = json!({ "session_id": session_id, "url": server.url() });
        let result = cmd.execute(&engine, args).await.expect("web_navigate");
        assert_eq!(result["title"], "Cmd Page");
    }

    #[tokio::test]
    async fn test_web_navigate_missing_url() {
        let engine = make_engine();
        let session_id = engine.create_session();
        let cmd = WebNavigate;
        let result = cmd.execute(&engine, json!({ "session_id": session_id })).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_web_extract_command() {
        let mut server = mockito::Server::new_async().await;
        let html = "<html><body><p class='x'>Target</p></body></html>";
        let _m1 = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html)
            .create_async()
            .await;
        let _m2 = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html)
            .create_async()
            .await;

        let engine = make_engine();
        let session_id = engine.create_session();
        engine.navigate(&session_id, &server.url()).await.expect("navigate");

        let cmd = WebExtract;
        let args = json!({ "session_id": session_id, "selector": ".x" });
        let result = cmd.execute(&engine, args).await.expect("web_extract");
        assert!(result.as_str().unwrap().contains("Target"));
    }

    #[tokio::test]
    async fn test_web_screenshot_command_no_playwright() {
        if BrowserEngine::playwright_available() {
            return;
        }
        let engine = make_engine();
        let session_id = engine.create_session();
        let result = WebScreenshot.execute(&engine, json!({ "session_id": session_id })).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_web_interact_command_no_playwright() {
        if BrowserEngine::playwright_available() {
            return;
        }
        let engine = make_engine();
        let session_id = engine.create_session();
        let args = json!({ "session_id": session_id, "action": "click", "target": "#btn" });
        let result = WebInteract.execute(&engine, args).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_web_search_command() {
        let mut server = mockito::Server::new_async().await;
        let html = r#"<html><body>
            <div class="result">
                <div class="result__title"><a href="https://r.com">R</a></div>
                <div class="result__snippet">S</div>
            </div>
        </body></html>"#;
        let _mock = server
            .mock("GET", mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html)
            .create_async()
            .await;

        let engine = BrowserEngine::new(BrowserConfig::default())
            .expect("engine")
            .with_search_url(server.url());

        let result = WebSearch.execute(&engine, json!({ "query": "hello" })).await.expect("search");
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["title"], "R");
    }
}
