// HTTP handlers for project's conversation-relationship routes.
// Relocated from `project/handlers.rs` as part of the project↔chat
// inversion.
//
// Why these handlers live here: they all return chat types
// (`ConversationResponse`) or operate on conversation-id URLs, so
// importing chat types is the architecturally-correct direction inside
// the `chat_extension/` bridge. Project core stays chat-agnostic.

use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::{Extension, Path, Query},
    http::StatusCode,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::{EventBus, Repos};
use crate::modules::chat::core::types::ConversationResponse;
use crate::modules::permissions::{extractors::RequirePermissions, with_permission};
use crate::modules::project::core::extension::get_global_registry as get_project_extension_registry;
use crate::modules::project::events::ProjectEvent;
use crate::modules::project::handlers::PaginationQuery;
use crate::modules::project::models::Project;
use crate::modules::project::permissions::{ProjectsEdit, ProjectsRead};
use crate::modules::sync::{SyncAction, SyncEntity, SyncOrigin, publish as sync_publish};

#[debug_handler]
pub async fn list_project_conversations(
    auth: RequirePermissions<(ProjectsRead,)>,
    Path(id): Path<Uuid>,
    Query(query): Query<PaginationQuery>,
) -> ApiResult<Json<Vec<ConversationResponse>>> {
    // Project must exist and be owned by the user.
    let _ = Repos
        .project
        .get_for_user(id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Project"))?;

    let (page, limit) = query.resolved();
    let offset = (page - 1).saturating_mul(limit);
    let conversations = Repos
        .chat
        .project
        .list_conversations_in_project(id, auth.user.id, limit, offset)
        .await?;
    Ok((StatusCode::OK, Json(conversations)))
}

pub fn list_project_conversations_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProjectsRead,)>(op)
        .id("Project.listConversations")
        .tag("Projects")
        .summary("List conversations in a project")
        .response::<200, Json<Vec<ConversationResponse>>>()
        .response_with::<404, (), _>(|res| res.description("Project not found"))
}

/// Look up the project a conversation currently belongs to.
/// Returns the project the given conversation is attached to, or
/// `null` if the conversation is unfiled / doesn't exist / belongs to
/// a different user. Always 200 — "unfiled" is legitimate data, not
/// an error condition, and treating it as a 404 caused noisy client-
/// side error logs on every chat surface load (chat-extension polls
/// this for every loaded conversation).
#[debug_handler]
pub async fn project_for_conversation(
    auth: RequirePermissions<(ProjectsRead,)>,
    Path(conversation_id): Path<Uuid>,
) -> ApiResult<Json<Option<Project>>> {
    let project = Repos
        .project
        .get_for_conversation(conversation_id, auth.user.id)
        .await?;
    Ok((StatusCode::OK, Json(project)))
}

pub fn project_for_conversation_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProjectsRead,)>(op)
        .id("Project.forConversation")
        .tag("Projects")
        .summary("Resolve the project a conversation is attached to")
        .description(
            "Returns the project the given conversation is currently attached to, \
             or `null` if the conversation is unfiled, doesn't exist, or belongs to \
             a different user. Always 200 — \"unfiled\" is legitimate data."
        )
        .response::<200, Json<Option<Project>>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<403, (), _>(|res| res.description("Missing required permissions"))
}

