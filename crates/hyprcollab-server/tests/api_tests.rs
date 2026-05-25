use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use serde_json::Value;

use hyprcollab_artifacts::ArtifactStore;
use hyprcollab_server::{
    create_app, AppState, ChatRequest,
    ApprovalRequest, ApprovalResponse, HealthResponse,
};

/// Create a temporary AppState for tests (uses a tempdir for artifact storage).
async fn test_state() -> AppState {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.keep();
    let store = ArtifactStore::new(base.join("api_test.db"), base.join("arts"))
        .await
        .expect("artifact store");
    AppState::new(store)
}

#[tokio::test]
async fn test_health_check_status_ok() {
    let app = create_app(test_state().await);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_health_check_body_payload() {
    let app = create_app(test_state().await);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let health: HealthResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(health.status, "ok");
    assert!(!health.version.is_empty());
}

/// With no providers registered (default test state), non-streaming chat
/// proxies through the real router and returns 500 when no model is reachable.
#[tokio::test]
async fn test_chat_non_streaming_success() {
    let app = create_app(test_state().await);
    let req_body = ChatRequest {
        message: "hello there".to_string(),
        model: "gpt-4".to_string(),
        persona_id: Some("persona-123".to_string()),
        stream: Some(false),
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/chat")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&req_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Real router with no providers registered → 500 Internal Server Error
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

/// Streaming SSE always starts with HTTP 200; without a provider the stream
/// emits a structured `error` event instead of token events.
#[tokio::test]
async fn test_chat_streaming_success() {
    let app = create_app(test_state().await);
    let req_body = ChatRequest {
        message: "hello stream".to_string(),
        model: "gpt-4".to_string(),
        persona_id: None,
        stream: Some(true),
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/chat")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&req_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // SSE always opens with 200; provider failure appears in the stream body
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );

    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    // With no providers registered, an error event is emitted in the stream
    assert!(body_str.contains("data:"), "SSE body should have data lines");
    assert!(body_str.contains("error"), "should contain an error event: {body_str}");
}

#[tokio::test]
async fn test_chat_empty_message() {
    let app = create_app(test_state().await);
    let req_body = ChatRequest {
        message: "".to_string(),
        model: "gpt-4".to_string(),
        persona_id: None,
        stream: None,
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/chat")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&req_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let err_json: Value = serde_json::from_slice(&body).unwrap();
    assert!(err_json["error"].as_str().unwrap().contains("Message cannot be empty"));
}

#[tokio::test]
async fn test_chat_invalid_json() {
    let app = create_app(test_state().await);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/chat")
                .header("content-type", "application/json")
                .body(Body::from("{invalid_json"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_tool_approve_approved() {
    let state = test_state().await;
    let app = create_app(state.clone());
    let req_body = ApprovalRequest {
        tool_name: "read_file".to_string(),
        arguments: "{\"path\": \"/foo/bar\"}".to_string(),
        context: None,
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/tools/approve")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&req_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let app_resp: ApprovalResponse = serde_json::from_slice(&body).unwrap();
    assert!(app_resp.approved);
    assert!(app_resp.reason.unwrap().contains("approved"));

    let approvals = state.approvals.lock().await;
    assert_eq!(approvals.len(), 1);
    assert_eq!(approvals[0], "read_file");
}

#[tokio::test]
async fn test_tool_approve_denied() {
    let state = test_state().await;
    let app = create_app(state.clone());
    let req_body = ApprovalRequest {
        tool_name: "unsafe_rm_rf".to_string(),
        arguments: "{\"path\": \"/\"}".to_string(),
        context: Some("high risk action".to_string()),
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/tools/approve")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&req_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let app_resp: ApprovalResponse = serde_json::from_slice(&body).unwrap();
    assert!(!app_resp.approved);
    assert!(app_resp.reason.unwrap().contains("requires explicit user authorization"));

    let approvals = state.approvals.lock().await;
    assert_eq!(approvals.len(), 1);
    assert_eq!(approvals[0], "unsafe_rm_rf");
}

#[tokio::test]
async fn test_tool_approve_empty_name() {
    let app = create_app(test_state().await);
    let req_body = ApprovalRequest {
        tool_name: "".to_string(),
        arguments: "{}".to_string(),
        context: None,
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/tools/approve")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&req_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_cors_preflight_chat() {
    let app = create_app(test_state().await);
    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/chat")
                .header("origin", "http://localhost:3000")
                .header("access-control-request-method", "POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("access-control-allow-origin"));
    assert!(response.headers().contains_key("access-control-allow-methods"));
}

#[tokio::test]
async fn test_cors_preflight_health() {
    let app = create_app(test_state().await);
    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/health")
                .header("origin", "http://localhost:3000")
                .header("access-control-request-method", "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("access-control-allow-origin"));
    assert!(response.headers().contains_key("access-control-allow-methods"));
}
