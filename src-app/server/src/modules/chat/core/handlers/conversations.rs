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

            types::{
                streaming::{SSEChatStreamEvent, SSEChatStreamHistoryReplacedData},
                ConversationListResponse, CreateConversationRequest, UpdateConversationRequest,
            },
        },
        chat::stream::publish_raw_event,
        permissions::{extractors::RequirePermissions, with_permission},
        sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish},
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

    /// Optional case-insensitive search term. Matches a conversation's title
    /// OR the text of any of its messages (substring). Omit for no filter.
    #[serde(default)]
    pub search: Option<String>,

    /// Optional sort order: `recent` (default, most-recently updated first),
    /// `oldest`, `alpha` (by title A→Z), or `most_messages`. Unknown values
    /// fall back to `recent`.
    #[serde(default)]
    pub sort: Option<String>,
}

fn default_page() -> i64 {
    1
}

/// Escape LIKE/ILIKE metacharacters so a search term matches literally.
/// Backslash first (it's the default ESCAPE char), then the wildcards.
fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
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
    origin: SyncOrigin,
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

    sync_publish(
        SyncEntity::Conversation,
        SyncAction::Create,
        conversation.id,
        Audience::owner(auth.user.id),
        origin.0,
    );

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
) -> ApiResult<Json<ConversationListResponse>> {
    let limit = params.limit.min(100).max(1);
    let page = params.page.max(1);
    let offset = (page - 1) * limit;

    // Treat a blank/whitespace search as "no filter" so an empty box doesn't
    // ILIKE '%%' every row through the content-search path. Escape LIKE
    // metacharacters (\ % _) so the term is matched as a LITERAL substring
    // (matching the client-side plain-substring find), not as SQL wildcards —
    // ILIKE's default ESCAPE is backslash.
    let search: Option<String> = params
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(escape_like);
    let sort = params.sort.as_deref();

    let conversations = Repos
        .chat
        .core
        .list_conversations(auth.user.id, limit, offset, search.as_deref(), sort)
        .await?;

    let total = Repos
        .chat
        .core
        .count_conversations(auth.user.id, search.as_deref())
        .await?;

    Ok((StatusCode::OK, Json(ConversationListResponse { conversations, total })))
}

pub fn list_conversations_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsRead,)>(op)
        .id("Conversation.list")
        .tag("Chat")
        .summary("List conversations")
        .description("List all conversations for the authenticated user with pagination")
        .response::<200, Json<ConversationListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Update conversation metadata (title).
#[debug_handler]
pub async fn update_conversation(
    auth: RequirePermissions<(ConversationsEdit,)>,
    origin: SyncOrigin,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateConversationRequest>,
) -> ApiResult<Json<Conversation>> {
    // Validate title length if provided
    if let Some(Some(title)) = &request.title
        && title.len() > 500 {
            return Err(AppError::bad_request("VALIDATION_ERROR", "Title must not exceed 500 characters").into());
        }

    let conversation = Repos
        .chat
        .core
        .update_conversation(id, auth.user.id, request.title)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    sync_publish(
        SyncEntity::Conversation,
        SyncAction::Update,
        conversation.id,
        Audience::owner(auth.user.id),
        origin.0,
    );

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
    origin: SyncOrigin,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let deleted = Repos.chat.core.delete_conversation( id, auth.user.id).await?;

    if !deleted {
        return Err(AppError::not_found("Conversation").into());
    }

    // Cascade fs cleanup: drop the conversation's lit-search `/lit` view dir so
    // its hard-linked full-text files don't linger on disk after delete.
    crate::modules::lit_search::fulltext::cache::cleanup_conversation_view(id);

    // Cascade fs cleanup: drop the conversation's sandbox workspace dir now
    // rather than waiting up to 30 days for the workspace reaper.
    crate::modules::code_sandbox::cleanup_conversation_workspace(id);

    sync_publish(
        SyncEntity::Conversation,
        SyncAction::Delete,
        id,
        Audience::owner(auth.user.id),
        origin.0,
    );

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

/// Request body for `POST /conversations/{id}/compact` (ITEM-61 / DEC-137).
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct CompactConversationRequest {
    /// Optional focus hint steering what the compaction should preserve. Advisory
    /// today (the rolling summary is regenerated from the full active-branch
    /// history); reserved for a future focus-aware summarizer.
    #[serde(default)]
    pub focus: Option<String>,
}

/// Manually COMPACT a conversation's context (ITEM-61 / DEC-137): regenerate the
/// active branch's rolling summary now, then emit a `historyReplaced` marker so a
/// viewing client renders a "context compacted" divider in the timeline without a
/// reload. OUTBOUND-ONLY — the stored `message_contents` are never rewritten or
/// deleted; only the `conversation_summaries` row is upserted.
#[debug_handler]
pub async fn compact_conversation(
    auth: RequirePermissions<(ConversationsEdit,)>,
    Path(id): Path<Uuid>,
    Json(_request): Json<CompactConversationRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    // Ownership: a foreign / missing conversation is a 404 (never leak existence).
    let conversation = Repos
        .chat
        .core
        .get_conversation(id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    let branch_id = conversation.active_branch_id.ok_or_else(|| {
        AppError::bad_request("NO_BRANCH", "Conversation has no active branch to compact")
    })?;
    let model_id = conversation.model_id.ok_or_else(|| {
        AppError::bad_request("NO_MODEL", "Conversation has no model to summarize with")
    })?;

    // Force a compaction now: `Some(1)` biases the trigger low so a manual
    // `/compact` summarizes even below the automatic threshold. Best-effort — a
    // summarizer failure must NOT fail the affordance; the marker still confirms
    // the user's action and the conversation stays usable.
    if let Err(e) = crate::modules::summarization::engine::summarizer::refresh_summary(
        branch_id, model_id, Some(1),
    )
    .await
    {
        tracing::warn!("manual /compact (conversation {id}): refresh_summary failed (soft): {e}");
    }

    // Emit the timeline marker (ITEM-61) on the raw side-channel — the same channel
    // `taskListChanged` / `historyReplaced`-from-the-loop use — so a viewing client
    // renders the "context compacted" divider live.
    let event = SSEChatStreamEvent::HistoryReplaced(SSEChatStreamHistoryReplacedData {
        conversation_id: id,
        summary_upto: 0,
    });
    publish_raw_event(auth.user.id, id, event.into());

    Ok((StatusCode::OK, Json(serde_json::json!({ "compacted": true }))))
}

pub fn compact_conversation_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsEdit,)>(op)
        .id("Conversation.compact")
        .tag("Chat")
        .summary("Manually compact a conversation's context")
        .description(
            "Regenerate the active branch's rolling summary and emit a \
             `historyReplaced` timeline marker. Outbound-only: stored messages are \
             never rewritten or deleted.",
        )
        .response::<200, Json<serde_json::Value>>()
        .response_with::<400, (), _>(|res| res.description("No active branch or model"))
        .response_with::<404, (), _>(|res| res.description("Conversation not found"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}
