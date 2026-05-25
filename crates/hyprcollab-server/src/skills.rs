//! REST handlers for `/api/skills` — skills engine (F5 S23).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use hyprcollab_skills::{
    learner::SkillLearner, SkillCreateRequest, SkillId, SkillMatcher, SkillUpdateRequest,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

// ── Request / Response types ──────────────────────────────────────────────────

/// Skill descriptor returned by the API.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct SkillResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: Option<String>,
    pub trigger_patterns: Vec<String>,
    pub instructions: String,
    pub enabled: bool,
    pub priority: i32,
    pub usage_count: i64,
    pub success_rate: f64,
    pub created_at: String,
    pub updated_at: String,
}

/// Paginated list of skills.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ListSkillsResponse {
    pub skills: Vec<SkillResponse>,
    pub total: usize,
}

/// Request body to create a skill.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateSkillRequest {
    pub name: String,
    pub description: String,
    pub category: Option<String>,
    pub trigger_patterns: Vec<String>,
    pub instructions: String,
    pub enabled: Option<bool>,
    pub priority: Option<i32>,
}

/// Request body to partially update a skill.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateSkillRequest {
    pub description: Option<String>,
    pub category: Option<String>,
    pub trigger_patterns: Option<Vec<String>>,
    pub instructions: Option<String>,
    pub enabled: Option<bool>,
    pub priority: Option<i32>,
}

/// Request body to match skills against a user query.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct MatchSkillsRequest {
    pub query: String,
}

/// Response for skill matching.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MatchSkillsResponse {
    pub matched: Vec<SkillResponse>,
    pub injected_prompt: Option<String>,
    pub system_prompt: Option<String>,
}

/// Request body for skill auto-learning.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct LearnSkillsRequest {
    pub messages: Vec<String>,
}

/// Response for skill learning.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct LearnSkillsResponse {
    pub suggested_name: Option<String>,
    pub suggested_patterns: Vec<String>,
    pub suggested_instructions: Option<String>,
    pub confidence: f64,
    pub evidence_count: usize,
    pub skill_created: bool,
    pub skill: Option<SkillResponse>,
}

/// Response after deleting a skill.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DeleteSkillResponse {
    pub deleted: bool,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn skill_to_response(s: hyprcollab_skills::Skill) -> SkillResponse {
    SkillResponse {
        id: s.id.to_string(),
        name: s.name,
        description: s.description,
        category: s.category,
        trigger_patterns: s.trigger_patterns,
        instructions: s.instructions,
        enabled: s.enabled,
        priority: s.priority,
        usage_count: s.usage_count,
        success_rate: s.success_rate,
        created_at: s.created_at,
        updated_at: s.updated_at,
    }
}

fn parse_skill_id(raw: &str) -> Result<SkillId, AppError> {
    Uuid::parse_str(raw)
        .map(SkillId)
        .map_err(|_| AppError::BadRequest(format!("invalid skill id: {raw}")))
}

fn map_store_err(e: hyprcollab_skills::store::StoreError) -> AppError {
    match e {
        hyprcollab_skills::store::StoreError::NotFound(id) => {
            AppError::NotFound(format!("skill '{id}' not found"))
        }
        e => AppError::Internal(e.to_string()),
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// `GET /api/skills` — list all skills.
#[utoipa::path(
    get,
    path = "/api/skills",
    responses(
        (status = 200, description = "All skills ordered by priority", body = ListSkillsResponse),
        (status = 500, description = "Database error"),
    ),
    tag = "Skills"
)]
pub async fn list_skills(State(state): State<AppState>) -> Result<Json<ListSkillsResponse>, AppError> {
    let skills = state.skills.list_skills().map_err(map_store_err)?;
    let total = skills.len();
    Ok(Json(ListSkillsResponse {
        skills: skills.into_iter().map(skill_to_response).collect(),
        total,
    }))
}

/// `POST /api/skills` — create a new skill.
#[utoipa::path(
    post,
    path = "/api/skills",
    request_body = CreateSkillRequest,
    responses(
        (status = 201, description = "Skill created", body = SkillResponse),
        (status = 400, description = "Validation error"),
        (status = 500, description = "Database error"),
    ),
    tag = "Skills"
)]
pub async fn create_skill(
    State(state): State<AppState>,
    Json(body): Json<CreateSkillRequest>,
) -> Result<(StatusCode, Json<SkillResponse>), AppError> {
    let req = SkillCreateRequest {
        name: body.name,
        description: body.description,
        category: body.category,
        trigger_patterns: body.trigger_patterns,
        instructions: body.instructions,
        enabled: body.enabled,
        priority: body.priority,
    };
    let skill = state.skills.add_skill(&req).map_err(map_store_err)?;
    Ok((StatusCode::CREATED, Json(skill_to_response(skill))))
}

