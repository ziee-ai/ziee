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
    },
};

use super::models;

// =====================================================
// Request/Response Types
// =====================================================

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct McpSettingsResponse {
    pub settings: Option<models::ConversationMcpSettings>,
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
        .await?;

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
    Path(conversation_id): Path<Uuid>,
    Json(request): Json<models::UpsertMcpSettingsRequest>,
) -> ApiResult<Json<models::ConversationMcpSettings>> {
    // Verify user owns this conversation
    let _conversation = crate::core::Repos
        .chat
        .core
        .get_conversation(conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    // Validate auto_approved_tools format by attempting to normalize
    // This will validate all 3 formats: string, object with server_id, object with server_name
    let _normalized = crate::core::Repos
        .chat
        .mcp
        .normalize_auto_approved_tools(&request.auto_approved_tools)
        .await
        .map_err(|e| {
            AppError::bad_request(
                "INVALID_AUTO_APPROVED_TOOLS",
                format!("Invalid auto_approved_tools format: {}", e),
            )
        })?;

    // Additional validation: ensure string format tools contain "__"
    if let Ok(tools) = serde_json::from_value::<Vec<models::AutoApprovedTool>>(request.auto_approved_tools.clone()) {
        for tool in tools {
            if let models::AutoApprovedTool::String(ref tool_name) = tool {
                if !tool_name.contains("__") {
                    return Err(AppError::bad_request(
                        "INVALID_TOOL_NAME",
                        format!("Invalid tool name format: {}. Expected 'server_name__tool_name'", tool_name),
                    )
                    .into());
                }
            }
        }
    }

    // Upsert settings
    let settings = crate::core::Repos
        .chat
        .mcp
        .upsert_conversation_settings(
            conversation_id,
            auth.user.id,
            request.approval_mode,
            request.auto_approved_tools,
        )
        .await?;

    Ok((StatusCode::OK, Json(settings)))
}

pub fn update_mcp_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsEdit,)>(op)
        .id("Conversation.updateMcpSettings")
        .tag("Chat")
        .summary("Update MCP settings for a conversation")
        .description("Create or update the MCP approval settings for a conversation")
        .response::<200, Json<models::ConversationMcpSettings>>()
        .response_with::<400, (), _>(|res| res.description("Invalid request"))
        .response_with::<404, (), _>(|res| res.description("Conversation not found"))
}

/// Get pending tool approvals for a branch
#[debug_handler]
pub async fn get_pending_approvals_for_branch(
    _auth: RequirePermissions<(ConversationsRead,)>,
    Path(branch_id): Path<Uuid>,
) -> ApiResult<Json<PendingApprovalsResponse>> {
    // Get pending approvals for the branch
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
