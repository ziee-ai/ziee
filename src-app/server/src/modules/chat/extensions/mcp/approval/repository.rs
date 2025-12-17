//! MCP approval workflow repository

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

use super::models::{
    ApprovalMode, AutoApprovedServer, ConversationMcpSettings, DisabledServer, ToolUseApproval,
};

// ============================================================================
// Conversation MCP Settings
// ============================================================================

/// Get MCP settings for a conversation
pub async fn get_conversation_settings(
    pool: &PgPool,
    conversation_id: Uuid,
) -> Result<Option<ConversationMcpSettings>, AppError> {
    let settings = sqlx::query_as!(
        ConversationMcpSettings,
        r#"
        SELECT
            id, conversation_id, user_id,
            approval_mode, auto_approved_tools, disabled_servers,
            created_at as "created_at: _", updated_at as "updated_at: _"
        FROM conversation_mcp_settings
        WHERE conversation_id = $1
        "#,
        conversation_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(settings)
}

/// Upsert MCP settings for a conversation
pub async fn upsert_conversation_settings(
    pool: &PgPool,
    conversation_id: Uuid,
    user_id: Uuid,
    approval_mode: ApprovalMode,
    auto_approved_tools: &[AutoApprovedServer],
    disabled_servers: &[DisabledServer],
) -> Result<ConversationMcpSettings, AppError> {
    let auto_approved_tools_json = serde_json::to_value(auto_approved_tools)
        .map_err(|e| AppError::internal_error(format!("Failed to serialize auto_approved_tools: {}", e)))?;
    let disabled_servers_json = serde_json::to_value(disabled_servers)
        .map_err(|e| AppError::internal_error(format!("Failed to serialize disabled_servers: {}", e)))?;

    let settings = sqlx::query_as!(
        ConversationMcpSettings,
        r#"
        INSERT INTO conversation_mcp_settings (
            conversation_id, user_id, approval_mode, auto_approved_tools, disabled_servers
        )
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (conversation_id)
        DO UPDATE SET
            approval_mode = EXCLUDED.approval_mode,
            auto_approved_tools = EXCLUDED.auto_approved_tools,
            disabled_servers = EXCLUDED.disabled_servers,
            updated_at = NOW()
        RETURNING
            id, conversation_id, user_id,
            approval_mode, auto_approved_tools, disabled_servers,
            created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        conversation_id,
        user_id,
        approval_mode.to_string(),
        auto_approved_tools_json,
        disabled_servers_json
    )
    .fetch_one(pool)
    .await?;

    Ok(settings)
}

// ============================================================================
// Tool Use Approvals
// ============================================================================

/// Create a pending tool use approval
pub async fn create_tool_approval(
    pool: &PgPool,
    conversation_id: Uuid,
    branch_id: Uuid,
    message_id: Uuid,
    user_id: Uuid,
    tool_use_id: String,
    tool_name: String,
    tool_input: serde_json::Value,
    server_id: Option<Uuid>,
    server_name: String,
) -> Result<ToolUseApproval, AppError> {
    let approval = sqlx::query_as!(
        ToolUseApproval,
        r#"
        INSERT INTO tool_use_approvals (
            conversation_id, branch_id, message_id, user_id,
            tool_use_id, tool_name, tool_input, server_id, server_name, status
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'pending')
        RETURNING
            id, conversation_id, branch_id, message_id, user_id,
            tool_use_id, tool_name, tool_input, server_id, server_name, status,
            approved_at as "approved_at: _", approved_by, approval_note,
            created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        conversation_id,
        branch_id,
        message_id,
        user_id,
        tool_use_id,
        tool_name,
        tool_input,
        server_id,
        server_name
    )
    .fetch_one(pool)
    .await?;

    Ok(approval)
}

/// Get all pending approvals for a branch
pub async fn get_pending_approvals_for_branch(
    pool: &PgPool,
    branch_id: Uuid,
) -> Result<Vec<ToolUseApproval>, AppError> {
    let approvals = sqlx::query_as!(
        ToolUseApproval,
        r#"
        SELECT
            id, conversation_id, branch_id, message_id, user_id,
            tool_use_id, tool_name, tool_input, server_id, server_name, status,
            approved_at as "approved_at: _", approved_by, approval_note,
            created_at as "created_at: _", updated_at as "updated_at: _"
        FROM tool_use_approvals
        WHERE branch_id = $1 AND status = 'pending'
        ORDER BY created_at ASC
        "#,
        branch_id
    )
    .fetch_all(pool)
    .await?;

    Ok(approvals)
}

/// Get all approved tools for a branch
pub async fn get_approved_tools_for_branch(
    pool: &PgPool,
    branch_id: Uuid,
) -> Result<Vec<ToolUseApproval>, AppError> {
    let approvals = sqlx::query_as!(
        ToolUseApproval,
        r#"
        SELECT
            id, conversation_id, branch_id, message_id, user_id,
            tool_use_id, tool_name, tool_input, server_id, server_name, status,
            approved_at as "approved_at: _", approved_by, approval_note,
            created_at as "created_at: _", updated_at as "updated_at: _"
        FROM tool_use_approvals
        WHERE branch_id = $1 AND status = 'approved'
        ORDER BY created_at ASC
        "#,
        branch_id
    )
    .fetch_all(pool)
    .await?;

    Ok(approvals)
}

/// Approve a tool use
pub async fn approve_tool_use(
    pool: &PgPool,
    tool_use_id: String,
    branch_id: Uuid,
    approved_by: Uuid,
    note: Option<String>,
) -> Result<ToolUseApproval, AppError> {
    let approval = sqlx::query_as!(
        ToolUseApproval,
        r#"
        UPDATE tool_use_approvals
        SET
            status = 'approved',
            approved_at = NOW(),
            approved_by = $3,
            approval_note = $4,
            updated_at = NOW()
        WHERE tool_use_id = $1 AND branch_id = $2 AND status = 'pending'
        RETURNING
            id, conversation_id, branch_id, message_id, user_id,
            tool_use_id, tool_name, tool_input, server_id, server_name, status,
            approved_at as "approved_at: _", approved_by, approval_note,
            created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        tool_use_id,
        branch_id,
        approved_by,
        note
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        AppError::not_found("Approval not found or already processed")
    })?;

    Ok(approval)
}

