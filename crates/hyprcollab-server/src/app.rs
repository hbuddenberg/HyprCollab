use axum::{
    extract::State,
    http::Method,
    response::{sse::Event, IntoResponse, Response, Sse},
    routing::{delete, get, post, put},
    Json, Router,
};
use futures::StreamExt as _;
use hyprcollab_core::{
    traits::LlmProvider as _,
    types::{
        ChatId, ChatRequest as CoreChatRequest, Message, MessageId, MessageRole, PersonaId,
        SseEvent, TokenUsage,
    },
};
use std::convert::Infallible;
use tower_http::cors::{Any, CorsLayer};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable as _};

use crate::artifacts::{
    create_artifact, delete_artifact, get_artifact, list_artifacts, update_artifact,
};
use crate::conversations::{
    create_conversation, delete_conversation, get_conversation, list_conversations,
};
use crate::error::AppError;
use crate::rag::{
    rag_delete_document, rag_list_documents, rag_query, rag_upload, RagDeleteResponse,
    RagDocument, RagDocumentsResponse, RagIngestResponse, RagQueryRequest, RagQueryResponse,
    RagSearchResult,
};
use crate::sse::sse_event_to_event;
use crate::state::AppState;

/// HTTP request payload for chat completions.
#[derive(Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct ChatRequest {
    pub message: String,
    pub model: String,
    pub persona_id: Option<String>,
    pub stream: Option<bool>,
}

/// Response payload for non-streaming chat completions.
#[derive(Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct ChatResponse {
    pub response: String,
    pub model: String,
    pub persona_id: Option<String>,
}

/// Response chunk for streaming chat completions.
#[derive(Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct ChatStreamChunk {
    pub content: String,
    pub done: bool,
    pub model: String,
    pub persona_id: Option<String>,
}

/// Response payload for health checks.
#[derive(Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Request payload for tool approvals.
#[derive(Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct ApprovalRequest {
    pub tool_name: String,
    pub arguments: String,
    pub context: Option<String>,
}

/// Response payload for tool approvals.
#[derive(Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct ApprovalResponse {
    pub approved: bool,
    pub reason: Option<String>,
}

// ── OpenAPI spec ──────────────────────────────────────────────────────────────

#[derive(OpenApi)]
#[openapi(
    paths(
        health_handler,
        chat_handler,
        approve_handler,
        crate::artifacts::create_artifact,
        crate::artifacts::list_artifacts,
        crate::artifacts::get_artifact,
        crate::artifacts::update_artifact,
        crate::artifacts::delete_artifact,
        crate::conversations::create_conversation,
        crate::conversations::list_conversations,
        crate::conversations::get_conversation,
        crate::conversations::delete_conversation,
        crate::rag::rag_upload,
        crate::rag::rag_query,
        crate::rag::rag_list_documents,
        crate::rag::rag_delete_document,
    ),
    components(
        schemas(
            ChatRequest, ChatResponse, ChatStreamChunk,
            HealthResponse, ApprovalRequest, ApprovalResponse,
            SseEvent, TokenUsage,
            hyprcollab_artifacts::Artifact,
            hyprcollab_artifacts::ArtifactId,
            hyprcollab_artifacts::ArtifactMeta,
            crate::artifacts::CreateArtifactRequest,
            crate::artifacts::CreateArtifactResponse,
            crate::artifacts::ListArtifactsResponse,
            crate::artifacts::UpdateArtifactRequest,
            crate::artifacts::DeleteArtifactResponse,
            crate::conversations::CreateConversationRequest,
            crate::conversations::CreateConversationResponse,
            crate::conversations::ConversationSummary,
            crate::conversations::ListConversationsResponse,
            crate::conversations::MessageSummary,
            crate::conversations::ConversationDetail,
            crate::conversations::DeleteConversationResponse,
            RagIngestResponse,
            RagQueryRequest,
            RagQueryResponse,
            RagSearchResult,
            RagDocument,
            RagDocumentsResponse,
            RagDeleteResponse,
        )
    ),
    tags(
        (name = "System", description = "Health and system endpoints"),
        (name = "Chat", description = "Chat completion endpoints"),
        (name = "Artifacts", description = "Artifact management"),
        (name = "Conversations", description = "Conversation CRUD"),
        (name = "RAG", description = "Retrieval-Augmented Generation"),
    ),
    info(
        title = "HyprCollab API",
        version = "0.1.0",
        description = "HyprCollab AI collaboration platform — agentic chat with tools, artifacts, and personas",
    )
)]
pub struct ApiDoc;

