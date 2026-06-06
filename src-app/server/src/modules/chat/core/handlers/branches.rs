// Branch handlers - Operations for conversation branches (edit/regenerate)

use crate::core::Repos;
use aide::transform::TransformOperation;
use axum::{Json, debug_handler, extract::Path, http::StatusCode};
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    modules::{
        chat::core::{
            models::Branch,
            permissions::*,
            types::CreateBranchRequest,
        },
        permissions::{extractors::RequirePermissions, with_permission},
        sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish},
    },
};

// =====================================================
// Branch Handlers
// =====================================================

/// Cap on the number of branches a single conversation can have.
///
/// Closes 04-chat F-08 (Medium): without this cap, a user with
/// `branches::create` permission can spam create_branch in a tight loop,
/// growing the `branches` row count and `branch_messages` association
/// table without bound. Even modest abuse (10 calls per second) hits
/// 100K rows / hour. 256 is large enough for any reasonable
/// edit/regenerate workflow.
const MAX_BRANCHES_PER_CONVERSATION: i64 = 256;

/// Create a new branch (for edit/regenerate functionality)
#[debug_handler]
pub async fn create_branch(
    auth: RequirePermissions<(BranchesCreate,)>,

    Path(conversation_id): Path<Uuid>,
    Json(request): Json<CreateBranchRequest>,
) -> ApiResult<Json<Branch>> {
    // Verify conversation exists and user owns it
    let conversation = Repos.chat.core.get_conversation( conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    // Ensure conversation has an active branch to use as parent
    let parent_branch_id = conversation.active_branch_id.ok_or_else(|| {
        AppError::bad_request(
            "NO_ACTIVE_BRANCH",
            "Conversation must have an active branch to create a new branch from",
        )
    })?;

    // SECURITY: enforce a per-conversation branch cap (04-chat F-08).
    let existing = Repos.chat.core
        .list_branches(conversation_id)
        .await?;
    if existing.len() as i64 >= MAX_BRANCHES_PER_CONVERSATION {
        return Err(AppError::bad_request(
            "BRANCH_LIMIT",
            "Maximum number of branches per conversation reached",
        )
        .into());
    }

    // Create new branch with message cloning (handled in repository)
    let branch = Repos.chat.core
        .create_branch(conversation_id, parent_branch_id, request.from_message_id, &request.fork_level)
        .await?;

    Ok((StatusCode::CREATED, Json(branch)))
}

pub fn create_branch_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(BranchesCreate,)>(op)
        .id("Branch.create")
        .tag("Chat")
        .summary("Create a new branch")
        .description("Create a new conversation branch for edit/regenerate functionality. Optionally copy messages up to a specific point.")
        .response::<201, Json<Branch>>()
        .response_with::<404, (), _>(|res| res.description("Conversation not found"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// List all branches for a conversation
#[debug_handler]
pub async fn list_branches(
    auth: RequirePermissions<(ConversationsRead,)>,

    Path(conversation_id): Path<Uuid>,
) -> ApiResult<Json<Vec<Branch>>> {
    // Verify conversation exists and user owns it
    let _conversation = Repos.chat.core.get_conversation( conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    let branches = Repos.chat.core.list_branches( conversation_id).await?;

    Ok((StatusCode::OK, Json(branches)))
}

pub fn list_branches_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsRead,)>(op)
        .id("Branch.list")
        .tag("Chat")
        .summary("List branches")
        .description("List all branches for a conversation")
        .response::<200, Json<Vec<Branch>>>()
        .response_with::<404, (), _>(|res| res.description("Conversation not found"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Switch to a different branch (activate it)
#[debug_handler]
pub async fn activate_branch(
    auth: RequirePermissions<(BranchesSwitch,)>,
    origin: SyncOrigin,
    Path((conversation_id, branch_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    // Verify conversation exists and user owns it
    let _conversation = Repos.chat.core.get_conversation( conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    // Verify branch exists and belongs to this conversation
    let branch = Repos.chat.core
        .get_branch(branch_id)
        .await?
        .ok_or_else(|| AppError::not_found("Branch"))?;

    if branch.conversation_id != conversation_id {
        return Err(AppError::bad_request(
            "INVALID_BRANCH",
            "Branch does not belong to this conversation",
        )
        .into());
    }

    // Activate the branch
    Repos.chat.core.set_active_branch( conversation_id, branch_id).await?;

    sync_publish(
        SyncEntity::Conversation,
        SyncAction::Update,
        conversation_id,
        Audience::owner(auth.user.id),
        origin.0,
    );

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn activate_branch_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(BranchesSwitch,)>(op)
        .id("Branch.activate")
        .tag("Chat")
        .summary("Activate a branch")
        .description("Switch the active branch for a conversation")
        .response_with::<204, (), _>(|res| res.description("Branch activated successfully"))
        .response_with::<404, (), _>(|res| res.description("Conversation or branch not found"))
        .response_with::<400, (), _>(|res| {
            res.description("Branch does not belong to conversation")
        })
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}
