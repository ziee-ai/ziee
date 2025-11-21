//! MCP approval workflow repository

use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::common::AppError;

use super::models::{
    ApprovalMode, ApprovalStatus, AutoApprovedTool, ConversationMcpSettings, ToolUseApproval,
};

// ============================================================================
// Helper Functions
// ============================================================================

/// Normalize auto_approved_tools JSONB to canonical string format
/// Supports 3 formats: string, object with server_id, object with server_name
pub async fn normalize_auto_approved_tools(
    pool: &PgPool,
    auto_approved_tools: &serde_json::Value,
) -> Result<Vec<String>, AppError> {
    // Parse JSONB into AutoApprovedTool enum
    let tools: Vec<AutoApprovedTool> = serde_json::from_value(auto_approved_tools.clone())
        .map_err(|e| AppError::bad_request("INVALID_AUTO_APPROVED_TOOLS", format!("Failed to parse auto_approved_tools: {}", e)))?;

    // Build server_id -> server_name map for normalization
    let mut server_id_map: HashMap<Uuid, String> = HashMap::new();

    // Collect all server_ids that need lookup
    let server_ids: Vec<Uuid> = tools
        .iter()
        .filter_map(|tool| match tool {
            AutoApprovedTool::WithServerId { server_id, .. } => Some(*server_id),
            _ => None,
        })
        .collect();

    // Fetch server names if needed
    if !server_ids.is_empty() {
        let servers = sqlx::query!(
            r#"
            SELECT id, name
            FROM mcp_servers
            WHERE id = ANY($1)
            "#,
            &server_ids
        )
        .fetch_all(pool)
        .await?;

        for server in servers {
            server_id_map.insert(server.id, server.name);
        }
    }

    // Normalize all tools to canonical string format
    let normalized: Vec<String> = tools
        .iter()
        .filter_map(|tool| tool.to_canonical_string(Some(&server_id_map)))
        .collect();

    Ok(normalized)
}

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
            approval_mode, auto_approved_tools,
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
    auto_approved_tools: serde_json::Value,
) -> Result<ConversationMcpSettings, AppError> {
    let auto_approved_tools_json = auto_approved_tools;

    let settings = sqlx::query_as!(
        ConversationMcpSettings,
        r#"
        INSERT INTO conversation_mcp_settings (
            conversation_id, user_id, approval_mode, auto_approved_tools
        )
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (conversation_id)
        DO UPDATE SET
            approval_mode = EXCLUDED.approval_mode,
            auto_approved_tools = EXCLUDED.auto_approved_tools,
            updated_at = NOW()
        RETURNING
            id, conversation_id, user_id,
            approval_mode, auto_approved_tools,
            created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        conversation_id,
        user_id,
        approval_mode.to_string(),
        auto_approved_tools_json
    )
    .fetch_one(pool)
    .await?;

    Ok(settings)
}

/// Delete MCP settings for a conversation
pub async fn delete_conversation_settings(
    pool: &PgPool,
    conversation_id: Uuid,
) -> Result<bool, AppError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM conversation_mcp_settings
        WHERE conversation_id = $1
        "#,
        conversation_id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
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

/// Get pending approvals for a message
pub async fn get_pending_approvals_for_message(
    pool: &PgPool,
    message_id: Uuid,
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
        WHERE message_id = $1 AND status = 'pending'
        ORDER BY created_at ASC
        "#,
        message_id
    )
    .fetch_all(pool)
    .await?;

    Ok(approvals)
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
    .fetch_one(pool)
    .await?;

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
    .fetch_one(pool)
    .await?;

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

/// Batch approve multiple tool uses (all-or-nothing)
/// Returns the number of approvals updated
pub async fn batch_approve_tool_uses(
    pool: &PgPool,
    message_id: Uuid,
    tool_use_ids: Vec<String>,
    approved_by: Uuid,
) -> Result<Vec<ToolUseApproval>, AppError> {
    // Start a transaction for all-or-nothing semantics
    let mut tx = pool.begin().await?;

    // Check that all tool uses exist and are pending
    let pending_count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM tool_use_approvals
        WHERE message_id = $1 AND tool_use_id = ANY($2) AND status = 'pending'
        "#,
        message_id,
        &tool_use_ids
    )
    .fetch_one(&mut *tx)
    .await?;

    if pending_count != tool_use_ids.len() as i64 {
        return Err(AppError::bad_request(
            "INVALID_APPROVAL_BATCH",
            format!(
                "Not all tool uses are pending. Expected {}, found {}",
                tool_use_ids.len(),
                pending_count
            ),
        ));
    }

    // Approve all tool uses
    let approvals = sqlx::query_as!(
        ToolUseApproval,
        r#"
        UPDATE tool_use_approvals
        SET
            status = 'approved',
            approved_at = NOW(),
            approved_by = $3,
            updated_at = NOW()
        WHERE message_id = $1 AND tool_use_id = ANY($2) AND status = 'pending'
        RETURNING
            id, conversation_id, branch_id, message_id, user_id,
            tool_use_id, tool_name, tool_input, server_id, server_name, status,
            approved_at as "approved_at: _", approved_by, approval_note,
            created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        message_id,
        &tool_use_ids,
        approved_by
    )
    .fetch_all(&mut *tx)
    .await?;

    // Commit transaction
    tx.commit().await?;

    Ok(approvals)
}

/// Get approval by tool_use_id and message_id
pub async fn get_tool_approval(
    pool: &PgPool,
    tool_use_id: String,
    message_id: Uuid,
) -> Result<Option<ToolUseApproval>, AppError> {
    let approval = sqlx::query_as!(
        ToolUseApproval,
        r#"
        SELECT
            id, conversation_id, branch_id, message_id, user_id,
            tool_use_id, tool_name, tool_input, server_id, server_name, status,
            approved_at as "approved_at: _", approved_by, approval_note,
            created_at as "created_at: _", updated_at as "updated_at: _"
        FROM tool_use_approvals
        WHERE tool_use_id = $1 AND message_id = $2
        "#,
        tool_use_id,
        message_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(approval)
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
