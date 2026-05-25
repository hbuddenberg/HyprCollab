use axum::{
    extract::{Multipart, Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::state::AppState;

// ── request / response types ──────────────────────────────────────────────────

/// Response after ingesting a document into the RAG store.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RagIngestResponse {
    pub chunks_created: usize,
    pub document_title: Option<String>,
}

/// Request body for a RAG semantic query.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RagQueryRequest {
    pub query: String,
    pub limit: Option<usize>,
}

/// A single result from a RAG semantic search.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RagSearchResult {
    pub content: String,
    pub source_file: Option<String>,
    pub section_heading: Option<String>,
    pub score: f32,
}

/// Response from a RAG semantic query.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RagQueryResponse {
    pub results: Vec<RagSearchResult>,
}

/// A document entry returned by the documents listing.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RagDocument {
    pub id: String,
    pub source_file: String,
}

/// Response listing all indexed documents.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RagDocumentsResponse {
    pub documents: Vec<RagDocument>,
}

/// Response after deleting a document from the RAG store.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RagDeleteResponse {
    pub deleted: bool,
    pub source_file: String,
}

// ── handlers ──────────────────────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/api/rag/upload",
    responses(
        (status = 200, description = "Document ingested", body = RagIngestResponse),
        (status = 400, description = "No file field or bad multipart"),
        (status = 503, description = "RAG pipeline not configured"),
    ),
    tag = "RAG"
)]
pub async fn rag_upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<RagIngestResponse>, AppError> {
    let pipeline = state
        .rag
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("RAG pipeline not configured".into()))?;

    let field = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?
        .ok_or_else(|| AppError::BadRequest("No file field found in multipart upload".into()))?;

    let filename = field
        .file_name()
        .map(String::from)
        .unwrap_or_else(|| "upload".to_string());

    let data = field.bytes().await.map_err(|e| AppError::BadRequest(e.to_string()))?;

    let result = pipeline
        .ingest(&data, &filename)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(RagIngestResponse {
        chunks_created: result.chunks_created,
        document_title: result.document_title,
    }))
}

#[utoipa::path(
    post,
    path = "/api/rag/query",
    request_body = RagQueryRequest,
    responses(
        (status = 200, description = "Semantic search results", body = RagQueryResponse),
        (status = 503, description = "RAG pipeline not configured"),
    ),
    tag = "RAG"
)]
pub async fn rag_query(
    State(state): State<AppState>,
    Json(payload): Json<RagQueryRequest>,
) -> Result<Json<RagQueryResponse>, AppError> {
    let pipeline = state
        .rag
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("RAG pipeline not configured".into()))?;

    let limit = payload.limit.unwrap_or(10);
    let raw = pipeline
        .query(&payload.query, limit)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let results = raw
        .into_iter()
        .map(|r| RagSearchResult {
            content: r.chunk.content,
            source_file: r.chunk.metadata.source_file,
            section_heading: r.chunk.metadata.section_heading,
            score: r.score,
        })
        .collect();

    Ok(Json(RagQueryResponse { results }))
}

#[utoipa::path(
    get,
    path = "/api/rag/documents",
    responses(
        (status = 200, description = "List of indexed documents", body = RagDocumentsResponse),
        (status = 503, description = "RAG pipeline not configured"),
    ),
    tag = "RAG"
)]
pub async fn rag_list_documents(
    State(state): State<AppState>,
) -> Result<Json<RagDocumentsResponse>, AppError> {
    let pipeline = state
        .rag
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("RAG pipeline not configured".into()))?;

    let sources = pipeline
        .list_sources()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let documents = sources
        .into_iter()
        .map(|s| RagDocument { id: s.clone(), source_file: s })
        .collect();

    Ok(Json(RagDocumentsResponse { documents }))
}

#[utoipa::path(
    delete,
    path = "/api/rag/documents/{id}",
    params(("id" = String, Path, description = "Source file identifier")),
    responses(
        (status = 200, description = "Document deleted", body = RagDeleteResponse),
        (status = 503, description = "RAG pipeline not configured"),
    ),
    tag = "RAG"
)]
pub async fn rag_delete_document(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<RagDeleteResponse>, AppError> {
    let pipeline = state
        .rag
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("RAG pipeline not configured".into()))?;

    pipeline
        .delete_source(&id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(RagDeleteResponse { deleted: true, source_file: id }))
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::json;
    use tower::ServiceExt;

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

    fn json_request(method: &str, uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap()
    }

    #[tokio::test]
    async fn rag_upload_returns_503_when_not_configured() {
        let state = test_state().await;
        let app = create_app(state);
        let boundary = "boundary123";
        let body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"test.txt\"\r\n\r\nhello\r\n--{boundary}--\r\n"
        );
        let req = Request::builder()
            .method("POST")
            .uri("/api/rag/upload")
            .header("content-type", format!("multipart/form-data; boundary={boundary}"))
            .body(Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn rag_query_returns_503_when_not_configured() {
        let state = test_state().await;
        let app = create_app(state);
        let resp = app
            .oneshot(json_request("POST", "/api/rag/query", json!({"query": "hello"})))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn rag_list_documents_returns_503_when_not_configured() {
        let state = test_state().await;
        let app = create_app(state);
        let req = Request::builder().uri("/api/rag/documents").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn rag_delete_document_returns_503_when_not_configured() {
        let state = test_state().await;
        let app = create_app(state);
        let req = Request::builder()
            .method("DELETE")
            .uri("/api/rag/documents/my-doc.txt")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
