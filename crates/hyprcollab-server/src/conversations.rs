//! REST handlers for the `/api/conversations` family of endpoints (S5).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use hyprcollab_core::types::ChatId;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::state::AppState;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_chat_id(s: &str) -> Result<ChatId, AppError> {
    uuid::Uuid::parse_str(s)
        .map(ChatId::from)
        .map_err(|_| AppError::BadRequest(format!("invalid conversation id: {s}")))
}

// ── Request / Response types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateConversationRequest {
    pub title: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateConversationResponse {
    pub id: String,
}

#[derive(Debug, Serialize)]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub model: String,
    pub created_at: String,
    pub last_message_at: String,
}

#[derive(Debug, Serialize)]
pub struct ListConversationsResponse {
    pub conversations: Vec<ConversationSummary>,
}

#[derive(Debug, Serialize)]
pub struct MessageSummary {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ConversationDetail {
    pub id: String,
    pub title: String,
    pub model: String,
    pub created_at: String,
    pub messages: Vec<MessageSummary>,
}

#[derive(Debug, Serialize)]
pub struct DeleteConversationResponse {
    pub deleted: bool,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// `GET /api/conversations` — list all conversations.
pub async fn list_conversations(
    State(state): State<AppState>,
) -> Result<Json<ListConversationsResponse>, AppError> {
    let chats = state
        .memory
        .list_chats()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let conversations = chats
        .into_iter()
        .map(|c| ConversationSummary {
            id: c.id.to_string(),
            title: c.title,
            model: c.model,
            created_at: c.created_at,
            last_message_at: c.updated_at,
        })
        .collect();

    Ok(Json(ListConversationsResponse { conversations }))
}

/// `GET /api/conversations/{id}` — get one conversation with its messages.
pub async fn get_conversation(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ConversationDetail>, AppError> {
    let chat_id = parse_chat_id(&id)?;

    let chat = state
        .memory
        .get_chat(chat_id)
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("conversation '{id}' not found")))?;

    let messages = state
        .memory
        .get_messages(chat_id)
        .map_err(|e| AppError::Internal(e.to_string()))?
        .into_iter()
        .map(|m| MessageSummary {
            id: m.id.to_string(),
            role: m.role.to_string(),
            content: m.content,
            timestamp: m.timestamp.to_rfc3339(),
        })
        .collect();

    Ok(Json(ConversationDetail {
        id: chat.id.to_string(),
        title: chat.title,
        model: chat.model,
        created_at: chat.created_at,
        messages,
    }))
}

/// `POST /api/conversations` — create a new conversation.
pub async fn create_conversation(
    State(state): State<AppState>,
    Json(payload): Json<CreateConversationRequest>,
) -> Result<(StatusCode, Json<CreateConversationResponse>), AppError> {
    let id = ChatId::new();
    let title = payload.title.as_deref().unwrap_or("New Conversation");
    let model = payload.model.as_deref().unwrap_or("openai/gpt-4o");

    state
        .memory
        .create_chat(id, title, None, None, None, model)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(CreateConversationResponse { id: id.to_string() })))
}

/// `DELETE /api/conversations/{id}` — delete a conversation and its messages.
pub async fn delete_conversation(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteConversationResponse>, AppError> {
    let chat_id = parse_chat_id(&id)?;

    let deleted = state
        .memory
        .delete_chat(chat_id)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(DeleteConversationResponse { deleted }))
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

    use hyprcollab_core::types::MessageRole;

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

    fn json_post(uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap()
    }

    fn get_req(uri: &str) -> Request<Body> {
        Request::builder().method("GET").uri(uri).body(Body::empty()).unwrap()
    }

    fn delete_req(uri: &str) -> Request<Body> {
        Request::builder().method("DELETE").uri(uri).body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn create_conversation_returns_201() {
        let state = test_state().await;
        let app = create_app(state);

        let body = json!({"title": "Test Chat", "model": "openai/gpt-4o"});
        let resp = app.oneshot(json_post("/api/conversations", body)).await.unwrap();

        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(val["id"].as_str().is_some(), "should return an id");
    }

    #[tokio::test]
    async fn create_with_defaults_works() {
        let state = test_state().await;
        let app = create_app(state);

        let resp = app.oneshot(json_post("/api/conversations", json!({}))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn list_conversations_empty_initially() {
        let state = test_state().await;
        let app = create_app(state);

        let resp = app.oneshot(get_req("/api/conversations")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["conversations"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn list_conversations_shows_created() {
        let state = test_state().await;

        // Create directly via memory store
        let id = hyprcollab_core::types::ChatId::new();
        state
            .memory
            .create_chat(id, "My Chat", None, None, None, "gpt-4")
            .unwrap();

        let app = create_app(state);
        let resp = app.oneshot(get_req("/api/conversations")).await.unwrap();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["conversations"].as_array().unwrap().len(), 1);
        assert_eq!(val["conversations"][0]["title"].as_str(), Some("My Chat"));
    }

    #[tokio::test]
    async fn get_conversation_not_found() {
        let state = test_state().await;
        let app = create_app(state);

        let fake_id = uuid::Uuid::new_v4();
        let resp = app
            .oneshot(get_req(&format!("/api/conversations/{fake_id}")))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn get_conversation_invalid_id_returns_400() {
        let state = test_state().await;
        let app = create_app(state);

        let resp = app
            .oneshot(get_req("/api/conversations/not-a-uuid"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_then_get_conversation() {
        let state = test_state().await;

        let id = hyprcollab_core::types::ChatId::new();
        state.memory.create_chat(id, "Round-trip", None, None, None, "claude-3").unwrap();

        let app = create_app(state);
        let resp = app
            .oneshot(get_req(&format!("/api/conversations/{}", id)))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["title"].as_str(), Some("Round-trip"));
        assert_eq!(val["model"].as_str(), Some("claude-3"));
        assert!(val["messages"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn delete_conversation_returns_true() {
        let state = test_state().await;

        let id = hyprcollab_core::types::ChatId::new();
        state.memory.create_chat(id, "Bye", None, None, None, "gpt-4").unwrap();

        let app = create_app(state);
        let resp = app
            .oneshot(delete_req(&format!("/api/conversations/{}", id)))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["deleted"].as_bool(), Some(true));
    }

    #[tokio::test]
    async fn delete_nonexistent_conversation_returns_false() {
        let state = test_state().await;
        let app = create_app(state);

        let fake_id = uuid::Uuid::new_v4();
        let resp = app
            .oneshot(delete_req(&format!("/api/conversations/{fake_id}")))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["deleted"].as_bool(), Some(false));
    }

    #[tokio::test]
    async fn get_conversation_includes_messages() {
        let state = test_state().await;

        let chat_id = hyprcollab_core::types::ChatId::new();
        state.memory.create_chat(chat_id, "With Messages", None, None, None, "gpt-4").unwrap();

        let msg = hyprcollab_core::types::Message {
            id: hyprcollab_core::types::MessageId::new(),
            chat_id,
            role: MessageRole::User,
            content: "Hello there".into(),
            tool_calls: vec![],
            artifacts: vec![],
            timestamp: chrono::Utc::now(),
            metadata: serde_json::Value::Null,
        };
        state.memory.add_message(&msg).unwrap();

        let app = create_app(state);
        let resp = app
            .oneshot(get_req(&format!("/api/conversations/{}", chat_id)))
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["messages"].as_array().unwrap().len(), 1);
        assert_eq!(val["messages"][0]["content"].as_str(), Some("Hello there"));
        assert_eq!(val["messages"][0]["role"].as_str(), Some("user"));
    }
}
