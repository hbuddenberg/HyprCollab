//! Web search tool — search the web via SearXNG or Brave API.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::Tool;
use serde::Deserialize;

/// Web search tool: queries SearXNG or Brave Search API.
pub struct WebSearchTool {
    /// SearXNG instance URL (default: https://searx.be).
    searx_url: String,
    /// Optional Brave API key.
    brave_api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchParams {
    query: String,
    #[serde(default = "default_count")]
    count: usize,
}

fn default_count() -> usize {
    5
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub query: String,
    pub total: usize,
}

impl WebSearchTool {
    pub fn new() -> Self {
        Self {
            searx_url: "https://searx.be".to_string(),
            brave_api_key: None,
        }
    }

    pub fn with_searx_url(mut self, url: impl Into<String>) -> Self {
        self.searx_url = url.into();
        self
    }

    pub fn with_brave_key(mut self, key: impl Into<String>) -> Self {
        self.brave_api_key = Some(key.into());
        self
    }

    /// Search using Brave API.
    async fn search_brave(&self, query: &str, count: usize) -> Result<SearchResponse> {
        let key = self
            .brave_api_key
            .as_ref()
            .ok_or_else(|| CoreError::Tool("Brave API key not configured".into()))?;

        let client = reqwest::Client::new();
        let resp = client
            .get("https://api.search.brave.com/res/v1/web/search")
            .header("X-Subscription-Token", key)
            .query(&[("q", query), ("count", &count.to_string())])
            .send()
            .await
            .map_err(|e| CoreError::Tool(format!("Brave search request failed: {e}")))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CoreError::Tool(format!("Failed to parse Brave response: {e}")))?;

        let mut results = Vec::new();
        if let Some(web_results) = body.get("web").and_then(|w| w.get("results"))
            && let Some(arr) = web_results.as_array()
        {
            for item in arr.iter().take(count) {
                results.push(SearchResult {
                    title: item
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    url: item
                        .get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    snippet: item
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                });
            }
        }

        let total = results.len();
        Ok(SearchResponse {
            results,
            query: query.to_string(),
            total,
        })
    }

    /// Search using SearXNG JSON API.
    async fn search_searx(&self, query: &str, count: usize) -> Result<SearchResponse> {
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{}/search", self.searx_url))
            .query(&[
                ("q", query),
                ("format", "json"),
                ("categories", "general"),
            ])
            .send()
            .await
            .map_err(|e| CoreError::Tool(format!("SearXNG request failed: {e}")))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CoreError::Tool(format!("Failed to parse SearXNG response: {e}")))?;

        let mut results = Vec::new();
        if let Some(arr) = body.get("results").and_then(|r| r.as_array()) {
            for item in arr.iter().take(count) {
                results.push(SearchResult {
                    title: item
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    url: item
                        .get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    snippet: item
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                });
            }
        }

        let total = results.len();
        Ok(SearchResponse {
            results,
            query: query.to_string(),
            total,
        })
    }
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web using Brave API or SearXNG. Returns titles, URLs, and snippets."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query string"
                },
                "count": {
                    "type": "integer",
                    "description": "Number of results (default: 5, max: 10)",
                    "default": 5
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let params: SearchParams = serde_json::from_value(args)
            .map_err(|e| CoreError::Tool(format!("Invalid web_search params: {e}")))?;

        let count = params.count.clamp(1, 10);

        let response = if self.brave_api_key.is_some() {
            self.search_brave(&params.query, count).await?
        } else {
            self.search_searx(&params.query, count).await?
        };

        serde_json::to_string(&response)
            .map_err(|e| CoreError::Tool(format!("Failed to serialize results: {e}")))
    }
}
