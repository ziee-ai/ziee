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
            repository::{branches as branch_repo, conversations as conv_repo},
            types::CreateBranchRequest,
        },
        permissions::{extractors::RequirePermissions, with_permission},
    },
};

// =====================================================
// Branch Handlers
// =====================================================

/// Create a new branch (for edit/regenerate functionality)
#[debug_handler]
pub async fn create_branch(
    auth: RequirePermissions<(BranchesCreate,)>,

    Path(conversation_id): Path<Uuid>,
    Json(request): Json<CreateBranchRequest>,
) -> ApiResult<Json<Branch>> {
    // Verify conversation exists and user owns it
    let conversation = conv_repo::get_conversation(Repos.pool(), conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    // Ensure conversation has an active branch to use as parent
    let parent_branch_id = conversation.active_branch_id.ok_or_else(|| {
        AppError::bad_request(
            "NO_ACTIVE_BRANCH",
            "Conversation must have an active branch to create a new branch from",
        )
    })?;

    // Create new branch with message cloning (handled in repository)
    let branch = branch_repo::create_branch(
        Repos.pool(),
        conversation_id,
        parent_branch_id,
        request.from_message_id,
    )
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
    let _conversation = conv_repo::get_conversation(Repos.pool(), conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    let branches = branch_repo::list_branches(Repos.pool(), conversation_id).await?;

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

    Path((conversation_id, branch_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    // Verify conversation exists and user owns it
    let _conversation = conv_repo::get_conversation(Repos.pool(), conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    // Verify branch exists and belongs to this conversation
    let branch = branch_repo::get_branch(Repos.pool(), branch_id)
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
    branch_repo::set_active_branch(Repos.pool(), conversation_id, branch_id).await?;

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
