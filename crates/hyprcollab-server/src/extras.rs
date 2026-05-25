//! Extra conversation endpoints — branching, bookmarks, pinning, FTS5 search (F5 S24).

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use hyprcollab_core::types::{ChatId, MessageId};
use hyprcollab_memory::{Bookmark, BookmarkId, BookmarkStore, ConversationSearch, MessageTreeNode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

// ── Request / Response types ──────────────────────────────────────────────────

/// Response item for a single message in tree/search views.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MessageItem {
    pub id: String,
    pub chat_id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
    pub parent_id: Option<String>,
}

/// A node in the message tree.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct TreeNode {
    pub message: MessageItem,
    pub children: Vec<TreeNode>,
}

/// Response for the tree endpoint.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MessageTreeResponse {
    pub roots: Vec<TreeNode>,
}

/// Request body to fork a conversation.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ForkRequest {
    pub message_id: String,
}

/// Response after forking.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ForkResponse {
    pub new_conversation_id: String,
}

/// A bookmark as returned by the API.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct BookmarkResponse {
    pub id: String,
    pub chat_id: String,
    pub message_id: String,
    pub label: Option<String>,
    pub created_at: String,
}

/// List of bookmarks.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ListBookmarksResponse {
    pub bookmarks: Vec<BookmarkResponse>,
}

/// Request body to add a bookmark.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct AddBookmarkRequest {
    pub message_id: String,
    pub label: Option<String>,
}

/// Response after deleting a bookmark.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DeleteBookmarkResponse {
    pub deleted: bool,
}

/// Request body to pin/unpin a conversation.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct PinRequest {
    pub pinned: bool,
}

/// Response after pinning.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PinResponse {
    pub pinned: bool,
}

/// A single FTS5 search result.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ConversationSearchResult {
    pub message: MessageItem,
    pub chat_title: String,
    pub rank: f64,
}

/// Response for full-text search.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct SearchResponse {
    pub results: Vec<ConversationSearchResult>,
    pub total: usize,
}

/// Query params for the search endpoint.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct SearchQuery {
    pub q: String,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_chat_id(s: &str) -> Result<ChatId, AppError> {
    Uuid::parse_str(s)
        .map(ChatId::from)
        .map_err(|_| AppError::BadRequest(format!("invalid conversation id: {s}")))
}

fn parse_message_id(s: &str) -> Result<MessageId, AppError> {
    Uuid::parse_str(s)
        .map(MessageId::from)
        .map_err(|_| AppError::BadRequest(format!("invalid message id: {s}")))
}

fn parse_bookmark_id(s: &str) -> Result<BookmarkId, AppError> {
    Uuid::parse_str(s)
        .map(BookmarkId)
        .map_err(|_| AppError::BadRequest(format!("invalid bookmark id: {s}")))
}

fn msg_item(m: &hyprcollab_core::types::Message) -> MessageItem {
    MessageItem {
        id: m.id.to_string(),
        chat_id: m.chat_id.to_string(),
        role: m.role.to_string(),
        content: m.content.clone(),
        timestamp: m.timestamp.to_rfc3339(),
        parent_id: m.parent_id.map(|p| p.to_string()),
    }
}

fn tree_node(node: &MessageTreeNode) -> TreeNode {
    TreeNode {
        message: msg_item(&node.message),
        children: node.children.iter().map(tree_node).collect(),
    }
}

fn bookmark_response(b: Bookmark) -> BookmarkResponse {
    BookmarkResponse {
        id: b.id.to_string(),
        chat_id: b.chat_id.to_string(),
        message_id: b.message_id.to_string(),
        label: b.label,
        created_at: b.created_at,
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// `POST /api/conversations/{id}/fork` — fork a conversation at a specific message.
#[utoipa::path(
    post,
    path = "/api/conversations/{id}/fork",
    params(("id" = String, Path, description = "Conversation UUID")),
    request_body = ForkRequest,
    responses(
        (status = 201, description = "Fork created", body = ForkResponse),
        (status = 400, description = "Invalid IDs or message not in chat"),
        (status = 404, description = "Conversation not found"),
        (status = 500, description = "Storage error"),
    ),
    tag = "Conversations"
)]
pub async fn fork_conversation(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ForkRequest>,
) -> Result<(StatusCode, Json<ForkResponse>), AppError> {
    let chat_id = parse_chat_id(&id)?;
    let msg_id = parse_message_id(&body.message_id)?;

    let new_id = state
        .memory
        .fork_from_message(chat_id, msg_id)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(ForkResponse { new_conversation_id: new_id.to_string() })))
}

