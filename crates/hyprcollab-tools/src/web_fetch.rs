//! Web fetch tool — fetch and extract text from web pages.

use async_trait::async_trait;
use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::Tool;
use serde::Deserialize;

/// Web fetch tool: downloads a URL and returns the response body.
pub struct WebFetchTool {
    /// Maximum response size in bytes (default: 1 MB).
    max_size: usize,
    /// Request timeout in seconds.
    timeout_secs: u64,
}

#[derive(Debug, Deserialize)]
struct FetchParams {
    url: String,
    #[serde(default = "default_raw")]
    raw: bool,
}

fn default_raw() -> bool {
    false
}

#[derive(Debug, serde::Serialize)]
pub struct FetchResponse {
    pub url: String,
    pub status: u16,
    pub content_type: Option<String>,
    pub content: String,
    pub size_bytes: usize,
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self {
            max_size: 1024 * 1024, // 1 MB
            timeout_secs: 15,
        }
    }

    pub fn with_max_size(mut self, bytes: usize) -> Self {
        self.max_size = bytes;
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Naïve HTML tag stripper — removes everything between < and >.
    ///
    /// Iterates over `char_indices()` so multi-byte UTF-8 characters are
    /// handled correctly (the old byte-cast approach corrupted non-ASCII text).
    pub fn strip_html(html: &str) -> String {
        let mut result = String::with_capacity(html.len());
        let mut in_tag = false;
        let mut in_script = false;
        let lower = html.to_ascii_lowercase();

        let mut char_iter = html.char_indices().peekable();

        while let Some((byte_pos, ch)) = char_iter.next() {
            // Script block detection uses the byte-position slice of the
            // lowercased string; byte_pos is always on a char boundary because
            // char_indices guarantees it.
            if lower[byte_pos..].starts_with("<script") {
                in_script = true;
            }
            if in_script && lower[byte_pos..].starts_with("</script") {
                in_script = false;
                // Consume up to and including the closing '>'.
                for (_, c) in char_iter.by_ref() {
                    if c == '>' {
                        break;
                    }
                }
                continue;
            }
            if in_script {
                continue;
            }
            if ch == '<' {
                in_tag = true;
            } else if ch == '>' && in_tag {
                in_tag = false;
                result.push(' ');
            } else if !in_tag {
                result.push(ch);
            }
        }

        // Collapse whitespace
        let mut cleaned = String::new();
        let mut prev_ws = false;
        for ch in result.chars() {
            if ch.is_whitespace() {
                if !prev_ws {
                    cleaned.push(' ');
                    prev_ws = true;
                }
            } else {
                cleaned.push(ch);
                prev_ws = false;
            }
        }
        cleaned.trim().to_string()
    }
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch a web page by URL and return its text content. Optionally return raw HTML."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch"
                },
                "raw": {
                    "type": "boolean",
                    "description": "Return raw HTML instead of stripped text (default: false)",
                    "default": false
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let params: FetchParams = serde_json::from_value(args)
            .map_err(|e| CoreError::Tool(format!("Invalid web_fetch params: {e}")))?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(self.timeout_secs))
            .user_agent("HyprCollab/0.1 (agent)")
            .build()
            .map_err(|e| CoreError::Tool(format!("Failed to build HTTP client: {e}")))?;

        let resp = client
            .get(&params.url)
            .send()
            .await
            .map_err(|e| CoreError::Tool(format!("Request failed for {}: {e}", params.url)))?;

        let status = resp.status().as_u16();
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let body = resp
            .text()
            .await
            .map_err(|e| CoreError::Tool(format!("Failed to read response: {e}")))?;

        if body.len() > self.max_size {
            return Err(CoreError::Tool(format!(
                "Response too large: {} bytes (max: {})",
                body.len(),
                self.max_size
            )));
        }

        let size_bytes = body.len();
        let content = if params.raw {
            body
        } else {
            // Only strip HTML if content type suggests HTML.
            let is_html = content_type
                .as_deref()
                .map(|ct| ct.contains("html"))
                .unwrap_or(true);
            if is_html {
                Self::strip_html(&body)
            } else {
                body
            }
        };

        let response = FetchResponse {
            url: params.url,
            status,
            content_type,
            content,
            size_bytes,
        };

        serde_json::to_string(&response)
            .map_err(|e| CoreError::Tool(format!("Failed to serialize response: {e}")))
    }
}
