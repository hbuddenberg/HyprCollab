//! REST handlers for `/api/memory/facts` — working memory (F5 S21).

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use hyprcollab_memory::{Fact, FactCategory, FactId, WorkingMemory};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::state::AppState;

// ── Request / Response types ──────────────────────────────────────────────────

/// Request body to create a new fact.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateFactRequest {
    pub category: String,
    pub content: String,
    pub confidence: Option<f64>,
    pub source: Option<String>,
}

/// Request body to update a fact's content and/or confidence.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateFactRequest {
    pub content: Option<String>,
    pub confidence: Option<f64>,
}

/// Request body to prune facts below a confidence threshold.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct PruneFactsRequest {
    pub min_confidence: f64,
}

/// A single fact as returned by the API.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct FactResponse {
    pub id: String,
    pub category: String,
    pub content: String,
    pub confidence: f64,
    pub source: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub access_count: i64,
}

/// Response listing multiple facts.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ListFactsResponse {
    pub facts: Vec<FactResponse>,
}

/// Response after deleting a fact.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DeleteFactResponse {
    pub deleted: bool,
}

/// Response after pruning low-confidence facts.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PruneFactsResponse {
    pub pruned: usize,
}

/// Query parameters for listing facts.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ListFactsParams {
    /// Filter by category (preference, fact, pattern, correction, environment).
    pub category: Option<String>,
    /// Maximum number of results to return.
    pub limit: Option<usize>,
}

/// Query parameters for searching facts.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct SearchFactsParams {
    /// FTS5 search query string.
    pub q: String,
    /// Maximum number of results to return (default: 20).
    pub limit: Option<usize>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_fact_id(s: &str) -> Result<FactId, AppError> {
    uuid::Uuid::parse_str(s)
        .map(FactId)
        .map_err(|_| AppError::BadRequest(format!("invalid fact id: {s}")))
}

fn fact_to_response(fact: Fact) -> FactResponse {
    FactResponse {
        id: fact.id.to_string(),
        category: fact.category.to_string(),
        content: fact.content,
        confidence: fact.confidence,
        source: fact.source,
        created_at: fact.created_at,
        updated_at: fact.updated_at,
        access_count: fact.access_count,
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// `GET /api/memory/facts` — list all facts with optional category filter.
#[utoipa::path(
    get,
    path = "/api/memory/facts",
    params(ListFactsParams),
    responses(
        (status = 200, description = "List of facts", body = ListFactsResponse),
        (status = 400, description = "Invalid category"),
        (status = 500, description = "Storage error"),
    ),
    tag = "WorkingMemory"
)]
pub async fn list_facts(
    State(state): State<AppState>,
    Query(params): Query<ListFactsParams>,
) -> Result<Json<ListFactsResponse>, AppError> {
    let category = params
        .category
        .as_deref()
        .map(|s| s.parse::<FactCategory>())
        .transpose()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let wm = WorkingMemory::new(&state.memory);
    let facts = wm
        .list_facts(category, params.limit)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(ListFactsResponse { facts: facts.into_iter().map(fact_to_response).collect() }))
}

/// `POST /api/memory/facts` — create a new fact.
#[utoipa::path(
    post,
    path = "/api/memory/facts",
    request_body = CreateFactRequest,
    responses(
        (status = 201, description = "Fact created", body = FactResponse),
        (status = 400, description = "Invalid category or request body"),
        (status = 500, description = "Storage error"),
    ),
    tag = "WorkingMemory"
)]
pub async fn create_fact(
    State(state): State<AppState>,
    Json(body): Json<CreateFactRequest>,
) -> Result<(StatusCode, Json<FactResponse>), AppError> {
    let category = body
        .category
        .parse::<FactCategory>()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let wm = WorkingMemory::new(&state.memory);
    let fact = wm
        .add_fact(category, body.content, body.confidence.unwrap_or(0.8), body.source)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(fact_to_response(fact))))
}

