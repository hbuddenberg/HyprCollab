use thiserror::Error;

#[derive(Debug, Error)]
pub enum BrowserError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("parse: {0}")]
    Parse(String),
    #[error("selector: {0}")]
    Selector(String),
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("playwright not available")]
    PlaywrightUnavailable,
    #[error("playwright error: {0}")]
    Playwright(String),
    #[error("url: {0}")]
    Url(#[from] url::ParseError),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, BrowserError>;