/// Deny a tool use
pub async fn deny_tool_use(
    pool: &PgPool,
    tool_use_id: String,
    branch_id: Uuid,
    approved_by: Uuid,
    note: Option<String>,
) -> Result<ToolUseApproval, AppError> {
    let approval = sqlx::query_as!(
        ToolUseApproval,
        r#"
        UPDATE tool_use_approvals
        SET
            status = 'denied',
            approved_at = NOW(),
            approved_by = $3,
            approval_note = $4,
            updated_at = NOW()
        WHERE tool_use_id = $1 AND branch_id = $2 AND status = 'pending'
        RETURNING
            id, conversation_id, branch_id, message_id, user_id,
            tool_use_id, tool_name, tool_input, server_id, server_name, status,
            approved_at as "approved_at: _", approved_by, approval_note,
            created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        tool_use_id,
        branch_id,
        approved_by,
        note
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        AppError::not_found("Approval not found or already processed")
    })?;

    Ok(approval)
}

/// Cancel all pending approvals for a branch
/// Used when a new message is sent on a different branch
pub async fn cancel_pending_approvals_for_branch(
    pool: &PgPool,
    branch_id: Uuid,
) -> Result<u64, AppError> {
    let result = sqlx::query!(
        r#"
        UPDATE tool_use_approvals
        SET
            status = 'cancelled',
            updated_at = NOW()
        WHERE branch_id = $1 AND status = 'pending'
        "#,
        branch_id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// Delete tool use approval record (after execution)
pub async fn delete_tool_approval(
    pool: &PgPool,
    tool_use_id: String,
    message_id: Uuid,
) -> Result<bool, AppError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM tool_use_approvals
        WHERE tool_use_id = $1 AND message_id = $2
        "#,
        tool_use_id,
        message_id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}
