// Conversation handlers - CRUD operations for chat conversations

use crate::core::Repos;
use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::{Path, Query},
    http::StatusCode,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    modules::{
        chat::core::{
            models::Conversation,
            permissions::*,

            types::{ConversationResponse, CreateConversationRequest, UpdateConversationRequest},
        },
        permissions::{extractors::RequirePermissions, with_permission},
    },
};

// =====================================================
// Query Parameters
// =====================================================

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PaginationQuery {
    /// Page number (1-indexed)
    #[serde(default = "default_page")]
    pub page: i64,

    /// Items per page (max 100)
    #[serde(default = "default_limit", alias = "per_page")]
    pub limit: i64,
}

fn default_page() -> i64 {
    1
}
fn default_limit() -> i64 {
    20
}

// =====================================================
// CRUD Handlers
// =====================================================

/// Create a new conversation with a default branch.
#[debug_handler]
pub async fn create_conversation(
    auth: RequirePermissions<(ConversationsCreate,)>,

    Json(request): Json<CreateConversationRequest>,
) -> ApiResult<Json<Conversation>> {
    // Validate title length if provided
    if let Some(title) = &request.title
        && title.len() > 500 {
            return Err(AppError::bad_request("VALIDATION_ERROR", "Title must not exceed 500 characters").into());
        }

    let conversation =
        Repos.chat.core.create_conversation(auth.user.id, request.model_id, request.title)
            .await?;

    Ok((StatusCode::CREATED, Json(conversation)))
}

pub fn create_conversation_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsCreate,)>(op)
        .id("Conversation.create")
        .tag("Chat")
        .summary("Create a new conversation")
        .description("Create a new chat conversation with a default branch")
        .response::<201, Json<Conversation>>()
        .response_with::<400, (), _>(|res| res.description("Invalid request"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Get conversation by ID
#[debug_handler]
pub async fn get_conversation(
    auth: RequirePermissions<(ConversationsRead,)>,

    Path(id): Path<Uuid>,
) -> ApiResult<Json<Conversation>> {
    let conversation = Repos.chat.core.get_conversation( id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    Ok((StatusCode::OK, Json(conversation)))
}

pub fn get_conversation_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsRead,)>(op)
        .id("Conversation.get")
        .tag("Chat")
        .summary("Get conversation by ID")
        .description("Get a conversation by its ID. Only accessible to the owner.")
        .response::<200, Json<Conversation>>()
        .response_with::<404, (), _>(|res| res.description("Conversation not found"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// List the authenticated user's conversations, ordered by
/// most-recently updated.
#[debug_handler]
pub async fn list_conversations(
    auth: RequirePermissions<(ConversationsRead,)>,

    Query(params): Query<PaginationQuery>,
) -> ApiResult<Json<Vec<ConversationResponse>>> {
    let limit = params.limit.min(100).max(1);
    let page = params.page.max(1);
    let offset = (page - 1) * limit;

    let conversations = Repos
        .chat
        .core
        .list_conversations(auth.user.id, limit, offset)
        .await?;

    Ok((StatusCode::OK, Json(conversations)))
}

pub fn list_conversations_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsRead,)>(op)
        .id("Conversation.list")
        .tag("Chat")
        .summary("List conversations")
        .description("List all conversations for the authenticated user with pagination")
        .response::<200, Json<Vec<ConversationResponse>>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Update conversation metadata (title + memory_mode).
#[debug_handler]
pub async fn update_conversation(
    auth: RequirePermissions<(ConversationsEdit,)>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateConversationRequest>,
) -> ApiResult<Json<Conversation>> {
    // Validate title length if provided
    if let Some(Some(title)) = &request.title
        && title.len() > 500 {
            return Err(AppError::bad_request("VALIDATION_ERROR", "Title must not exceed 500 characters").into());
        }

    // Validate memory_mode if provided.
    if let Some(ref mode) = request.memory_mode {
        if !matches!(mode.as_str(), "inherit" | "on" | "off") {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "memory_mode must be one of: inherit, on, off",
            )
            .into());
        }
    }

    let mut conversation = Repos
        .chat
        .core
        .update_conversation(id, auth.user.id, request.title)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    // memory_mode lives on the conversations table but isn't covered by
    // the generic update_conversation repo method; one-shot SQL keeps the
    // diff small. Re-fetch afterwards so the returned struct is current.
    if let Some(mode) = request.memory_mode {
        let pool = crate::core::Repos.memory.pool_clone();
        sqlx::query!(
            "UPDATE conversations SET memory_mode = $1, updated_at = NOW() WHERE id = $2 AND user_id = $3",
            mode,
            id,
            auth.user.id
        )
        .execute(&pool)
        .await
        .map_err(AppError::database_error)?;
        conversation = Repos
            .chat
            .core
            .get_conversation(id, auth.user.id)
            .await?
            .ok_or_else(|| AppError::not_found("Conversation"))?;
    }

    Ok((StatusCode::OK, Json(conversation)))
}

pub fn update_conversation_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsEdit,)>(op)
        .id("Conversation.update")
        .tag("Chat")
        .summary("Update conversation")
        .description("Update conversation metadata (currently only title)")
        .response::<200, Json<Conversation>>()
        .response_with::<404, (), _>(|res| res.description("Conversation not found"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Delete conversation
#[debug_handler]
pub async fn delete_conversation(
    auth: RequirePermissions<(ConversationsDelete,)>,

    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let deleted = Repos.chat.core.delete_conversation( id, auth.user.id).await?;

    if !deleted {
        return Err(AppError::not_found("Conversation").into());
    }

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn delete_conversation_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsDelete,)>(op)
        .id("Conversation.delete")
        .tag("Chat")
        .summary("Delete conversation")
        .description("Delete a conversation and all its branches, messages, and content")
        .response_with::<204, (), _>(|res| res.description("Conversation deleted successfully"))
        .response_with::<404, (), _>(|res| res.description("Conversation not found"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}
