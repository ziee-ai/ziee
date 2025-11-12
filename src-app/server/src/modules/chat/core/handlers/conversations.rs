// Conversation handlers - CRUD operations for chat conversations

use aide::transform::TransformOperation;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    modules::{
        chat::core::{
            models::{Conversation, ConversationResponse, CreateConversationRequest, UpdateConversationRequest},
            permissions::*,
            repository::conversations as repo,
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
    #[serde(default = "default_limit")]
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

/// Create a new conversation
pub async fn create_conversation(
    auth: RequirePermissions<(ConversationsCreate,)>,
    State(pool): State<PgPool>,
    Json(request): Json<CreateConversationRequest>,
) -> ApiResult<Json<Conversation>> {
    let conversation = repo::create_conversation(
        &pool,
        auth.user.id,
        request.model_id,
        request.title,
    )
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
pub async fn get_conversation(
    auth: RequirePermissions<(ConversationsRead,)>,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Conversation>> {
    let conversation = repo::get_conversation(&pool, id, auth.user.id)
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

/// List conversations for the authenticated user
pub async fn list_conversations(
    auth: RequirePermissions<(ConversationsRead,)>,
    State(pool): State<PgPool>,
    Query(params): Query<PaginationQuery>,
) -> ApiResult<Json<Vec<ConversationResponse>>> {
    // Validate pagination
    let limit = params.limit.min(100).max(1);
    let page = params.page.max(1);
    let offset = (page - 1) * limit;

    let conversations = repo::list_conversations(&pool, auth.user.id, limit, offset).await?;

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

/// Update conversation metadata
pub async fn update_conversation(
    auth: RequirePermissions<(ConversationsEdit,)>,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateConversationRequest>,
) -> ApiResult<Json<Conversation>> {
    let conversation = repo::update_conversation(&pool, id, auth.user.id, request.title)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

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
pub async fn delete_conversation(
    auth: RequirePermissions<(ConversationsDelete,)>,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let deleted = repo::delete_conversation(&pool, id, auth.user.id).await?;

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
