//! MCP approval workflow repository

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

use crate::modules::mcp::chat_extension::defaults::models::LoopSettings;
use super::models::{
    ApprovalMode, AutoApprovedServer, ConversationMcpSettings, DisabledServer, ToolUseApproval,
};

// ============================================================================
// Conversation MCP Settings
// ============================================================================

/// Get MCP settings for a conversation.
///
/// Reads from the unified `mcp_settings` table (migration 78). The
/// `ConversationMcpSettings` return type is preserved as a typed view
/// — its `FromRow`/column-name shape happens to match the unified
/// table's conversation-scoped row 1:1 (NOT NULL on conversation_id +
/// NULL on project_id are guaranteed by the SQL filter + the table's
/// CHECK constraint).
pub async fn get_conversation_settings(
    pool: &PgPool,
    conversation_id: Uuid,
) -> Result<Option<ConversationMcpSettings>, AppError> {
    // Force non-NULL on conversation_id via the column alias trick:
    // `mcp_settings.conversation_id` is nullable in the schema but
    // every row matching `WHERE conversation_id = $1` has it set.
    let settings = sqlx::query_as!(
        ConversationMcpSettings,
        r#"
        SELECT
            id,
            conversation_id as "conversation_id!: _",
            user_id,
            approval_mode, auto_approved_tools, disabled_servers, loop_settings,
            created_at as "created_at: _", updated_at as "updated_at: _"
        FROM mcp_settings
        WHERE conversation_id = $1
        "#,
        conversation_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(settings)
}

/// Upsert MCP settings for a conversation.
///
/// Writes into the unified `mcp_settings` table — sets `conversation_id`
/// and leaves `project_id` NULL (CHECK constraint enforces exactly one).
/// `auto_approved_tools`: None = preserve existing DB value; Some(tools) = overwrite.
pub async fn upsert_conversation_settings(
    pool: &PgPool,
    conversation_id: Uuid,
    user_id: Uuid,
    approval_mode: ApprovalMode,
    auto_approved_tools: Option<&[AutoApprovedServer]>,
    disabled_servers: &[DisabledServer],
    loop_settings: &LoopSettings,
) -> Result<ConversationMcpSettings, AppError> {
    let auto_approved_tools_json = match auto_approved_tools {
        Some(tools) => serde_json::to_value(tools)
            .map_err(|e| AppError::internal_error(format!("Failed to serialize auto_approved_tools: {}", e)))?,
        None => serde_json::Value::Null,
    };
    let disabled_servers_json = serde_json::to_value(disabled_servers)
        .map_err(|e| AppError::internal_error(format!("Failed to serialize disabled_servers: {}", e)))?;
    let loop_settings_json = serde_json::to_value(loop_settings)
        .map_err(|e| AppError::internal_error(format!("Failed to serialize loop_settings: {}", e)))?;

    let settings = sqlx::query_as!(
        ConversationMcpSettings,
        r#"
        INSERT INTO mcp_settings (
            conversation_id, user_id, approval_mode, auto_approved_tools, disabled_servers, loop_settings
        )
        VALUES ($1, $2, $3, COALESCE($4, '[]'::jsonb), $5, $6)
        ON CONFLICT (conversation_id)
        DO UPDATE SET
            approval_mode = EXCLUDED.approval_mode,
            auto_approved_tools = COALESCE($4, mcp_settings.auto_approved_tools),
            disabled_servers = EXCLUDED.disabled_servers,
            loop_settings = EXCLUDED.loop_settings,
            updated_at = NOW()
        RETURNING
            id,
            conversation_id as "conversation_id!: _",
            user_id,
            approval_mode, auto_approved_tools, disabled_servers, loop_settings,
            created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        conversation_id,
        user_id,
        approval_mode.to_string(),
        auto_approved_tools_json,
        disabled_servers_json,
        loop_settings_json
    )
    .fetch_one(pool)
    .await?;

    Ok(settings)
}

// ============================================================================
// Tool Use Approvals
// ============================================================================

/// Maximum serialized size of a stored tool_input JSON blob. LLMs can
/// in principle return arbitrarily-large structured inputs; without
/// this cap, a runaway model (or a prompt-injection-induced one) can
/// fill the approvals table with multi-MB rows. 256 KiB matches what
/// every real tool genuinely needs and is well below the typical
/// model context. Closes 04-chat F-05 (Medium).
const MAX_TOOL_INPUT_BYTES: usize = 256 * 1024;

/// One pending-approval row to insert in a batch.
pub struct NewToolApproval {
    pub tool_use_id: String,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub server_id: Option<Uuid>,
    pub server_name: String,
}

/// Create pending tool use approvals in a batch (INSERT, not individual).
/// Returns the inserted rows.
pub async fn create_tool_approvals(
    pool: &PgPool,
    conversation_id: Uuid,
    branch_id: Uuid,
    message_id: Uuid,
    user_id: Uuid,
    items: &[NewToolApproval],
) -> Result<Vec<ToolUseApproval>, AppError> {
    if items.is_empty() {
        return Ok(Vec::new());
    }
    // Reject oversized tool_input before any of the batch lands in the DB.
    for it in items {
        let serialized_len = serde_json::to_string(&it.tool_input)
            .map(|s| s.len())
            .unwrap_or(0);
        if serialized_len > MAX_TOOL_INPUT_BYTES {
            return Err(AppError::bad_request(
                "TOOL_INPUT_TOO_LARGE",
                format!(
                    "tool_input is {} bytes serialized; cap is {}",
                    serialized_len, MAX_TOOL_INPUT_BYTES
                ),
            ));
        }
    }
    let tool_use_ids: Vec<String> = items.iter().map(|i| i.tool_use_id.clone()).collect();
    let tool_names: Vec<String> = items.iter().map(|i| i.tool_name.clone()).collect();
    let tool_inputs: Vec<serde_json::Value> = items.iter().map(|i| i.tool_input.clone()).collect();
    let server_ids: Vec<Option<Uuid>> = items.iter().map(|i| i.server_id).collect();
    let server_names: Vec<String> = items.iter().map(|i| i.server_name.clone()).collect();

    let rows = sqlx::query_as!(
        ToolUseApproval,
        r#"
        INSERT INTO tool_use_approvals (
            conversation_id, branch_id, message_id, user_id,
            tool_use_id, tool_name, tool_input, server_id, server_name, status
        )
        SELECT $1, $2, $3, $4,
               t.tool_use_id, t.tool_name, t.tool_input, t.server_id, t.server_name, 'pending'
        FROM UNNEST($5::text[], $6::text[], $7::jsonb[], $8::uuid[], $9::text[])
            AS t(tool_use_id, tool_name, tool_input, server_id, server_name)
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
        &tool_use_ids,
        &tool_names,
        &tool_inputs,
        &server_ids as &[Option<Uuid>],
        &server_names
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
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

/// Get all denied tools for a branch
pub async fn get_denied_tools_for_branch(
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
        WHERE branch_id = $1 AND status = 'denied'
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

/// Consume a tool-use approval record.
///
/// Returns whether a row was actually deleted, which is the CLAIM verdict its main
/// caller depends on: `execute_approved_tools_sync` deletes the row BEFORE running
/// the tool, so `true` means "we own this execution" and `false` means a concurrent
/// pass already claimed it and this one must not execute. Callers must not discard
/// the bool.
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
