use axum::{
    extract::State,
    http::Method,
    response::{IntoResponse, Response, sse::Event, Sse},
    routing::{get, post},
    Json, Router,
};
use tower_http::cors::{Any, CorsLayer};
use std::convert::Infallible;
use futures::stream;

use crate::state::AppState;
use crate::error::AppError;

/// Request payload for chat completions.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub model: String,
    pub persona_id: Option<String>,
    pub stream: Option<bool>,
}

/// Response payload for non-streaming chat completions.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ChatResponse {
    pub response: String,
    pub model: String,
    pub persona_id: Option<String>,
}

/// Response chunk for streaming chat completions.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ChatStreamChunk {
    pub content: String,
    pub done: bool,
    pub model: String,
    pub persona_id: Option<String>,
}

/// Response payload for health checks.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Request payload for tool approvals.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ApprovalRequest {
    pub tool_name: String,
    pub arguments: String,
    pub context: Option<String>,
}

/// Response payload for tool approvals.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ApprovalResponse {
    pub approved: bool,
    pub reason: Option<String>,
}

/// Create the Axum Router configured with CORS and endpoints.
pub fn create_app(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/chat", post(chat_handler))
        .route("/api/tools/approve", post(approve_handler))
        .layer(cors)
        .with_state(state)
}

async fn health_handler(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: state.version.clone(),
    })
}

async fn chat_handler(
    State(_state): State<AppState>,
    Json(payload): Json<ChatRequest>,
) -> Result<Response, AppError> {
    if payload.message.trim().is_empty() {
        return Err(AppError::BadRequest("Message cannot be empty".to_string()));
    }

    let is_streaming = payload.stream.unwrap_or(false);

    if is_streaming {
        let response_text = format!("Response to: {}", payload.message);
        let words: Vec<String> = response_text
            .split_whitespace()
            .map(|w| format!("{} ", w))
            .collect();
        
        let model = payload.model.clone();
        let persona_id = payload.persona_id.clone();
        
        let mut events = Vec::new();
        let len = words.len();
        
        for (i, word) in words.into_iter().enumerate() {
            let done = i == len - 1;
            let chunk = ChatStreamChunk {
                content: word,
                done,
                model: model.clone(),
                persona_id: persona_id.clone(),
            };
            
            let event = Event::default()
                .json_data(&chunk)
                .map_err(|e| AppError::Internal(e.to_string()))?;
            events.push(Ok::<Event, Infallible>(event));
        }
        
        let stream = stream::iter(events);
        Ok(Sse::new(stream).into_response())
    } else {
        let response = ChatResponse {
            response: format!("Response to: {}", payload.message),
            model: payload.model,
            persona_id: payload.persona_id,
        };
        Ok(Json(response).into_response())
    }
}

async fn approve_handler(
    State(state): State<AppState>,
    Json(payload): Json<ApprovalRequest>,
) -> Result<Json<ApprovalResponse>, AppError> {
    if payload.tool_name.trim().is_empty() {
        return Err(AppError::BadRequest("Tool name cannot be empty".to_string()));
    }

    {
        let mut approvals = state
            .approvals
            .lock()
            .map_err(|_| AppError::Internal("Lock poisoned".to_string()))?;
        approvals.push(payload.tool_name.clone());
    }

    let approved = !payload.tool_name.starts_with("unsafe");
    let reason = if approved {
        Some(format!("Tool '{}' execution is approved by default policy", payload.tool_name))
    } else {
        Some(format!("Tool '{}' execution requires explicit user authorization", payload.tool_name))
    };

    Ok(Json(ApprovalResponse { approved, reason }))
}
