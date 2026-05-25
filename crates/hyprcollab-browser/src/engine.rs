use std::{
    collections::HashMap,
    sync::RwLock,
    time::Duration,
};

use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use scraper::{Html, Selector};
use tracing::debug;
use url::Url;
use uuid::Uuid;

use crate::{
    errors::{BrowserError, Result},
    page::{parse_html, parse_search_results, PageSnapshot, SearchResult},
    session::{BrowserConfig, BrowserSession, Viewport},
};

pub struct BrowserEngine {
    sessions: RwLock<HashMap<String, BrowserSession>>,
    pub(crate) http: reqwest::Client,
    pub(crate) playwright_url: Option<String>,
    pub(crate) config: BrowserConfig,
    search_base_url: String,
}

impl BrowserEngine {
    pub fn new(config: BrowserConfig) -> Result<Self> {
        let ua = config
            .user_agent
            .as_deref()
            .unwrap_or("Mozilla/5.0 (compatible; HyprCollab/0.1)");
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(ua).map_err(|e| BrowserError::Parse(e.to_string()))?,
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .cookie_store(true)
            .timeout(Duration::from_millis(config.default_timeout_ms))
            .build()?;

        Ok(Self {
            sessions: RwLock::new(HashMap::new()),
            http,
            playwright_url: None,
            config,
            search_base_url: "https://html.duckduckgo.com/html/".to_string(),
        })
    }

    pub fn with_playwright(mut self, url: String) -> Self {
        self.playwright_url = Some(url);
        self
    }

    pub fn with_search_url(mut self, url: String) -> Self {
        self.search_base_url = url;
        self
    }

    pub fn playwright_available() -> bool {
        which::which("npx").is_ok()
    }

    pub fn create_session(&self) -> String {
        let id = Uuid::new_v4().to_string();
        let session = BrowserSession::new(id.clone(), self.config.viewport.clone());
        self.sessions.write().expect("sessions lock").insert(id.clone(), session);
        id
    }

    pub fn create_session_with_id(&self, id: &str) -> String {
        let session = BrowserSession::new(id.to_string(), self.config.viewport.clone());
        self.sessions.write().expect("sessions lock").insert(id.to_string(), session);
        id.to_string()
    }

    pub fn get_session(&self, id: &str) -> Option<BrowserSession> {
        self.sessions.read().expect("sessions lock").get(id).cloned()
    }

    pub fn remove_session(&self, id: &str) -> bool {
        self.sessions.write().expect("sessions lock").remove(id).is_some()
    }

    pub fn session_count(&self) -> usize {
        self.sessions.read().expect("sessions lock").len()
    }

    pub fn session_ids(&self) -> Vec<String> {
        self.sessions.read().expect("sessions lock").keys().cloned().collect()
    }

    pub async fn navigate(&self, session_id: &str, url: &str) -> Result<PageSnapshot> {
        let parsed_url = Url::parse(url)?;
        debug!("navigate session={session_id} url={url}");

        let response = self.http.get(parsed_url.as_str()).send().await?;
        let final_url = response.url().to_string();
        let html_text = response.text().await?;

        let snapshot = parse_html(&html_text, &final_url);

        {
            let mut sessions = self.sessions.write().expect("sessions lock");
            if let Some(session) = sessions.get_mut(session_id) {
                session.url = Some(final_url.clone());
                session.history.push(final_url);
            }
        }

        Ok(snapshot)
    }

    pub async fn extract(&self, session_id: &str, selector: &str) -> Result<String> {
        let current_url = {
            let sessions = self.sessions.read().expect("sessions lock");
            sessions
                .get(session_id)
                .ok_or_else(|| BrowserError::SessionNotFound(session_id.to_string()))?
                .url
                .clone()
        };

        let url = current_url
            .ok_or_else(|| BrowserError::Parse("no page loaded in session".to_string()))?;

        debug!("extract session={session_id} selector={selector}");
        let response = self.http.get(&url).send().await?;
        let html_text = response.text().await?;

        let document = Html::parse_document(&html_text);
        let sel = Selector::parse(selector)
            .map_err(|e| BrowserError::Selector(format!("{e:?}")))?;

        let extracted: Vec<String> = document
            .select(&sel)
            .map(|el| {
                el.text()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .filter(|s| !s.is_empty())
            .collect();

        Ok(extracted.join("\n"))
    }

    pub async fn screenshot(&self, _session_id: &str) -> Result<Vec<u8>> {
        if !Self::playwright_available() {
            return Err(BrowserError::PlaywrightUnavailable);
        }
        Err(BrowserError::Playwright("playwright bridge not yet implemented".to_string()))
    }

    pub async fn interact(
        &self,
        _session_id: &str,
        _action: &str,
        _target: &str,
        _value: Option<&str>,
    ) -> Result<()> {
        if !Self::playwright_available() {
            return Err(BrowserError::PlaywrightUnavailable);
        }
        Err(BrowserError::Playwright("playwright bridge not yet implemented".to_string()))
    }

    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        debug!("search query={query}");
        let response = self
            .http
            .get(&self.search_base_url)
            .query(&[("q", query)])
            .send()
            .await?;
        let html_text = response.text().await?;
        Ok(parse_search_results(&html_text))
    }

    pub fn config(&self) -> &BrowserConfig {
        &self.config
    }

    pub fn set_viewport(&self, session_id: &str, viewport: Viewport) {
        let mut sessions = self.sessions.write().expect("sessions lock");
        if let Some(session) = sessions.get_mut(session_id) {
            session.viewport = viewport;
        }
    }
}