// ── Router ────────────────────────────────────────────────────────────────────

/// Create the Axum Router configured with CORS and all endpoints.
///
/// `allow_origin(Any)` is intentional: HyprCollab runs as a local desktop
/// server accessed by the companion web/Tauri frontend on the same machine.
/// In production deployments this should be restricted to specific origins.
pub fn create_app(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers(Any);

    Router::new()
        // Core endpoints
        .route("/api/health", get(health_handler))
        .route("/api/chat", post(chat_handler))
        .route("/api/tools/approve", post(approve_handler))
        // Artifact endpoints
        .route("/api/artifacts", post(create_artifact))
        .route("/api/artifacts", get(list_artifacts))
        .route("/api/artifacts/{id}", get(get_artifact))
        .route("/api/artifacts/{id}", put(update_artifact))
        .route("/api/artifacts/{id}", delete(delete_artifact))
        // Conversation endpoints
        .route("/api/conversations", get(list_conversations))
        .route("/api/conversations", post(create_conversation))
        .route("/api/conversations/{id}", get(get_conversation))
        .route("/api/conversations/{id}", delete(delete_conversation))
        // RAG endpoints
        .route("/api/rag/upload", post(rag_upload))
        .route("/api/rag/query", post(rag_query))
        .route("/api/rag/documents", get(rag_list_documents))
        .route("/api/rag/documents/{id}", delete(rag_delete_document))
        // OpenAPI docs UI
        .merge(Scalar::with_url("/docs", ApiDoc::openapi()))
        .layer(cors)
        .with_state(state)
}

// ── Handlers ──────────────────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse),
    ),
    tag = "System"
)]
pub async fn health_handler(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: state.version.clone(),
    })
}

