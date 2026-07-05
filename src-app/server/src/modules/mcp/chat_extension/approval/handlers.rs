//! MCP approval workflow handlers

use aide::transform::TransformOperation;
use axum::{
    debug_handler,
    extract::Path,
    http::StatusCode,
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    modules::{
        chat::{
            core::permissions::*,
        },
        permissions::{extractors::RequirePermissions, with_permission},
        sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish},
    },
};

use super::models;

// =====================================================
// Request/Response Types
// =====================================================

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct McpSettingsResponse {
    pub settings: Option<models::ConversationMcpSettingsResponse>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct PendingApprovalsResponse {
    pub approvals: Vec<models::ToolUseApproval>,
}

// =====================================================
// Handlers
// =====================================================

/// Get MCP settings for a conversation
#[debug_handler]
pub async fn get_mcp_settings(
    auth: RequirePermissions<(ConversationsRead,)>,
    Path(conversation_id): Path<Uuid>,
) -> ApiResult<Json<McpSettingsResponse>> {
    // Verify user owns this conversation
    let _conversation = crate::core::Repos
        .chat
        .core
        .get_conversation(conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    // Get MCP settings
    let settings = crate::core::Repos
        .chat
        .mcp
        .get_conversation_settings(conversation_id)
        .await?
        .map(models::ConversationMcpSettingsResponse::from);

    Ok((StatusCode::OK, Json(McpSettingsResponse { settings })))
}

pub fn get_mcp_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsRead,)>(op)
        .id("Conversation.getMcpSettings")
        .tag("Chat")
        .summary("Get MCP settings for a conversation")
        .description("Get the MCP approval settings for a conversation")
        .response::<200, Json<McpSettingsResponse>>()
        .response_with::<404, (), _>(|res| res.description("Conversation not found"))
}

/// Update MCP settings for a conversation
#[debug_handler]
pub async fn update_mcp_settings(
    auth: RequirePermissions<(ConversationsEdit,)>,
    origin: SyncOrigin,
    Path(conversation_id): Path<Uuid>,
    Json(request): Json<models::UpsertMcpSettingsRequest>,
) -> ApiResult<Json<models::ConversationMcpSettingsResponse>> {
    // Verify user owns this conversation
    let _conversation = crate::core::Repos
        .chat
        .core
        .get_conversation(conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    // No validation needed - the type system enforces correct structure

    // Upsert settings
    let settings = crate::core::Repos
        .chat
        .mcp
        .upsert_conversation_settings(
            conversation_id,
            auth.user.id,
            request.approval_mode,
            request.auto_approved_tools.as_deref(),
            &request.disabled_servers,
            &request.loop_settings,
        )
        .await?;

    // The conversation's MCP settings render in the chat MCP panel; notify the
    // owner's other devices so an open conversation refetches (notify-only).
    sync_publish(
        SyncEntity::Conversation,
        SyncAction::Update,
        conversation_id,
        Audience::owner(auth.user.id),
        origin.0,
    );

    Ok((StatusCode::OK, Json(models::ConversationMcpSettingsResponse::from(settings))))
}

pub fn update_mcp_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsEdit,)>(op)
        .id("Conversation.updateMcpSettings")
        .tag("Chat")
        .summary("Update MCP settings for a conversation")
        .description("Create or update the MCP approval settings for a conversation")
        .response::<200, Json<models::ConversationMcpSettingsResponse>>()
        .response_with::<400, (), _>(|res| res.description("Invalid request"))
        .response_with::<404, (), _>(|res| res.description("Conversation not found"))
}

/// Get pending tool approvals for a branch
#[debug_handler]
pub async fn get_pending_approvals_for_branch(
    auth: RequirePermissions<(ConversationsRead,)>,
    Path(branch_id): Path<Uuid>,
) -> ApiResult<Json<PendingApprovalsResponse>> {
    // SECURITY: verify the caller owns the conversation that contains
    // this branch. The original handler used `_auth` and ran the query
    // unconditionally, leaking tool_use_id / tool_name / full tool_input
    // JSON / conversation+message metadata to anyone who could name a
    // branch UUID. Carryover-fix from 2025-01; closes 04-chat F-01
    // (Critical, open ~16 months).
    //
    // Both lookups return 404 (not 403) so an attacker can't distinguish
    // "branch does not exist" from "branch exists but belongs to another
    // user" — prevents UUID enumeration.
    let branch = crate::core::Repos
        .chat
        .core
        .get_branch(branch_id)
        .await?
        .ok_or_else(|| AppError::not_found("Branch"))?;

    let _conversation = crate::core::Repos
        .chat
        .core
        .get_conversation(branch.conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Branch"))?;

    let approvals = crate::core::Repos
        .chat
        .mcp
        .get_pending_approvals_for_branch(branch_id)
        .await?;

    Ok((StatusCode::OK, Json(PendingApprovalsResponse { approvals })))
}

pub fn get_pending_approvals_for_branch_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsRead,)>(op)
        .id("Branch.getPendingApprovals")
        .tag("Chat")
        .summary("Get pending tool approvals for a branch")
        .description("Get all pending tool use approvals for a specific branch (active conversation)")
        .response::<200, Json<PendingApprovalsResponse>>()
        .response_with::<404, (), _>(|res| res.description("Branch not found"))
}
