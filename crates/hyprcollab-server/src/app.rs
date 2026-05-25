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
use crate::memory::{
    create_fact, delete_fact, get_fact, list_facts, prune_facts, search_facts, update_fact,
    CreateFactRequest, DeleteFactResponse, FactResponse, ListFactsResponse, PruneFactsRequest,
    PruneFactsResponse, UpdateFactRequest,
};
use crate::skills::{
    create_skill, delete_skill, get_skill, learn_skills, list_skills, match_skills_handler,
    update_skill, CreateSkillRequest, DeleteSkillResponse, LearnSkillsRequest, LearnSkillsResponse,
    ListSkillsResponse, MatchSkillsRequest, MatchSkillsResponse, SkillResponse, UpdateSkillRequest,
};
use crate::themes::{
    create_theme, delete_theme, get_theme, get_theme_css, list_themes, CreateThemeRequest,
    DeleteThemeResponse, ListThemesResponse, ThemeColorsResponse, ThemeResponse,
};
use crate::browser::{
    browser_navigate, browser_screenshot, browser_search, browser_sessions, BrowserLink,
    BrowserNavigateRequest, BrowserNavigateResponse, BrowserScreenshotRequest,
    BrowserScreenshotResponse, BrowserSearchRequest, BrowserSearchResponse, BrowserSessionEntry,
    BrowserSessionsResponse,
};
use crate::conversations::{
    create_conversation, delete_conversation, get_conversation, list_conversations,
};
use crate::extras::{
    add_bookmark, delete_bookmark, fork_conversation, get_message_tree, list_bookmarks,
    pin_conversation, search_conversations, AddBookmarkRequest, BookmarkResponse,
    ConversationSearchResult, DeleteBookmarkResponse, ForkRequest, ForkResponse,
    ListBookmarksResponse, MessageItem, MessageTreeResponse, PinRequest, PinResponse,
    SearchResponse, TreeNode,
};
use crate::error::AppError;
use crate::image_gen::{
    image_generate, image_providers, GeneratedImageItem, ImageGenerateRequest,
    ImageGenerateResponse, ImageProviderEntry, ImageProvidersResponse,
};
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
        crate::browser::browser_navigate,
        crate::browser::browser_search,
        crate::browser::browser_sessions,
        crate::browser::browser_screenshot,
        crate::image_gen::image_generate,
        crate::image_gen::image_providers,
        crate::memory::list_facts,
        crate::memory::create_fact,
        crate::memory::search_facts,
        crate::memory::get_fact,
        crate::memory::update_fact,
        crate::memory::delete_fact,
        crate::memory::prune_facts,
        crate::themes::list_themes,
        crate::themes::get_theme,
        crate::themes::get_theme_css,
        crate::themes::create_theme,
        crate::themes::delete_theme,
        crate::skills::list_skills,
        crate::skills::create_skill,
        crate::skills::match_skills_handler,
        crate::skills::learn_skills,
        crate::skills::get_skill,
        crate::skills::update_skill,
        crate::skills::delete_skill,
        crate::extras::fork_conversation,
        crate::extras::get_message_tree,
        crate::extras::list_bookmarks,
        crate::extras::add_bookmark,
        crate::extras::delete_bookmark,
        crate::extras::pin_conversation,
        crate::extras::search_conversations,
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
            BrowserNavigateRequest,
            BrowserNavigateResponse,
            BrowserLink,
            BrowserSearchRequest,
            BrowserSearchResponse,
            BrowserSessionEntry,
            BrowserSessionsResponse,
            BrowserScreenshotRequest,
            BrowserScreenshotResponse,
            ImageGenerateRequest,
            ImageGenerateResponse,
            GeneratedImageItem,
            ImageProviderEntry,
            ImageProvidersResponse,
            CreateFactRequest,
            UpdateFactRequest,
            PruneFactsRequest,
            FactResponse,
            ListFactsResponse,
            DeleteFactResponse,
            PruneFactsResponse,
            CreateThemeRequest,
            ThemeResponse,
            ThemeColorsResponse,
            ListThemesResponse,
            DeleteThemeResponse,
            SkillResponse,
            ListSkillsResponse,
            CreateSkillRequest,
            UpdateSkillRequest,
            DeleteSkillResponse,
            MatchSkillsRequest,
            MatchSkillsResponse,
            LearnSkillsRequest,
            LearnSkillsResponse,
            MessageItem,
            TreeNode,
            MessageTreeResponse,
            ForkRequest,
            ForkResponse,
            BookmarkResponse,
            ListBookmarksResponse,
            AddBookmarkRequest,
            DeleteBookmarkResponse,
            PinRequest,
            PinResponse,
            ConversationSearchResult,
            SearchResponse,
        )
    ),
    tags(
        (name = "System", description = "Health and system endpoints"),
        (name = "Chat", description = "Chat completion endpoints"),
        (name = "Artifacts", description = "Artifact management"),
        (name = "Conversations", description = "Conversation CRUD"),
        (name = "RAG", description = "Retrieval-Augmented Generation"),
        (name = "Browser", description = "Browser automation and web scraping"),
        (name = "Image", description = "Image generation"),
        (name = "WorkingMemory", description = "Working memory — persistent facts with FTS5 search"),
        (name = "Themes", description = "UI theme engine — built-in and custom themes"),
        (name = "Skills", description = "Skills engine — YAML loader, regex matcher, auto-learner"),
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
        // Conversation endpoints (search + static sub-paths before wildcards)
        .route("/api/conversations/search", get(search_conversations))
        .route("/api/conversations", get(list_conversations))
        .route("/api/conversations", post(create_conversation))
        .route("/api/conversations/{id}/fork", post(fork_conversation))
        .route("/api/conversations/{id}/tree", get(get_message_tree))
        .route("/api/conversations/{id}/bookmarks", get(list_bookmarks))
        .route("/api/conversations/{id}/bookmarks", post(add_bookmark))
        .route("/api/conversations/{id}/pin", put(pin_conversation))
        .route("/api/conversations/{id}", get(get_conversation))
        .route("/api/conversations/{id}", delete(delete_conversation))
        .route("/api/bookmarks/{id}", delete(delete_bookmark))
        // RAG endpoints
        .route("/api/rag/upload", post(rag_upload))
        .route("/api/rag/query", post(rag_query))
        .route("/api/rag/documents", get(rag_list_documents))
        .route("/api/rag/documents/{id}", delete(rag_delete_document))
        // Browser endpoints
        .route("/api/browser/navigate", post(browser_navigate))
        .route("/api/browser/search", post(browser_search))
        .route("/api/browser/sessions", get(browser_sessions))
        .route("/api/browser/screenshot", post(browser_screenshot))
        // Image generation endpoints
        .route("/api/image/generate", post(image_generate))
        .route("/api/image/providers", get(image_providers))
        // Theme endpoints (css before {name} to avoid wildcard capture)
        .route("/api/themes", get(list_themes))
        .route("/api/themes", post(create_theme))
        .route("/api/themes/{name}/css", get(get_theme_css))
        .route("/api/themes/{name}", get(get_theme))
        .route("/api/themes/{name}", delete(delete_theme))
        // Skills endpoints (match + learn before {id} to avoid wildcard capture)
        .route("/api/skills", get(list_skills))
        .route("/api/skills", post(create_skill))
        .route("/api/skills/match", post(match_skills_handler))
        .route("/api/skills/learn", post(learn_skills))
        .route("/api/skills/{id}", get(get_skill))
        .route("/api/skills/{id}", put(update_skill))
        .route("/api/skills/{id}", delete(delete_skill))
        // Working memory endpoints (search + prune before {id} to avoid conflicts)
        .route("/api/memory/facts", get(list_facts))
        .route("/api/memory/facts", post(create_fact))
        .route("/api/memory/facts/search", get(search_facts))
        .route("/api/memory/facts/prune", post(prune_facts))
        .route("/api/memory/facts/{id}", get(get_fact))
        .route("/api/memory/facts/{id}", put(update_fact))
        .route("/api/memory/facts/{id}", delete(delete_fact))
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
            parent_id: None,
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