/// `GET /api/memory/facts/search` — full-text search over facts.
#[utoipa::path(
    get,
    path = "/api/memory/facts/search",
    params(SearchFactsParams),
    responses(
        (status = 200, description = "Search results", body = ListFactsResponse),
        (status = 400, description = "Missing query parameter"),
        (status = 500, description = "Storage error"),
    ),
    tag = "WorkingMemory"
)]
pub async fn search_facts(
    State(state): State<AppState>,
    Query(params): Query<SearchFactsParams>,
) -> Result<Json<ListFactsResponse>, AppError> {
    let limit = params.limit.unwrap_or(20);
    let wm = WorkingMemory::new(&state.memory);
    let facts = wm
        .search_facts(&params.q, limit)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(ListFactsResponse { facts: facts.into_iter().map(fact_to_response).collect() }))
}

/// `GET /api/memory/facts/:id` — get a single fact.
#[utoipa::path(
    get,
    path = "/api/memory/facts/{id}",
    params(("id" = String, Path, description = "Fact UUID")),
    responses(
        (status = 200, description = "Fact found", body = FactResponse),
        (status = 400, description = "Invalid id"),
        (status = 404, description = "Fact not found"),
        (status = 500, description = "Storage error"),
    ),
    tag = "WorkingMemory"
)]
pub async fn get_fact(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<FactResponse>, AppError> {
    let fact_id = parse_fact_id(&id)?;
    let wm = WorkingMemory::new(&state.memory);

    wm.touch_fact(fact_id).map_err(|e| AppError::Internal(e.to_string()))?;

    let fact = wm
        .get_fact(fact_id)
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("fact {id} not found")))?;

    Ok(Json(fact_to_response(fact)))
}

/// `PUT /api/memory/facts/:id` — update a fact's content and/or confidence.
#[utoipa::path(
    put,
    path = "/api/memory/facts/{id}",
    params(("id" = String, Path, description = "Fact UUID")),
    request_body = UpdateFactRequest,
    responses(
        (status = 200, description = "Fact updated", body = FactResponse),
        (status = 400, description = "Invalid id"),
        (status = 404, description = "Fact not found"),
        (status = 500, description = "Storage error"),
    ),
    tag = "WorkingMemory"
)]
pub async fn update_fact(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateFactRequest>,
) -> Result<Json<FactResponse>, AppError> {
    let fact_id = parse_fact_id(&id)?;
    let wm = WorkingMemory::new(&state.memory);

    let fact = wm
        .update_fact(fact_id, body.content, body.confidence)
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("fact {id} not found")))?;

    Ok(Json(fact_to_response(fact)))
}

/// `DELETE /api/memory/facts/:id` — delete a fact.
#[utoipa::path(
    delete,
    path = "/api/memory/facts/{id}",
    params(("id" = String, Path, description = "Fact UUID")),
    responses(
        (status = 200, description = "Deletion result", body = DeleteFactResponse),
        (status = 400, description = "Invalid id"),
        (status = 500, description = "Storage error"),
    ),
    tag = "WorkingMemory"
)]
pub async fn delete_fact(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteFactResponse>, AppError> {
    let fact_id = parse_fact_id(&id)?;
    let wm = WorkingMemory::new(&state.memory);

    let deleted = wm
        .delete_fact(fact_id)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(DeleteFactResponse { deleted }))
}

/// `POST /api/memory/facts/prune` — delete facts below a confidence threshold.
#[utoipa::path(
    post,
    path = "/api/memory/facts/prune",
    request_body = PruneFactsRequest,
    responses(
        (status = 200, description = "Number of pruned facts", body = PruneFactsResponse),
        (status = 500, description = "Storage error"),
    ),
    tag = "WorkingMemory"
)]
pub async fn prune_facts(
    State(state): State<AppState>,
    Json(body): Json<PruneFactsRequest>,
) -> Result<Json<PruneFactsResponse>, AppError> {
    let wm = WorkingMemory::new(&state.memory);
    let pruned = wm
        .prune_facts(body.min_confidence)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(PruneFactsResponse { pruned }))
}
