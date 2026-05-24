use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Application errors for HyprCollab server.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// 500 Internal Server Error
    #[error("Internal server error: {0}")]
    Internal(String),

    /// 400 Bad Request
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// 404 Not Found
    #[error("Resource not found: {0}")]
    NotFound(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
        };

        let body = Json(json!({
            "error": message,
            "status": status.as_u16(),
        }));

        (status, body).into_response()
    }
}