/// `GET /api/conversations/{id}/tree` — get the message tree for a conversation.
#[utoipa::path(
    get,
    path = "/api/conversations/{id}/tree",
    params(("id" = String, Path, description = "Conversation UUID")),
    responses(
        (status = 200, description = "Message tree", body = MessageTreeResponse),
        (status = 400, description = "Invalid UUID"),
        (status = 500, description = "Storage error"),
    ),
    tag = "Conversations"
)]
pub async fn get_message_tree(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<MessageTreeResponse>, AppError> {
    let chat_id = parse_chat_id(&id)?;
    let nodes = state
        .memory
        .get_message_tree(chat_id)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(MessageTreeResponse { roots: nodes.iter().map(tree_node).collect() }))
}

/// `GET /api/conversations/{id}/bookmarks` — list bookmarks for a conversation.
#[utoipa::path(
    get,
    path = "/api/conversations/{id}/bookmarks",
    params(("id" = String, Path, description = "Conversation UUID")),
    responses(
        (status = 200, description = "Bookmarks", body = ListBookmarksResponse),
        (status = 400, description = "Invalid UUID"),
        (status = 500, description = "Storage error"),
    ),
    tag = "Conversations"
)]
pub async fn list_bookmarks(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ListBookmarksResponse>, AppError> {
    let chat_id = parse_chat_id(&id)?;
    let bm = BookmarkStore::new(&state.memory);
    let bookmarks = bm
        .list_bookmarks(chat_id)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(ListBookmarksResponse {
        bookmarks: bookmarks.into_iter().map(bookmark_response).collect(),
    }))
}

/// `POST /api/conversations/{id}/bookmarks` — add a bookmark to a conversation.
#[utoipa::path(
    post,
    path = "/api/conversations/{id}/bookmarks",
    params(("id" = String, Path, description = "Conversation UUID")),
    request_body = AddBookmarkRequest,
    responses(
        (status = 201, description = "Bookmark created", body = BookmarkResponse),
        (status = 400, description = "Invalid IDs"),
        (status = 500, description = "Storage error"),
    ),
    tag = "Conversations"
)]
pub async fn add_bookmark(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<AddBookmarkRequest>,
) -> Result<(StatusCode, Json<BookmarkResponse>), AppError> {
    let chat_id = parse_chat_id(&id)?;
    let msg_id = parse_message_id(&body.message_id)?;

    let bm = BookmarkStore::new(&state.memory);
    let bookmark = bm
        .add_bookmark(chat_id, msg_id, body.label)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(bookmark_response(bookmark))))
}

/// `DELETE /api/bookmarks/{id}` — remove a bookmark.
#[utoipa::path(
    delete,
    path = "/api/bookmarks/{id}",
    params(("id" = String, Path, description = "Bookmark UUID")),
    responses(
        (status = 200, description = "Deletion result", body = DeleteBookmarkResponse),
        (status = 400, description = "Invalid UUID"),
        (status = 500, description = "Storage error"),
    ),
    tag = "Conversations"
)]
pub async fn delete_bookmark(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteBookmarkResponse>, AppError> {
    let bm_id = parse_bookmark_id(&id)?;
    let bm = BookmarkStore::new(&state.memory);
    let deleted = bm
        .remove_bookmark(bm_id)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(DeleteBookmarkResponse { deleted }))
}

/// `PUT /api/conversations/{id}/pin` — pin or unpin a conversation.
#[utoipa::path(
    put,
    path = "/api/conversations/{id}/pin",
    params(("id" = String, Path, description = "Conversation UUID")),
    request_body = PinRequest,
    responses(
        (status = 200, description = "Pin state updated", body = PinResponse),
        (status = 400, description = "Invalid UUID"),
        (status = 404, description = "Conversation not found"),
        (status = 500, description = "Storage error"),
    ),
    tag = "Conversations"
)]
pub async fn pin_conversation(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<PinRequest>,
) -> Result<Json<PinResponse>, AppError> {
    let chat_id = parse_chat_id(&id)?;
    let found = state
        .memory
        .pin_chat(chat_id, body.pinned)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    if !found {
        return Err(AppError::NotFound(format!("conversation '{id}' not found")));
    }

    Ok(Json(PinResponse { pinned: body.pinned }))
}

/// `GET /api/conversations/search` — full-text search across all conversations.
#[utoipa::path(
    get,
    path = "/api/conversations/search",
    params(SearchQuery),
    responses(
        (status = 200, description = "Search results", body = SearchResponse),
        (status = 400, description = "Empty query"),
        (status = 500, description = "Storage error"),
    ),
    tag = "Conversations"
)]
pub async fn search_conversations(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, AppError> {
    if params.q.trim().is_empty() {
        return Err(AppError::BadRequest("search query cannot be empty".into()));
    }

    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);

    let searcher = ConversationSearch::new(&state.memory);
    let results = searcher
        .search_conversations(&params.q, limit, offset)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let total = results.len();
    let items = results
        .into_iter()
        .map(|r| ConversationSearchResult {
            message: msg_item(&r.message),
            chat_title: r.chat_title,
            rank: r.rank,
        })
        .collect();

    Ok(Json(SearchResponse { results: items, total }))
}
