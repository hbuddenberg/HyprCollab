//! REST handlers for the `/api/artifacts` family of endpoints.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use hyprcollab_artifacts::{Artifact, ArtifactId, ArtifactMeta};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::state::AppState;

// ── Request / Response types ─────────────────────────────────────────

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateArtifactRequest {
    pub chat_id: String,
    pub artifact: Artifact,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CreateArtifactResponse {
    pub id: String,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ListArtifactsQuery {
    pub chat_id: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ListArtifactsResponse {
    pub artifacts: Vec<ArtifactMeta>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateArtifactRequest {
    pub artifact: Artifact,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DeleteArtifactResponse {
    pub deleted: bool,
}

// ── Handlers ─────────────────────────────────────────────────────────

/// `POST /api/artifacts` — create a new artifact.
#[utoipa::path(
    post,
    path = "/api/artifacts",
    request_body = CreateArtifactRequest,
    responses(
        (status = 201, description = "Artifact created", body = CreateArtifactResponse),
        (status = 400, description = "Missing or empty chat_id"),
        (status = 500, description = "Storage error"),
    ),
    tag = "Artifacts"
)]
pub async fn create_artifact(
    State(state): State<AppState>,
    Json(payload): Json<CreateArtifactRequest>,
) -> Result<(StatusCode, Json<CreateArtifactResponse>), AppError> {
    if payload.chat_id.trim().is_empty() {
        return Err(AppError::BadRequest("chat_id cannot be empty".into()));
    }

    let id = state
        .artifacts
        .create(&payload.chat_id, payload.artifact)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(CreateArtifactResponse { id: id.0 }),
    ))
}

/// `GET /api/artifacts?chat_id=<id>` — list artifacts for a chat.
#[utoipa::path(
    get,
    path = "/api/artifacts",
    params(ListArtifactsQuery),
    responses(
        (status = 200, description = "Artifact list", body = ListArtifactsResponse),
        (status = 400, description = "Missing chat_id"),
        (status = 500, description = "Storage error"),
    ),
    tag = "Artifacts"
)]
pub async fn list_artifacts(
    State(state): State<AppState>,
    Query(params): Query<ListArtifactsQuery>,
) -> Result<Json<ListArtifactsResponse>, AppError> {
    if params.chat_id.trim().is_empty() {
        return Err(AppError::BadRequest("chat_id query param is required".into()));
    }

    let artifacts = state
        .artifacts
        .list_for_chat(&params.chat_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(ListArtifactsResponse { artifacts }))
}

