use axum::{extract::State, Json};
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::state::AppState;

// ── request / response types ──────────────────────────────────────────────────

/// Request body for browser navigation.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BrowserNavigateRequest {
    pub url: String,
}

/// A hyperlink extracted from a page.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BrowserLink {
    pub text: String,
    pub href: String,
}

/// Response from a browser navigation.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BrowserNavigateResponse {
    pub url: String,
    pub title: Option<String>,
    pub text_content: String,
    pub links: Vec<BrowserLink>,
}

/// Request body for web search.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BrowserSearchRequest {
    pub query: String,
}

/// A single web search result.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BrowserSearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Response from a web search.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BrowserSearchResponse {
    pub results: Vec<BrowserSearchResult>,
}

/// A browser session entry.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BrowserSessionEntry {
    pub id: String,
    pub current_url: Option<String>,
}

/// Response listing active browser sessions.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BrowserSessionsResponse {
    pub sessions: Vec<BrowserSessionEntry>,
}

/// Request body for browser screenshots.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BrowserScreenshotRequest {
    pub url: String,
}

/// Response containing a base64-encoded screenshot.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BrowserScreenshotResponse {
    pub data: String,
    pub format: String,
}

// ── handlers ──────────────────────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/api/browser/navigate",
    request_body = BrowserNavigateRequest,
    responses(
        (status = 200, description = "Page loaded and parsed", body = BrowserNavigateResponse),
        (status = 503, description = "Browser engine not configured"),
    ),
    tag = "Browser"
)]
pub async fn browser_navigate(
    State(state): State<AppState>,
    Json(payload): Json<BrowserNavigateRequest>,
) -> Result<Json<BrowserNavigateResponse>, AppError> {
    let engine = state
        .browser
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("Browser engine not configured".into()))?;

    let session_id = engine.create_session();
    let snapshot = engine
        .navigate(&session_id, &payload.url)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    engine.remove_session(&session_id);

    let links = snapshot
        .links
        .into_iter()
        .map(|l| BrowserLink { text: l.text, href: l.href })
        .collect();

    Ok(Json(BrowserNavigateResponse {
        url: snapshot.url,
        title: snapshot.title,
        text_content: snapshot.text_content,
        links,
    }))
}

#[utoipa::path(
    post,
    path = "/api/browser/search",
    request_body = BrowserSearchRequest,
    responses(
        (status = 200, description = "Search results", body = BrowserSearchResponse),
        (status = 503, description = "Browser engine not configured"),
    ),
    tag = "Browser"
)]
pub async fn browser_search(
    State(state): State<AppState>,
    Json(payload): Json<BrowserSearchRequest>,
) -> Result<Json<BrowserSearchResponse>, AppError> {
    let engine = state
        .browser
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("Browser engine not configured".into()))?;

    let raw = engine
        .search(&payload.query)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let results = raw
        .into_iter()
        .map(|r| BrowserSearchResult { title: r.title, url: r.url, snippet: r.snippet })
        .collect();

    Ok(Json(BrowserSearchResponse { results }))
}

#[utoipa::path(
    get,
    path = "/api/browser/sessions",
    responses(
        (status = 200, description = "Active browser sessions", body = BrowserSessionsResponse),
        (status = 503, description = "Browser engine not configured"),
    ),
    tag = "Browser"
)]
pub async fn browser_sessions(
    State(state): State<AppState>,
) -> Result<Json<BrowserSessionsResponse>, AppError> {
    let engine = state
        .browser
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("Browser engine not configured".into()))?;

    let ids = engine.session_ids();
    let sessions = ids
        .into_iter()
        .filter_map(|id| {
            engine.get_session(&id).map(|s| BrowserSessionEntry { id, current_url: s.url })
        })
        .collect();

    Ok(Json(BrowserSessionsResponse { sessions }))
}

#[utoipa::path(
    post,
    path = "/api/browser/screenshot",
    request_body = BrowserScreenshotRequest,
    responses(
        (status = 200, description = "Base64-encoded PNG screenshot", body = BrowserScreenshotResponse),
        (status = 503, description = "Browser engine not configured or Playwright unavailable"),
    ),
    tag = "Browser"
)]
pub async fn browser_screenshot(
    State(state): State<AppState>,
    Json(payload): Json<BrowserScreenshotRequest>,
) -> Result<Json<BrowserScreenshotResponse>, AppError> {
    let engine = state
        .browser
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("Browser engine not configured".into()))?;

    let session_id = engine.create_session();
    engine
        .navigate(&session_id, &payload.url)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let bytes = engine
        .screenshot(&session_id)
        .await
        .map_err(|e| AppError::ServiceUnavailable(e.to_string()))?;
    engine.remove_session(&session_id);

    let data = base64::prelude::BASE64_STANDARD.encode(&bytes);
    Ok(Json(BrowserScreenshotResponse { data, format: "png".into() }))
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::json;
    use tower::ServiceExt;

    use crate::app::create_app;
    use crate::state::AppState;

    async fn test_state() -> AppState {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.keep();
        let store =
            hyprcollab_artifacts::ArtifactStore::new(base.join("test.db"), base.join("arts"))
                .await
                .expect("store");
        AppState::new(store)
    }

    fn json_request(method: &str, uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap()
    }

    #[tokio::test]
    async fn navigate_returns_503_when_not_configured() {
        let state = test_state().await;
        let app = create_app(state);
        let resp = app
            .oneshot(json_request("POST", "/api/browser/navigate", json!({"url": "https://example.com"})))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn search_returns_503_when_not_configured() {
        let state = test_state().await;
        let app = create_app(state);
        let resp = app
            .oneshot(json_request("POST", "/api/browser/search", json!({"query": "rust"})))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn sessions_returns_503_when_not_configured() {
        let state = test_state().await;
        let app = create_app(state);
        let req = Request::builder().uri("/api/browser/sessions").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn screenshot_returns_503_when_not_configured() {
        let state = test_state().await;
        let app = create_app(state);
        let resp = app
            .oneshot(json_request("POST", "/api/browser/screenshot", json!({"url": "https://example.com"})))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