/// `GET /api/skills/{id}` — get a skill by ID.
#[utoipa::path(
    get,
    path = "/api/skills/{id}",
    params(("id" = String, Path, description = "Skill UUID")),
    responses(
        (status = 200, description = "Skill found", body = SkillResponse),
        (status = 404, description = "Skill not found"),
    ),
    tag = "Skills"
)]
pub async fn get_skill(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<SkillResponse>, AppError> {
    let skill_id = parse_skill_id(&id)?;
    let skill = state.skills.get_skill(skill_id).map_err(map_store_err)?;
    Ok(Json(skill_to_response(skill)))
}

/// `PUT /api/skills/{id}` — update a skill.
#[utoipa::path(
    put,
    path = "/api/skills/{id}",
    params(("id" = String, Path, description = "Skill UUID")),
    request_body = UpdateSkillRequest,
    responses(
        (status = 200, description = "Skill updated", body = SkillResponse),
        (status = 404, description = "Skill not found"),
    ),
    tag = "Skills"
)]
pub async fn update_skill(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateSkillRequest>,
) -> Result<Json<SkillResponse>, AppError> {
    let skill_id = parse_skill_id(&id)?;
    let req = SkillUpdateRequest {
        description: body.description,
        category: body.category,
        trigger_patterns: body.trigger_patterns,
        instructions: body.instructions,
        enabled: body.enabled,
        priority: body.priority,
    };
    let skill = state.skills.update_skill(skill_id, &req).map_err(map_store_err)?;
    Ok(Json(skill_to_response(skill)))
}

/// `DELETE /api/skills/{id}` — delete a skill.
#[utoipa::path(
    delete,
    path = "/api/skills/{id}",
    params(("id" = String, Path, description = "Skill UUID")),
    responses(
        (status = 200, description = "Skill deleted", body = DeleteSkillResponse),
        (status = 404, description = "Skill not found"),
    ),
    tag = "Skills"
)]
pub async fn delete_skill(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteSkillResponse>, AppError> {
    let skill_id = parse_skill_id(&id)?;
    state.skills.delete_skill(skill_id).map_err(map_store_err)?;
    Ok(Json(DeleteSkillResponse { deleted: true }))
}

/// `POST /api/skills/match` — match skills against a user query.
#[utoipa::path(
    post,
    path = "/api/skills/match",
    request_body = MatchSkillsRequest,
    responses(
        (status = 200, description = "Matched skills with optional injected prompt", body = MatchSkillsResponse),
        (status = 400, description = "Empty query"),
        (status = 500, description = "Database error"),
    ),
    tag = "Skills"
)]
pub async fn match_skills_handler(
    State(state): State<AppState>,
    Json(body): Json<MatchSkillsRequest>,
) -> Result<Json<MatchSkillsResponse>, AppError> {
    if body.query.trim().is_empty() {
        return Err(AppError::BadRequest("query cannot be empty".into()));
    }
    let all_skills = state.skills.list_skills().map_err(map_store_err)?;
    let matcher = SkillMatcher::new();
    let matched_refs: Vec<&hyprcollab_skills::Skill> =
        matcher.match_skills(&body.query, &all_skills);

    let injected_prompt = if matched_refs.is_empty() {
        None
    } else {
        Some(SkillMatcher::inject_skills("", &matched_refs))
    };

    Ok(Json(MatchSkillsResponse {
        matched: matched_refs.iter().map(|s| skill_to_response((*s).clone())).collect(),
        injected_prompt,
        system_prompt: None,
    }))
}

/// `POST /api/skills/learn` — auto-detect patterns and optionally persist a new skill.
#[utoipa::path(
    post,
    path = "/api/skills/learn",
    request_body = LearnSkillsRequest,
    responses(
        (status = 200, description = "Learning result — suggestion and optionally created skill", body = LearnSkillsResponse),
        (status = 400, description = "No messages provided"),
        (status = 500, description = "Database error"),
    ),
    tag = "Skills"
)]
pub async fn learn_skills(
    State(state): State<AppState>,
    Json(body): Json<LearnSkillsRequest>,
) -> Result<Json<LearnSkillsResponse>, AppError> {
    if body.messages.is_empty() {
        return Err(AppError::BadRequest("messages cannot be empty".into()));
    }

    let learner = SkillLearner::new();
    let Some(suggestion) = learner.detect_patterns(&body.messages) else {
        return Ok(Json(LearnSkillsResponse {
            suggested_name: None,
            suggested_patterns: vec![],
            suggested_instructions: None,
            confidence: 0.0,
            evidence_count: 0,
            skill_created: false,
            skill: None,
        }));
    };

    // Only persist when confidence is high enough (≥0.5).
    let (skill_created, skill_resp) = if suggestion.confidence >= 0.5 {
        match learner.generate_skill_from_suggestion(&suggestion) {
            Ok(new_skill) => {
                let req = SkillCreateRequest {
                    name: new_skill.name.clone(),
                    description: new_skill.description.clone(),
                    category: new_skill.category.clone(),
                    trigger_patterns: new_skill.trigger_patterns.clone(),
                    instructions: new_skill.instructions.clone(),
                    enabled: Some(new_skill.enabled),
                    priority: Some(new_skill.priority),
                };
                match state.skills.add_skill(&req) {
                    Ok(saved) => (true, Some(skill_to_response(saved))),
                    Err(_) => (false, None),
                }
            }
            Err(_) => (false, None),
        }
    } else {
        (false, None)
    };

    Ok(Json(LearnSkillsResponse {
        suggested_name: Some(suggestion.suggested_name),
        suggested_patterns: suggestion.suggested_patterns,
        suggested_instructions: Some(suggestion.suggested_instructions),
        confidence: suggestion.confidence,
        evidence_count: suggestion.evidence_count,
        skill_created,
        skill: skill_resp,
    }))
}
