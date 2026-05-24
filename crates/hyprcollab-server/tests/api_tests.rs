use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use serde_json::Value;

use hyprcollab_server::{
    create_app, AppState, ChatRequest, ChatResponse,
    ApprovalRequest, ApprovalResponse, HealthResponse,
};

#[tokio::test]
async fn test_health_check_status_ok() {
    let app = create_app(AppState::default());
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
    let app = create_app(AppState::default());
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

#[tokio::test]
async fn test_chat_non_streaming_success() {
    let app = create_app(AppState::default());
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

    assert_eq!(response.status(), StatusCode::OK);
    
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let chat_resp: ChatResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(chat_resp.response, "Response to: hello there");
    assert_eq!(chat_resp.model, "gpt-4");
    assert_eq!(chat_resp.persona_id, Some("persona-123".to_string()));
}

#[tokio::test]
async fn test_chat_streaming_success() {
    let app = create_app(AppState::default());
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

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );

    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    
    assert!(body_str.contains("data:"));
    assert!(body_str.contains("hello"));
    assert!(body_str.contains("stream"));
}

#[tokio::test]
async fn test_chat_empty_message() {
    let app = create_app(AppState::default());
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
    let app = create_app(AppState::default());
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
    let state = AppState::default();
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

    let approvals = state.approvals.lock().unwrap();
    assert_eq!(approvals.len(), 1);
    assert_eq!(approvals[0], "read_file");
}

#[tokio::test]
async fn test_tool_approve_denied() {
    let state = AppState::default();
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

    let approvals = state.approvals.lock().unwrap();
    assert_eq!(approvals.len(), 1);
    assert_eq!(approvals[0], "unsafe_rm_rf");
}

#[tokio::test]
async fn test_tool_approve_empty_name() {
    let app = create_app(AppState::default());
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
    let app = create_app(AppState::default());
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
    let app = create_app(AppState::default());
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
