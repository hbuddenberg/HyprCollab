use std::sync::Arc;

use async_trait::async_trait;
use base64::prelude::*;
use hyprcollab_browser::BrowserEngine;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::Tool;

// ── BrowserNavigateTool ───────────────────────────────────────────────────────

pub struct BrowserNavigateTool {
    engine: Arc<BrowserEngine>,
}

impl BrowserNavigateTool {
    pub fn new(engine: Arc<BrowserEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl Tool for BrowserNavigateTool {
    fn name(&self) -> &str {
        "web_navigate"
    }

    fn description(&self) -> &str {
        "Navigate to a URL and return a structured page summary (title, links, text content)."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "The URL to navigate to" }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let url = args["url"]
            .as_str()
            .ok_or_else(|| CoreError::Tool("Missing 'url' parameter".into()))?;

        let session_id = self.engine.create_session();
        let snapshot = self
            .engine
            .navigate(&session_id, url)
            .await
            .map_err(|e| CoreError::Tool(format!("navigate: {e}")))?;
        self.engine.remove_session(&session_id);

        serde_json::to_string(&serde_json::json!({
            "url": snapshot.url,
            "title": snapshot.title,
            "text_content": snapshot.text_content,
            "links": snapshot.links.iter().map(|l| serde_json::json!({"text": l.text, "href": l.href})).collect::<Vec<_>>(),
        }))
        .map_err(|e| CoreError::Tool(e.to_string()))
    }
}

// ── BrowserSearchTool ─────────────────────────────────────────────────────────

pub struct BrowserSearchTool {
    engine: Arc<BrowserEngine>,
}

impl BrowserSearchTool {
    pub fn new(engine: Arc<BrowserEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl Tool for BrowserSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web and return a list of results with titles, URLs, and snippets."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "The search query" }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let query = args["query"]
            .as_str()
            .ok_or_else(|| CoreError::Tool("Missing 'query' parameter".into()))?;

        let results = self
            .engine
            .search(query)
            .await
            .map_err(|e| CoreError::Tool(format!("search: {e}")))?;

        let items: Vec<serde_json::Value> = results
            .iter()
            .map(|r| serde_json::json!({"title": r.title, "url": r.url, "snippet": r.snippet}))
            .collect();

        serde_json::to_string(&items).map_err(|e| CoreError::Tool(e.to_string()))
    }
}

// ── BrowserScreenshotTool ─────────────────────────────────────────────────────

pub struct BrowserScreenshotTool {
    engine: Arc<BrowserEngine>,
}

impl BrowserScreenshotTool {
    pub fn new(engine: Arc<BrowserEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl Tool for BrowserScreenshotTool {
    fn name(&self) -> &str {
        "web_screenshot"
    }

    fn description(&self) -> &str {
        "Take a screenshot of a URL (requires Playwright). Returns base64-encoded PNG."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "The URL to screenshot" }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let url = args["url"]
            .as_str()
            .ok_or_else(|| CoreError::Tool("Missing 'url' parameter".into()))?;

        let session_id = self.engine.create_session();
        self.engine
            .navigate(&session_id, url)
            .await
            .map_err(|e| CoreError::Tool(format!("navigate: {e}")))?;

        let bytes = self
            .engine
            .screenshot(&session_id)
            .await
            .map_err(|e| CoreError::Tool(format!("screenshot: {e}")))?;
        self.engine.remove_session(&session_id);

        let b64 = BASE64_STANDARD.encode(&bytes);
        serde_json::to_string(&serde_json::json!({"data": b64, "format": "png"}))
            .map_err(|e| CoreError::Tool(e.to_string()))
    }
}

// ── BrowserExtractTool ────────────────────────────────────────────────────────

pub struct BrowserExtractTool {
    engine: Arc<BrowserEngine>,
}

impl BrowserExtractTool {
    pub fn new(engine: Arc<BrowserEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl Tool for BrowserExtractTool {
    fn name(&self) -> &str {
        "web_extract"
    }

    fn description(&self) -> &str {
        "Navigate to a URL and extract text matching a CSS selector."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "The URL to navigate to" },
                "selector": { "type": "string", "description": "CSS selector to extract" }
            },
            "required": ["url", "selector"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let url = args["url"]
            .as_str()
            .ok_or_else(|| CoreError::Tool("Missing 'url' parameter".into()))?;
        let selector = args["selector"]
            .as_str()
            .ok_or_else(|| CoreError::Tool("Missing 'selector' parameter".into()))?;

        let session_id = self.engine.create_session();
        self.engine
            .navigate(&session_id, url)
            .await
            .map_err(|e| CoreError::Tool(format!("navigate: {e}")))?;

        let text = self
            .engine
            .extract(&session_id, selector)
            .await
            .map_err(|e| CoreError::Tool(format!("extract: {e}")))?;
        self.engine.remove_session(&session_id);

        Ok(text)
    }
}

// ── BrowserScrapeTool ─────────────────────────────────────────────────────────

pub struct BrowserScrapeTool {
    engine: Arc<BrowserEngine>,
}

impl BrowserScrapeTool {
    pub fn new(engine: Arc<BrowserEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl Tool for BrowserScrapeTool {
    fn name(&self) -> &str {
        "web_scrape"
    }

    fn description(&self) -> &str {
        "Scrape the full text content of a URL, suitable for ingestion into a RAG pipeline."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "The URL to scrape" }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let url = args["url"]
            .as_str()
            .ok_or_else(|| CoreError::Tool("Missing 'url' parameter".into()))?;

        let session_id = self.engine.create_session();
        let snapshot = self
            .engine
            .navigate(&session_id, url)
            .await
            .map_err(|e| CoreError::Tool(format!("scrape: {e}")))?;
        self.engine.remove_session(&session_id);

        Ok(snapshot.text_content)
    }
}
