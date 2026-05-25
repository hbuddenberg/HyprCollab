//! REST handlers for `/api/themes` — theme engine (F5 S22).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use hyprcollab_themes::{Theme, ThemeColors, ThemeEngine};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::state::AppState;

// ── Request / Response types ──────────────────────────────────────────────────

/// Colour palette returned by the API.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ThemeColorsResponse {
    pub bg: String,
    pub fg: String,
    pub accent: String,
    pub surface: String,
    pub border: String,
    pub muted: Option<String>,
    pub error: Option<String>,
    pub success: Option<String>,
    pub warning: Option<String>,
}

/// Full theme descriptor returned by the API.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ThemeResponse {
    pub name: String,
    pub colors: ThemeColorsResponse,
    pub font: String,
    pub font_size: u32,
    pub border_radius: u32,
    pub css: String,
    pub custom_css: Option<String>,
}

/// Response listing available theme names.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ListThemesResponse {
    pub themes: Vec<String>,
}

/// Request body to upload a custom theme.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateThemeRequest {
    pub name: String,
    pub bg: String,
    pub fg: String,
    pub accent: String,
    pub surface: String,
    pub border: String,
    pub muted: Option<String>,
    pub error: Option<String>,
    pub success: Option<String>,
    pub warning: Option<String>,
    pub font: Option<String>,
    pub font_size: Option<u32>,
    pub border_radius: Option<u32>,
    pub custom_css: Option<String>,
}

/// Response after deleting a custom theme.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DeleteThemeResponse {
    pub deleted: bool,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn theme_to_response(theme: Theme) -> ThemeResponse {
    let css = ThemeEngine::export_css(&theme);
    ThemeResponse {
        name: theme.name,
        colors: ThemeColorsResponse {
            bg: theme.colors.bg,
            fg: theme.colors.fg,
            accent: theme.colors.accent,
            surface: theme.colors.surface,
            border: theme.colors.border,
            muted: theme.colors.muted,
            error: theme.colors.error,
            success: theme.colors.success,
            warning: theme.colors.warning,
        },
        font: theme.font,
        font_size: theme.font_size,
        border_radius: theme.border_radius,
        css,
        custom_css: theme.custom_css,
    }
}

fn map_theme_err(e: hyprcollab_themes::ThemeError) -> AppError {
    match e {
        hyprcollab_themes::ThemeError::NotFound(n) => AppError::NotFound(format!("theme '{n}' not found")),
        hyprcollab_themes::ThemeError::BuiltIn(n) => {
            AppError::BadRequest(format!("'{n}' is a built-in theme and cannot be deleted"))
        }
        hyprcollab_themes::ThemeError::Validation(msg) => AppError::BadRequest(msg),
        e => AppError::Internal(e.to_string()),
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// `GET /api/themes` — list all available themes.
#[utoipa::path(
    get,
    path = "/api/themes",
    responses(
        (status = 200, description = "Available theme names", body = ListThemesResponse),
    ),
    tag = "Themes"
)]
pub async fn list_themes(State(state): State<AppState>) -> Json<ListThemesResponse> {
    Json(ListThemesResponse { themes: state.themes.list_themes() })
}

/// `GET /api/themes/:name` — get theme details and CSS.
#[utoipa::path(
    get,
    path = "/api/themes/{name}",
    params(("name" = String, Path, description = "Theme name")),
    responses(
        (status = 200, description = "Theme details with CSS", body = ThemeResponse),
        (status = 404, description = "Theme not found"),
    ),
    tag = "Themes"
)]
pub async fn get_theme(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ThemeResponse>, AppError> {
    let theme = state.themes.load_theme(&name).map_err(map_theme_err)?;
    Ok(Json(theme_to_response(theme)))
}

/// `GET /api/themes/:name/css` — raw CSS `:root` block for the theme.
#[utoipa::path(
    get,
    path = "/api/themes/{name}/css",
    params(("name" = String, Path, description = "Theme name")),
    responses(
        (status = 200, description = "CSS variables block (text/css)"),
        (status = 404, description = "Theme not found"),
    ),
    tag = "Themes"
)]
pub async fn get_theme_css(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, AppError> {
    let theme = state.themes.load_theme(&name).map_err(map_theme_err)?;
    let css = ThemeEngine::export_css(&theme);
    Ok((
        [(axum::http::header::CONTENT_TYPE, "text/css")],
        css,
    )
        .into_response())
}

/// `POST /api/themes` — upload a custom theme (JSON body).
#[utoipa::path(
    post,
    path = "/api/themes",
    request_body = CreateThemeRequest,
    responses(
        (status = 201, description = "Custom theme created", body = ThemeResponse),
        (status = 400, description = "Validation error"),
    ),
    tag = "Themes"
)]
pub async fn create_theme(
    State(state): State<AppState>,
    Json(body): Json<CreateThemeRequest>,
) -> Result<(StatusCode, Json<ThemeResponse>), AppError> {
    let theme = Theme {
        name: body.name,
        colors: ThemeColors {
            bg: body.bg,
            fg: body.fg,
            accent: body.accent,
            surface: body.surface,
            border: body.border,
            muted: body.muted,
            error: body.error,
            success: body.success,
            warning: body.warning,
        },
        font: body.font.unwrap_or_else(|| "monospace".into()),
        font_size: body.font_size.unwrap_or(14),
        border_radius: body.border_radius.unwrap_or(4),
        custom_css: body.custom_css,
    };

    state.themes.save_custom(&theme).map_err(map_theme_err)?;
    Ok((StatusCode::CREATED, Json(theme_to_response(theme))))
}

/// `DELETE /api/themes/:name` — delete a custom theme (built-ins are protected).
#[utoipa::path(
    delete,
    path = "/api/themes/{name}",
    params(("name" = String, Path, description = "Theme name")),
    responses(
        (status = 200, description = "Theme deleted", body = DeleteThemeResponse),
        (status = 400, description = "Cannot delete a built-in theme"),
        (status = 404, description = "Theme not found"),
    ),
    tag = "Themes"
)]
pub async fn delete_theme(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<DeleteThemeResponse>, AppError> {
    state.themes.delete_custom(&name).map_err(map_theme_err)?;
    Ok(Json(DeleteThemeResponse { deleted: true }))
}
