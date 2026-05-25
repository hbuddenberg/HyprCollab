use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self { width: 1280, height: 800 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserSession {
    pub id: String,
    pub url: Option<String>,
    pub cookies: Vec<Cookie>,
    pub history: Vec<String>,
    pub viewport: Viewport,
}

impl BrowserSession {
    pub fn new(id: String, viewport: Viewport) -> Self {
        Self { id, url: None, cookies: Vec::new(), history: Vec::new(), viewport }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum BrowserType {
    #[default]
    Chromium,
    Firefox,
    WebKit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserConfig {
    pub headless: bool,
    pub browser_type: BrowserType,
    pub default_timeout_ms: u64,
    pub viewport: Viewport,
    pub user_agent: Option<String>,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            headless: true,
            browser_type: BrowserType::default(),
            default_timeout_ms: 30_000,
            viewport: Viewport::default(),
            user_agent: None,
        }
    }
}
