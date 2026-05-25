//! Extra Features II — token usage, export, templates, session unread (F5 S25).

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use hyprcollab_core::types::{ChatId, MessageId};
use hyprcollab_memory::{
    DailyUsage, ExportFormat, ExportOptions, Exporter, PromptTemplate, TemplateEngine,
    TemplateId, TokenTracker, TokenUsageSummary,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_chat_id(s: &str) -> Result<ChatId, AppError> {
    Uuid::parse_str(s)
        .map(ChatId::from)
        .map_err(|_| AppError::BadRequest(format!("invalid conversation id: {s}")))
}

fn parse_template_id(s: &str) -> Result<TemplateId, AppError> {
    Uuid::parse_str(s)
        .map(TemplateId)
        .map_err(|_| AppError::BadRequest(format!("invalid template id: {s}")))
}

fn parse_message_id(s: &str) -> Result<MessageId, AppError> {
    Uuid::parse_str(s)
        .map(MessageId)
        .map_err(|_| AppError::BadRequest(format!("invalid message id: {s}")))
}

// ── Token usage types ─────────────────────────────────────────────────────────

/// A single recorded token usage event.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct TokenUsageItem {
    pub id: String,
    pub chat_id: Option<String>,
    pub model: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub created_at: String,
}

/// Aggregate usage summary.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct UsageSummaryResponse {
    pub total_tokens: i64,
    pub total_prompt: i64,
    pub total_completion: i64,
    pub by_model: HashMap<String, ModelUsageSummary>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ModelUsageSummary {
    pub total_tokens: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub call_count: i64,
}

/// Query parameters for the usage summary endpoint.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct UsageQuery {
    pub since: Option<String>,
    pub model: Option<String>,
}

/// Daily usage aggregate.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DailyUsageItem {
    pub date: String,
    pub total_tokens: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

/// Response for daily usage.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DailyUsageResponse {
    pub days: Vec<DailyUsageItem>,
}

/// Query parameters for daily usage.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct DailyQuery {
    pub days: Option<i64>,
}

// ── Export types ──────────────────────────────────────────────────────────────

/// Response for export endpoint.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ExportResponse {
    pub format: String,
    pub content: String,
}

/// Query parameters for the export endpoint.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ExportQuery {
    pub format: Option<String>,
    pub include_metadata: Option<bool>,
    pub include_tool_calls: Option<bool>,
    pub include_artifacts: Option<bool>,
}

// ── Template types ────────────────────────────────────────────────────────────

/// A template as returned by the API.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct TemplateResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub template: String,
    pub variables: Vec<String>,
    pub category: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// List of templates.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ListTemplatesResponse {
    pub templates: Vec<TemplateResponse>,
}

/// Request body to create a template.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateTemplateRequest {
    pub name: String,
    pub description: Option<String>,
    pub template: String,
    pub variables: Option<Vec<String>>,
    pub category: Option<String>,
}

/// Request body to update a template.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateTemplateRequest {
    pub name: String,
    pub description: Option<String>,
    pub template: String,
    pub variables: Option<Vec<String>>,
    pub category: Option<String>,
}

/// Request body to render a template.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct RenderTemplateRequest {
    pub variables: HashMap<String, String>,
}

/// Response after rendering a template.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct RenderTemplateResponse {
    pub rendered: String,
}

/// Response after deleting a template.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DeleteTemplateResponse {
    pub deleted: bool,
}

// ── Session types ─────────────────────────────────────────────────────────────

/// Response for the unread count endpoint.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct UnreadResponse {
    pub chat_id: String,
    pub unread_count: usize,
    pub last_read_message_id: Option<String>,
}

/// Request body to mark a chat as read.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct MarkReadRequest {
    pub message_id: String,
}

/// Response after marking as read.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MarkReadResponse {
    pub marked: bool,
}

// ── Converters ────────────────────────────────────────────────────────────────

fn summary_response(s: TokenUsageSummary) -> UsageSummaryResponse {
    UsageSummaryResponse {
        total_tokens: s.total_tokens,
        total_prompt: s.total_prompt,
        total_completion: s.total_completion,
        by_model: s
            .by_model
            .into_iter()
            .map(|(k, v)| {
                (
                    k,
                    ModelUsageSummary {
                        total_tokens: v.total_tokens,
                        prompt_tokens: v.prompt_tokens,
                        completion_tokens: v.completion_tokens,
                        call_count: v.call_count,
                    },
                )
            })
            .collect(),
    }
}

fn daily_item(d: DailyUsage) -> DailyUsageItem {
    DailyUsageItem {
        date: d.date,
        total_tokens: d.total_tokens,
        prompt_tokens: d.prompt_tokens,
        completion_tokens: d.completion_tokens,
    }
}

fn template_response(t: PromptTemplate) -> TemplateResponse {
    TemplateResponse {
        id: t.id.to_string(),
        name: t.name,
        description: t.description,
        template: t.template,
        variables: t.variables,
        category: t.category,
        created_at: t.created_at,
        updated_at: t.updated_at,
    }
}