/// `GET /api/artifacts/:id` — get a single artifact's full content.
#[utoipa::path(
    get,
    path = "/api/artifacts/{id}",
    params(
        ("id" = String, Path, description = "Artifact ID"),
    ),
    responses(
        (status = 200, description = "Artifact content", body = hyprcollab_artifacts::Artifact),
        (status = 404, description = "Not found"),
        (status = 500, description = "Storage error"),
    ),
    tag = "Artifacts"
)]
pub async fn get_artifact(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Artifact>, AppError> {
    let artifact_id = ArtifactId(id.clone());
    let artifact = state
        .artifacts
        .get(&artifact_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Artifact '{id}' not found")))?;

    Ok(Json(artifact))
}

/// `PUT /api/artifacts/:id` — replace an artifact's content.
#[utoipa::path(
    put,
    path = "/api/artifacts/{id}",
    params(
        ("id" = String, Path, description = "Artifact ID"),
    ),
    request_body = UpdateArtifactRequest,
    responses(
        (status = 204, description = "Updated successfully"),
        (status = 404, description = "Not found"),
        (status = 500, description = "Storage error"),
    ),
    tag = "Artifacts"
)]
pub async fn update_artifact(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateArtifactRequest>,
) -> Result<StatusCode, AppError> {
    let artifact_id = ArtifactId(id.clone());
    state
        .artifacts
        .update(&artifact_id, payload.artifact)
        .await
        .map_err(|e| match e {
            hyprcollab_artifacts::ArtifactError::NotFound(_) => {
                AppError::NotFound(format!("Artifact '{id}' not found"))
            }
            other => AppError::Internal(other.to_string()),
        })?;

    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/artifacts/:id` — delete an artifact.
#[utoipa::path(
    delete,
    path = "/api/artifacts/{id}",
    params(
        ("id" = String, Path, description = "Artifact ID"),
    ),
    responses(
        (status = 200, description = "Deletion result", body = DeleteArtifactResponse),
        (status = 500, description = "Storage error"),
    ),
    tag = "Artifacts"
)]
pub async fn delete_artifact(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteArtifactResponse>, AppError> {
    let artifact_id = ArtifactId(id);
    let deleted = state
        .artifacts
        .delete(&artifact_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(DeleteArtifactResponse { deleted }))
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{self, Request, StatusCode},
    };
    use serde_json::json;
    use tower::ServiceExt;

    use super::*;
    use crate::app::create_app;

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

    fn empty_request(method: &str, uri: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn post_artifact_returns_201() {
        let state = test_state().await;
        let app = create_app(state);

        let body = json!({
            "chat_id": "chat-test",
            "artifact": { "type": "markdown", "content": "# Hello" }
        });
        let resp = app
            .oneshot(json_request("POST", "/api/artifacts", body))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json["id"].as_str().is_some());
    }

    #[tokio::test]
    async fn post_artifact_empty_chat_id_returns_400() {
        let state = test_state().await;
        let app = create_app(state);

        let body = json!({
            "chat_id": "",
            "artifact": { "type": "markdown", "content": "x" }
        });
        let resp = app
            .oneshot(json_request("POST", "/api/artifacts", body))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_artifacts_empty() {
        let state = test_state().await;
        let app = create_app(state);

        let resp = app
            .oneshot(empty_request("GET", "/api/artifacts?chat_id=no-chat"))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["artifacts"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn get_artifact_not_found() {
        let state = test_state().await;
        let app = create_app(state);

        let resp = app
            .oneshot(empty_request("GET", "/api/artifacts/nonexistent-id"))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn create_then_get_artifact() {
        let state = test_state().await;

        // Create directly via store so we have the id.
        let id = state
            .artifacts
            .create(
                "chat-roundtrip",
                Artifact::Markdown { content: "roundtrip".into() },
            )
            .await
            .unwrap();

        let app = create_app(state);
        let resp = app
            .oneshot(empty_request("GET", &format!("/api/artifacts/{}", id.0)))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["type"].as_str(), Some("markdown"));
        assert_eq!(json["content"].as_str(), Some("roundtrip"));
    }

    #[tokio::test]
    async fn update_artifact_returns_204() {
        let state = test_state().await;
        let id = state
            .artifacts
            .create("chat-upd", Artifact::Markdown { content: "old".into() })
            .await
            .unwrap();

        let app = create_app(state);
        let body = json!({
            "artifact": { "type": "markdown", "content": "new" }
        });
        let resp = app
            .oneshot(json_request(
                "PUT",
                &format!("/api/artifacts/{}", id.0),
                body,
            ))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn delete_artifact_returns_true() {
        let state = test_state().await;
        let id = state
            .artifacts
            .create("chat-del", Artifact::Markdown { content: "bye".into() })
            .await
            .unwrap();

        let app = create_app(state.clone());
        let resp = app
            .oneshot(empty_request("DELETE", &format!("/api/artifacts/{}", id.0)))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["deleted"].as_bool(), Some(true));
    }

    #[tokio::test]
    async fn delete_nonexistent_returns_false() {
        let state = test_state().await;
        let app = create_app(state);

        let resp = app
            .oneshot(empty_request("DELETE", "/api/artifacts/ghost-id"))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["deleted"].as_bool(), Some(false));
    }
}