/// Attach an existing conversation to this project (or re-attach
/// from another project). Idempotent: re-POSTing the same (project,
/// conv) pair refreshes the MCP snapshot from the project's current
/// defaults. Serves both "create new chat in project" (chat creates
/// unfiled, then the frontend attaches) and "move existing chat
/// into project" use cases. Mirrors `attach_file`/`detach_file`.
#[debug_handler]
pub async fn attach_conversation(
    auth: RequirePermissions<(
        ProjectsEdit,
        crate::modules::chat::core::permissions::ConversationsEdit,
    )>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path((project_id, conversation_id)): Path<(Uuid, Uuid)>,
    origin: SyncOrigin,
) -> ApiResult<Json<ConversationResponse>> {
    // The project-extension registry was previously injected via
    // `axum::Extension` from project's router setup. Now that this
    // handler is mounted under chat's router (via the chat-extension
    // `register_routes` hook), we fetch it from project's global
    // OnceCell instead — same registry, different access path.
    let extension_registry = get_project_extension_registry().ok_or_else(|| {
        AppError::internal_error("Project extension registry not initialized")
    })?;
    // 1. Validate project ownership (404 if missing or foreign).
    let _project = Repos
        .project
        .get_for_user(project_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Project"))?;

    // 2. Validate conversation ownership.
    if !Repos
        .project
        .user_owns_conversation(conversation_id, auth.user.id)
        .await?
    {
        return Err(AppError::not_found("Conversation").into());
    }

    // 3. Atomic: insert/update join row + refresh MCP snapshot.
    let mut tx = Repos
        .pool()
        .begin()
        .await
        .map_err(AppError::database_error)?;

    let from_project_id = Repos
        .project
        .attach_conversation_in_tx(&mut tx, project_id, conversation_id)
        .await?;

    // Fan out the conversation-attach hook to every registered project
    // extension. MCP's impl snapshots the project's mcp_settings row
    // into a conversation-scoped row on the unified mcp_settings table.
    extension_registry
        .fire_on_conversation_attached(project_id, conversation_id, auth.user.id, &mut tx)
        .await?;

    tx.commit().await.map_err(AppError::database_error)?;

    event_bus.emit_async(ProjectEvent::conversation_attached(
        conversation_id,
        project_id,
        from_project_id,
        auth.user.id,
    ));

    // Re-fetch from the project's conversation list so the response
    // carries the canonical message_count shape clients see elsewhere.
    let convs = Repos
        .chat
        .project
        .list_conversations_in_project(project_id, auth.user.id, 1, 0)
        .await?;
    let response = convs
        .into_iter()
        .find(|c| c.conversation.id == conversation_id)
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    // The project's conversation membership changed → refresh the owner's
    // other devices.
    sync_publish(
        SyncEntity::Project,
        SyncAction::Update,
        project_id,
        Some(auth.user.id),
        origin.0,
    );

    Ok((StatusCode::OK, Json(response)))
}

pub fn attach_conversation_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(
        ProjectsEdit,
        crate::modules::chat::core::permissions::ConversationsEdit,
    )>(op)
        .id("Project.attachConversation")
        .tag("Projects")
        .summary("Attach (or re-attach) a conversation to this project")
        .description(
            "Attach an existing conversation to this project. Idempotent: re-POSTing the same \
             (project, conv) pair refreshes the project MCP snapshot stored on the conversation. \
             Cross-project moves (attach a conversation already in project A into project B) \
             re-snapshot from B's MCP defaults via ON CONFLICT DO UPDATE.\n\
             \n\
             Use cases:\n\
             - **Create new chat in project**: chat creates an unfiled conversation, then the \
               frontend's project chat extension calls this endpoint to file it.\n\
             - **Move existing chat into project**: sidebar drag-drop or context menu calls \
               this directly.",
        )
        .response::<200, Json<ConversationResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<403, (), _>(|res| res.description("Missing required permissions"))
        .response_with::<404, (), _>(|res| res.description("Project or conversation not found"))
}

/// Detach a conversation from this project ("unfile" it). Clears the
/// MCP snapshot row so subsequent chat use falls back to user/global
/// MCP defaults.
#[debug_handler]
pub async fn detach_conversation(
    auth: RequirePermissions<(
        ProjectsEdit,
        crate::modules::chat::core::permissions::ConversationsEdit,
    )>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path((project_id, conversation_id)): Path<(Uuid, Uuid)>,
    origin: SyncOrigin,
) -> ApiResult<()> {
    let extension_registry = get_project_extension_registry().ok_or_else(|| {
        AppError::internal_error("Project extension registry not initialized")
    })?;
    // 1. Validate project ownership.
    let _project = Repos
        .project
        .get_for_user(project_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Project"))?;

    // 2. Validate conversation ownership.
    if !Repos
        .project
        .user_owns_conversation(conversation_id, auth.user.id)
        .await?
    {
        return Err(AppError::not_found("Conversation").into());
    }

    // 3. Atomic: drop the join row + fire the conversation-detach hook
    // to every project extension. Mcp's hook deletes the conversation's
    // mcp_settings row so chat falls back to user/global defaults.
    // The detach repo call returns false if no row was deleted (the
    // conversation wasn't actually attached to THIS project) —
    // map to 404 so DELETE on a non-membership is a clean
    // mis-addressed-request error.
    let mut tx = Repos
        .pool()
        .begin()
        .await
        .map_err(AppError::database_error)?;

    let detached = Repos
        .project
        .detach_conversation_in_tx(&mut tx, project_id, conversation_id)
        .await?;
    if !detached {
        // No need to commit — the implicit rollback drops the empty tx.
        return Err(AppError::not_found("Conversation in project").into());
    }
    extension_registry
        .fire_on_conversation_detached(conversation_id, &mut tx)
        .await?;

    tx.commit().await.map_err(AppError::database_error)?;

    event_bus.emit_async(ProjectEvent::conversation_detached(
        conversation_id,
        project_id,
        auth.user.id,
    ));

    // The project's conversation membership changed → refresh the owner's
    // other devices.
    sync_publish(
        SyncEntity::Project,
        SyncAction::Update,
        project_id,
        Some(auth.user.id),
        origin.0,
    );

    Ok((StatusCode::NO_CONTENT, ()))
}

pub fn detach_conversation_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(
        ProjectsEdit,
        crate::modules::chat::core::permissions::ConversationsEdit,
    )>(op)
        .id("Project.detachConversation")
        .tag("Projects")
        .summary("Detach a conversation from this project")
        .description(
            "Detach a conversation from this project (it becomes unfiled). Clears the per-conversation \
             MCP snapshot so subsequent chat use falls back to user/global MCP defaults.",
        )
        .response_with::<204, (), _>(|res| res.description("Conversation detached"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<403, (), _>(|res| res.description("Missing required permissions"))
        .response_with::<404, (), _>(|res| {
            res.description("Project not found, conversation not found, or conversation not in this project")
        })
}