fn export_format(s: &str) -> ExportFormat {
    match s.to_lowercase().as_str() {
        "json" => ExportFormat::Json,
        _ => ExportFormat::Markdown,
    }
}

// ── Token usage handlers ──────────────────────────────────────────────────────

/// `GET /api/usage` — aggregate token usage summary.
#[utoipa::path(
    get,
    path = "/api/usage",
    params(UsageQuery),
    responses(
        (status = 200, description = "Usage summary", body = UsageSummaryResponse),
        (status = 500, description = "Storage error"),
    ),
    tag = "Usage"
)]
pub async fn get_usage_summary(
    State(state): State<AppState>,
    Query(params): Query<UsageQuery>,
) -> Result<Json<UsageSummaryResponse>, AppError> {
    let tracker = TokenTracker::new(&state.memory);
    let summary = tracker
        .get_usage_summary(params.since.as_deref())
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(summary_response(summary)))
}

/// `GET /api/usage/daily` — daily token usage aggregates.
#[utoipa::path(
    get,
    path = "/api/usage/daily",
    params(DailyQuery),
    responses(
        (status = 200, description = "Daily usage", body = DailyUsageResponse),
        (status = 500, description = "Storage error"),
    ),
    tag = "Usage"
)]
pub async fn get_daily_usage(
    State(state): State<AppState>,
    Query(params): Query<DailyQuery>,
) -> Result<Json<DailyUsageResponse>, AppError> {
    let days = params.days.unwrap_or(30).min(365);
    let tracker = TokenTracker::new(&state.memory);
    let daily = tracker
        .get_daily_usage(days)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(DailyUsageResponse { days: daily.into_iter().map(daily_item).collect() }))
}

// ── Export handler ────────────────────────────────────────────────────────────

/// `GET /api/conversations/{id}/export` — export a conversation.
#[utoipa::path(
    get,
    path = "/api/conversations/{id}/export",
    params(
        ("id" = String, Path, description = "Conversation UUID"),
        ExportQuery,
    ),
    responses(
        (status = 200, description = "Exported conversation", body = ExportResponse),
        (status = 400, description = "Invalid UUID"),
        (status = 404, description = "Conversation not found"),
        (status = 500, description = "Storage error"),
    ),
    tag = "Conversations"
)]
pub async fn export_conversation(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<ExportQuery>,
) -> Result<Json<ExportResponse>, AppError> {
    let chat_id = parse_chat_id(&id)?;
    let fmt = export_format(params.format.as_deref().unwrap_or("markdown"));
    let opts = ExportOptions {
        format: fmt,
        include_metadata: params.include_metadata.unwrap_or(false),
        include_tool_calls: params.include_tool_calls.unwrap_or(false),
        include_artifacts: params.include_artifacts.unwrap_or(false),
    };

    let exporter = Exporter::new(&state.memory);
    let content = exporter
        .export_chat(chat_id, &opts)
        .map_err(|e| {
            if e.to_string().contains("not found") {
                AppError::NotFound(format!("conversation '{id}' not found"))
            } else {
                AppError::Internal(e.to_string())
            }
        })?;

    Ok(Json(ExportResponse {
        format: format!("{:?}", fmt).to_lowercase(),
        content,
    }))
}

// ── Template handlers ─────────────────────────────────────────────────────────

/// `GET /api/templates` — list all templates.
#[utoipa::path(
    get,
    path = "/api/templates",
    responses(
        (status = 200, description = "All templates", body = ListTemplatesResponse),
        (status = 500, description = "Storage error"),
    ),
    tag = "Templates"
)]
pub async fn list_templates(
    State(state): State<AppState>,
) -> Result<Json<ListTemplatesResponse>, AppError> {
    let engine = TemplateEngine::new(&state.memory);
    let templates = engine
        .list_templates()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(ListTemplatesResponse {
        templates: templates.into_iter().map(template_response).collect(),
    }))
}

/// `POST /api/templates` — create a template.
#[utoipa::path(
    post,
    path = "/api/templates",
    request_body = CreateTemplateRequest,
    responses(
        (status = 201, description = "Template created", body = TemplateResponse),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Storage error"),
    ),
    tag = "Templates"
)]
pub async fn create_template(
    State(state): State<AppState>,
    Json(body): Json<CreateTemplateRequest>,
) -> Result<(StatusCode, Json<TemplateResponse>), AppError> {
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("template name cannot be empty".into()));
    }

    let engine = TemplateEngine::new(&state.memory);
    let tmpl = engine
        .create_template(
            &body.name,
            body.description,
            &body.template,
            body.variables.unwrap_or_default(),
            body.category,
        )
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(template_response(tmpl))))
}

