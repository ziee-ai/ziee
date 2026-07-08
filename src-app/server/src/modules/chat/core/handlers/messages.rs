// Message handlers - Operations for chat messages

use crate::core::Repos;
use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::{Path, Query},
    http::StatusCode,
};
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    modules::{
        chat::core::{
            permissions::*,
            types::{
                EditMessageRequest, EditMessageResponse, MessageHistoryQuery, MessageSearchQuery,
                MessageSearchResults, MessageWithContent, PaginatedMessages,
            },
        },
        permissions::{extractors::RequirePermissions, with_permission},
        sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish},
    },
};

// =====================================================
// Message Handlers
// =====================================================

/// Get one paginated (keyset) page of the active branch's messages with content.
///
/// Cursor is a message_id on the active branch. No cursor → newest page (tail);
/// `before` → older page; `after` → newer page; `around` → a centered window.
/// See [`MessageHistoryQuery`]. Full-history load (for AI context) lives in the
/// repository's untouched `get_conversation_history`.
#[debug_handler]
pub async fn get_conversation_history(
    auth: RequirePermissions<(MessagesRead,)>,
    Path(conversation_id): Path<Uuid>,
    Query(query): Query<MessageHistoryQuery>,
) -> ApiResult<Json<PaginatedMessages>> {
    // Verify conversation exists and user owns it
    let conversation = Repos
        .chat
        .core
        .get_conversation(conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    // Get active branch
    let branch_id = conversation
        .active_branch_id
        .ok_or_else(|| AppError::internal_error("Conversation has no active branch"))?;

    // Resolve the window mode (400 if >1 cursor) + clamp the page size.
    let mode = query.mode()?;
    let limit = query.clamped_limit();

    // A supplied cursor id that isn't in the active branch → 404.
    let page = Repos
        .chat
        .core
        .get_message_window(branch_id, mode, limit)
        .await?
        .ok_or_else(|| AppError::not_found("Message"))?;

    Ok((StatusCode::OK, Json(page)))
}

pub fn get_conversation_history_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MessagesRead,)>(op)
        .id("Message.getHistory")
        .tag("Chat")
        .summary("Get conversation history (paginated)")
        .description(
            "Get one page of messages with content for the active branch. Keyset pagination: \
             pass `before=<message_id>` for older messages, `after=<message_id>` for newer, or \
             `around=<message_id>` for a window centered on a message. No cursor returns the \
             newest page. `limit` defaults to 30 (max 100). At most one of before/after/around \
             may be set.",
        )
        .response::<200, Json<PaginatedMessages>>()
        .response_with::<400, (), _>(|res| {
            res.description("More than one of before/after/around set")
        })
        .response_with::<404, (), _>(|res| {
            res.description("Conversation or cursor message not found")
        })
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Search messages WITHIN one conversation's active branch (server-side, so a
/// match in an unloaded/paginated-out message is still found). Paginated.
#[debug_handler]
pub async fn search_conversation_messages(
    auth: RequirePermissions<(MessagesRead,)>,
    Path(conversation_id): Path<Uuid>,
    Query(query): Query<MessageSearchQuery>,
) -> ApiResult<Json<MessageSearchResults>> {
    let conversation = Repos
        .chat
        .core
        .get_conversation(conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    let page = query.clamped_page();
    let per_page = query.clamped_per_page();

    // Blank query → empty result without a DB scan.
    let Some(term) = query.trimmed_term() else {
        return Ok((
            StatusCode::OK,
            Json(MessageSearchResults {
                matches: Vec::new(),
                total: 0,
                page,
                per_page,
            }),
        ));
    };

    let branch_id = conversation
        .active_branch_id
        .ok_or_else(|| AppError::internal_error("Conversation has no active branch"))?;

    let results = Repos
        .chat
        .core
        .search_messages_in_conversation(branch_id, term, page, per_page)
        .await?;

    Ok((StatusCode::OK, Json(results)))
}

pub fn search_conversation_messages_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MessagesRead,)>(op)
        .id("Message.searchInConversation")
        .tag("Chat")
        .summary("Search messages within a conversation")
        .description(
            "Case-insensitive substring search over the text of the conversation's active-branch \
             messages, paginated. Returns matches with a snippet + a stable 1-based global \
             `ordinal` and the full `total`, so a find UI can display results and jump \
             (via `around=`) to a message that lazy-load has not yet loaded.",
        )
        .response::<200, Json<MessageSearchResults>>()
        .response_with::<404, (), _>(|res| res.description("Conversation not found"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Get a specific message with its content
#[debug_handler]
pub async fn get_message(
    auth: RequirePermissions<(MessagesRead,)>,

    Path(message_id): Path<Uuid>,
) -> ApiResult<Json<MessageWithContent>> {
    // Verify user owns the conversation containing this message
    let _conversation = Repos.chat.core
        .verify_message_ownership( message_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Message"))?;

    let message_with_content = Repos.chat.core.get_message_with_content( message_id)
        .await?
        .ok_or_else(|| AppError::not_found("Message"))?;

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
    origin: SyncOrigin,
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

    sync_publish(
        SyncEntity::Conversation,
        SyncAction::Update,
        conversation_id,
        Audience::owner(auth.user.id),
        origin.0,
    );

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

/// Delete a single message. The branch_messages cascade removes the
/// junction rows in every branch that referenced it. See 04-chat F-03
/// rationale in `messages::delete_message` (descendant semantics are
/// undefined for CoW-branched chats).
#[debug_handler]
pub async fn delete_message(
    auth: RequirePermissions<(MessagesDelete,)>,
    origin: SyncOrigin,
    Path(message_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    // Verify user owns the conversation containing this message
    let conversation = Repos.chat.core
        .verify_message_ownership( message_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Message"))?;

    let deleted_count = Repos.chat.core.delete_message(message_id).await?;

    if deleted_count == 0 {
        return Err(AppError::not_found("Message").into());
    }

    sync_publish(
        SyncEntity::Conversation,
        SyncAction::Update,
        conversation.id,
        Audience::owner(auth.user.id),
        origin.0,
    );

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
