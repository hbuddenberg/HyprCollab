use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::state::AppState;

// ── request / response types ──────────────────────────────────────────────────

/// Request body for image generation.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImageGenerateRequest {
    pub prompt: String,
    pub provider: Option<String>,
    /// Size: "square" | "landscape" | "portrait"
    pub size: Option<String>,
    pub n: Option<u32>,
}

/// A single generated image.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct GeneratedImageItem {
    pub url: Option<String>,
    pub width: u32,
    pub height: u32,
    pub revised_prompt: Option<String>,
    pub seed: Option<u64>,
}

/// Response from image generation.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImageGenerateResponse {
    pub images: Vec<GeneratedImageItem>,
    pub model: String,
    pub provider: String,
}

/// A single image provider entry.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImageProviderEntry {
    pub name: String,
}

/// Response listing all configured image providers.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImageProvidersResponse {
    pub providers: Vec<ImageProviderEntry>,
}

// ── handlers ──────────────────────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/api/image/generate",
    request_body = ImageGenerateRequest,
    responses(
        (status = 200, description = "Generated images", body = ImageGenerateResponse),
        (status = 503, description = "Image router not configured"),
    ),
    tag = "Image"
)]
pub async fn image_generate(
    State(state): State<AppState>,
    Json(payload): Json<ImageGenerateRequest>,
) -> Result<Json<ImageGenerateResponse>, AppError> {
    let router = state
        .image_router
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("Image router not configured".into()))?;

    let size = match payload.size.as_deref().unwrap_or("square") {
        "landscape" => hyprcollab_image::ImageSize::Landscape1792x1024,
        "portrait" => hyprcollab_image::ImageSize::Portrait1024x1792,
        _ => hyprcollab_image::ImageSize::Square1024,
    };

    let mut req = hyprcollab_image::ImageRequest::new(payload.prompt);
    req.size = size;
    req.n = payload.n.unwrap_or(1);

    let resp = if let Some(provider) = payload.provider.as_deref() {
        router
            .generate_with(provider, req)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
    } else {
        router.generate(req).await.map_err(|e| AppError::Internal(e.to_string()))?
    };

    let images = resp
        .images
        .into_iter()
        .map(|img| GeneratedImageItem {
            url: img.url,
            width: img.width,
            height: img.height,
            revised_prompt: img.revised_prompt,
            seed: img.seed,
        })
        .collect();

    Ok(Json(ImageGenerateResponse { images, model: resp.model, provider: resp.provider }))
}

#[utoipa::path(
    get,
    path = "/api/image/providers",
    responses(
        (status = 200, description = "List of configured image providers", body = ImageProvidersResponse),
        (status = 503, description = "Image router not configured"),
    ),
    tag = "Image"
)]
pub async fn image_providers(
    State(state): State<AppState>,
) -> Result<Json<ImageProvidersResponse>, AppError> {
    let router = state
        .image_router
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("Image router not configured".into()))?;

    let providers = router
        .provider_names()
        .into_iter()
        .map(|name| ImageProviderEntry { name: name.to_string() })
        .collect();

    Ok(Json(ImageProvidersResponse { providers }))
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
    async fn generate_returns_503_when_not_configured() {
        let state = test_state().await;
        let app = create_app(state);
        let resp = app
            .oneshot(json_request(
                "POST",
                "/api/image/generate",
                json!({"prompt": "a sunset"}),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn providers_returns_503_when_not_configured() {
        let state = test_state().await;
        let app = create_app(state);
        let req = Request::builder().uri("/api/image/providers").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