/// `PUT /api/templates/{id}` — update a template.
#[utoipa::path(
    put,
    path = "/api/templates/{id}",
    params(("id" = String, Path, description = "Template UUID")),
    request_body = UpdateTemplateRequest,
    responses(
        (status = 200, description = "Template updated", body = TemplateResponse),
        (status = 400, description = "Invalid UUID or empty name"),
        (status = 404, description = "Template not found"),
        (status = 500, description = "Storage error"),
    ),
    tag = "Templates"
)]
pub async fn update_template(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateTemplateRequest>,
) -> Result<Json<TemplateResponse>, AppError> {
    let tmpl_id = parse_template_id(&id)?;

    let engine = TemplateEngine::new(&state.memory);
    let found = engine
        .update_template(
            tmpl_id,
            &body.name,
            body.description,
            &body.template,
            body.variables.unwrap_or_default(),
            body.category,
        )
        .map_err(|e| AppError::Internal(e.to_string()))?;

    if !found {
        return Err(AppError::NotFound(format!("template '{id}' not found")));
    }

    let tmpl = engine
        .get_template(tmpl_id)
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("template '{id}' not found")))?;

    Ok(Json(template_response(tmpl)))
}

/// `DELETE /api/templates/{id}` — delete a template.
#[utoipa::path(
    delete,
    path = "/api/templates/{id}",
    params(("id" = String, Path, description = "Template UUID")),
    responses(
        (status = 200, description = "Deletion result", body = DeleteTemplateResponse),
        (status = 400, description = "Invalid UUID"),
        (status = 500, description = "Storage error"),
    ),
    tag = "Templates"
)]
pub async fn delete_template(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteTemplateResponse>, AppError> {
    let tmpl_id = parse_template_id(&id)?;
    let engine = TemplateEngine::new(&state.memory);
    let deleted = engine
        .delete_template(tmpl_id)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(DeleteTemplateResponse { deleted }))
}

/// `POST /api/templates/{id}/render` — render a template with variable substitution.
#[utoipa::path(
    post,
    path = "/api/templates/{id}/render",
    params(("id" = String, Path, description = "Template UUID")),
    request_body = RenderTemplateRequest,
    responses(
        (status = 200, description = "Rendered template", body = RenderTemplateResponse),
        (status = 400, description = "Invalid UUID or missing variables"),
        (status = 404, description = "Template not found"),
        (status = 500, description = "Storage error"),
    ),
    tag = "Templates"
)]
pub async fn render_template(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<RenderTemplateRequest>,
) -> Result<Json<RenderTemplateResponse>, AppError> {
    let tmpl_id = parse_template_id(&id)?;
    let engine = TemplateEngine::new(&state.memory);
    let rendered = engine
        .render_template(tmpl_id, &body.variables)
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("not found") {
                AppError::NotFound(format!("template '{id}' not found"))
            } else if msg.contains("missing required variable") {
                AppError::BadRequest(msg)
            } else {
                AppError::Internal(msg)
            }
        })?;

    Ok(Json(RenderTemplateResponse { rendered }))
}

// ── Session handlers ──────────────────────────────────────────────────────────

/// `GET /api/conversations/{id}/unread` — get unread message count.
#[utoipa::path(
    get,
    path = "/api/conversations/{id}/unread",
    params(("id" = String, Path, description = "Conversation UUID")),
    responses(
        (status = 200, description = "Unread count", body = UnreadResponse),
        (status = 400, description = "Invalid UUID"),
        (status = 404, description = "Conversation not found"),
        (status = 500, description = "Storage error"),
    ),
    tag = "Conversations"
)]
pub async fn get_unread_count(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<UnreadResponse>, AppError> {
    let chat_id = parse_chat_id(&id)?;
    let result = state
        .memory
        .get_chat_with_unread(chat_id)
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("conversation '{id}' not found")))?;

    let (chat, unread_count) = result;
    Ok(Json(UnreadResponse {
        chat_id: chat.id.to_string(),
        unread_count,
        last_read_message_id: chat.last_read_message_id.map(|m| m.to_string()),
    }))
}

/// `POST /api/conversations/{id}/read` — mark a conversation as read up to a message.
#[utoipa::path(
    post,
    path = "/api/conversations/{id}/read",
    params(("id" = String, Path, description = "Conversation UUID")),
    request_body = MarkReadRequest,
    responses(
        (status = 200, description = "Marked as read", body = MarkReadResponse),
        (status = 400, description = "Invalid UUID"),
        (status = 404, description = "Conversation not found"),
        (status = 500, description = "Storage error"),
    ),
    tag = "Conversations"
)]
pub async fn mark_conversation_read(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<MarkReadRequest>,
) -> Result<Json<MarkReadResponse>, AppError> {
    let chat_id = parse_chat_id(&id)?;
    let msg_id = parse_message_id(&body.message_id)?;

    let marked = state
        .memory
        .mark_chat_read(chat_id, msg_id)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    if !marked {
        return Err(AppError::NotFound(format!("conversation '{id}' not found")));
    }

    Ok(Json(MarkReadResponse { marked }))
}
