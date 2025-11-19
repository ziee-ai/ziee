// Message handlers - Operations for chat messages

use crate::core::Repos;
use aide::transform::TransformOperation;
use axum::{Json, debug_handler, extract::Path, http::StatusCode};
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    modules::{
        chat::core::{
            permissions::*,
            
            types::{EditMessageRequest, EditMessageResponse, MessageWithContent},
        },
        permissions::{extractors::RequirePermissions, with_permission},
    },
};

// =====================================================
// Message Handlers
// =====================================================

/// Get conversation history (all messages with content in active branch)
#[debug_handler]
pub async fn get_conversation_history(
    auth: RequirePermissions<(MessagesRead,)>,

    Path(conversation_id): Path<Uuid>,
) -> ApiResult<Json<Vec<MessageWithContent>>> {
    // Verify conversation exists and user owns it
    let conversation = Repos.chat.core.get_conversation( conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    // Get active branch
    let branch_id = conversation
        .active_branch_id
        .ok_or_else(|| AppError::internal_error("Conversation has no active branch"))?;

    // Get conversation history
    let history = Repos.chat.core.get_conversation_history( branch_id).await?;

    Ok((StatusCode::OK, Json(history)))
}

pub fn get_conversation_history_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MessagesRead,)>(op)
        .id("Message.getHistory")
        .tag("Chat")
        .summary("Get conversation history")
        .description("Get all messages with content for the active branch of a conversation")
        .response::<200, Json<Vec<MessageWithContent>>>()
        .response_with::<404, (), _>(|res| res.description("Conversation not found"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Get a specific message with its content
#[debug_handler]
pub async fn get_message(
    _auth: RequirePermissions<(MessagesRead,)>,

    Path(message_id): Path<Uuid>,
) -> ApiResult<Json<MessageWithContent>> {
    let message_with_content = Repos.chat.core.get_message_with_content( message_id)
        .await?
        .ok_or_else(|| AppError::not_found("Message"))?;

    // TODO: Verify user owns the conversation containing this message
    // For now, we'll allow any authenticated user with MessagesRead permission

    Ok((StatusCode::OK, Json(message_with_content)))
}

pub fn get_message_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MessagesRead,)>(op)
        .id("Message.get")
        .tag("Chat")
        .summary("Get message with content")
        .description("Get a specific message with all its content blocks")
        .response::<200, Json<MessageWithContent>>()
        .response_with::<404, (), _>(|res| res.description("Message not found"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Edit a message (creates new branch with updated message)
#[debug_handler]
pub async fn edit_message(
    auth: RequirePermissions<(MessagesCreate,)>,

    Path((conversation_id, message_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<EditMessageRequest>,
) -> ApiResult<Json<EditMessageResponse>> {
    // Validate content is not empty
    if request.content.trim().is_empty() {
        return Err(AppError::bad_request("VALIDATION_ERROR", "Message content cannot be empty").into());
    }

    // Verify conversation exists and user owns it
    let conversation = Repos.chat.core.get_conversation( conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    // Get active branch
    let current_branch_id = conversation
        .active_branch_id
        .ok_or_else(|| AppError::internal_error("Conversation has no active branch"))?;

    // Edit message (creates new branch with edited message)
    let response = Repos.chat.core
        .edit_message(message_id, conversation_id, request, current_branch_id)
        .await?;

    Ok((StatusCode::OK, Json(response)))
}

pub fn edit_message_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MessagesCreate,)>(op)
        .id("Message.edit")
        .tag("Chat")
        .summary("Edit message")
        .description("Edit a message. Creates a new branch with the updated message and clones all messages before it.")
        .response::<200, Json<EditMessageResponse>>()
        .response_with::<404, (), _>(|res| res.description("Message or conversation not found"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Delete a message and all its descendants
#[debug_handler]
pub async fn delete_message(
    _auth: RequirePermissions<(MessagesDelete,)>,

    Path(message_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    // TODO: Verify user owns the conversation containing this message

    let deleted_count = Repos.chat.core.delete_message_and_descendants( message_id).await?;

    if deleted_count == 0 {
        return Err(AppError::not_found("Message").into());
    }

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn delete_message_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MessagesDelete,)>(op)
        .id("Message.delete")
        .tag("Chat")
        .summary("Delete message")
        .description(
            "Delete a message and all its descendants. This cascades to all content blocks.",
        )
        .response_with::<204, (), _>(|res| res.description("Message deleted successfully"))
        .response_with::<404, (), _>(|res| res.description("Message not found"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}