#[utoipa::path(
    post,
    path = "/api/chat",
    request_body = ChatRequest,
    responses(
        (status = 200, description = "Chat completion (JSON) or SSE stream", body = ChatResponse),
        (status = 400, description = "Empty message"),
        (status = 500, description = "Provider error"),
    ),
    tag = "Chat"
)]
pub async fn chat_handler(
    State(state): State<AppState>,
    Json(payload): Json<ChatRequest>,
) -> Result<Response, AppError> {
    if payload.message.trim().is_empty() {
        return Err(AppError::BadRequest("Message cannot be empty".to_string()));
    }

    let is_streaming = payload.stream.unwrap_or(false);

    let core_req = CoreChatRequest {
        model: payload.model.clone(),
        messages: vec![Message {
            id: MessageId::new(),
            chat_id: ChatId::new(),
            role: MessageRole::User,
            content: payload.message.clone(),
            tool_calls: vec![],
            artifacts: vec![],
            timestamp: chrono::Utc::now(),
            metadata: serde_json::Value::Null,
        }],
        tools: vec![],
        temperature: None,
        max_tokens: None,
        stream: is_streaming,
        persona: payload
            .persona_id
            .as_deref()
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
            .map(PersonaId::from),
        agent_role: None,
    };

    if is_streaming {
        let stream = state.router.chat_stream(core_req).flat_map(|result| {
            let events: Vec<Result<Event, Infallible>> = match result {
                Ok(chunk) => {
                    let mut evts = Vec::new();
                    if !chunk.delta.is_empty() {
                        evts.push(Ok(sse_event_to_event(SseEvent::Token { content: chunk.delta })));
                    }
                    for tc in &chunk.tool_calls {
                        evts.push(Ok(sse_event_to_event(SseEvent::ToolCall {
                            name: tc.name.clone(),
                            args: tc.arguments.clone(),
                            id: tc.id.clone(),
                        })));
                    }
                    if chunk.finish_reason.is_some() {
                        let usage = chunk.usage.unwrap_or(TokenUsage {
                            prompt_tokens: 0,
                            completion_tokens: 0,
                            total_tokens: 0,
                        });
                        evts.push(Ok(sse_event_to_event(SseEvent::Done { usage })));
                    }
                    evts
                }
                Err(e) => vec![Ok(Event::default()
                    .event("error")
                    .data(serde_json::json!({"error": e.to_string()}).to_string()))],
            };
            futures::stream::iter(events)
        });

        Ok(Sse::new(stream).into_response())
    } else {
        match state.router.chat_completion(core_req).await {
            Ok(resp) => {
                let response = ChatResponse {
                    response: resp.message.content,
                    model: resp.model,
                    persona_id: payload.persona_id,
                };
                Ok(Json(response).into_response())
            }
            Err(e) => Err(AppError::Internal(e.to_string())),
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/tools/approve",
    request_body = ApprovalRequest,
    responses(
        (status = 200, description = "Approval decision", body = ApprovalResponse),
        (status = 400, description = "Empty tool name"),
    ),
    tag = "System"
)]
pub async fn approve_handler(
    State(state): State<AppState>,
    Json(payload): Json<ApprovalRequest>,
) -> Result<Json<ApprovalResponse>, AppError> {
    if payload.tool_name.trim().is_empty() {
        return Err(AppError::BadRequest("Tool name cannot be empty".to_string()));
    }

    {
        let mut approvals = state.approvals.lock().await;
        approvals.push(payload.tool_name.clone());
    }

    let approved = !payload.tool_name.starts_with("unsafe");
    let reason = if approved {
        Some(format!(
            "Tool '{}' execution is approved by default policy",
            payload.tool_name
        ))
    } else {
        Some(format!(
            "Tool '{}' execution requires explicit user authorization",
            payload.tool_name
        ))
    };

    Ok(Json(ApprovalResponse { approved, reason }))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{self, Request, StatusCode},
    };
    use serde_json::json;
    use tower::ServiceExt;

    use super::*;

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
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap()
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let state = test_state().await;
        let app = create_app(state);

        let req = Request::builder().uri("/api/health").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["status"].as_str(), Some("ok"));
    }

    #[tokio::test]
    async fn chat_empty_message_returns_400() {
        let state = test_state().await;
        let app = create_app(state);

        let body = json!({"message": "  ", "model": "openai/gpt-4o"});
        let resp = app.oneshot(json_request("POST", "/api/chat", body)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn chat_non_streaming_no_providers_returns_500() {
        let state = test_state().await;
        let app = create_app(state);

        let body = json!({"message": "hello", "model": "openai/gpt-4o", "stream": false});
        let resp = app.oneshot(json_request("POST", "/api/chat", body)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn chat_streaming_returns_200_with_sse_error() {
        let state = test_state().await;
        let app = create_app(state);

        let body = json!({"message": "hello", "model": "openai/gpt-4o", "stream": true});
        let resp = app.oneshot(json_request("POST", "/api/chat", body)).await.unwrap();
        // SSE always starts with 200; the error is in the body
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let body_str = std::str::from_utf8(&bytes).unwrap();
        // With no providers, the stream emits an error event
        assert!(body_str.contains("error"), "expected error event, got: {body_str}");
    }

    #[tokio::test]
    async fn approve_handler_approves_safe_tool() {
        let state = test_state().await;
        let app = create_app(state);

        let body = json!({"tool_name": "read_file", "arguments": "{}"});
        let resp = app.oneshot(json_request("POST", "/api/tools/approve", body)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["approved"].as_bool(), Some(true));
    }

    #[tokio::test]
    async fn approve_handler_blocks_unsafe_tool() {
        let state = test_state().await;
        let app = create_app(state);

        let body = json!({"tool_name": "unsafe_delete", "arguments": "{}"});
        let resp = app.oneshot(json_request("POST", "/api/tools/approve", body)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["approved"].as_bool(), Some(false));
    }

    #[tokio::test]
    async fn docs_route_returns_200() {
        let state = test_state().await;
        let app = create_app(state);

        let req = Request::builder().uri("/docs").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
