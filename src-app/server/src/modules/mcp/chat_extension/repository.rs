// MCP extension repository wrapper for chat

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

use super::approval::models::{ApprovalMode, AutoApprovedServer, ConversationMcpSettings, DisabledServer, ToolUseApproval};
use super::approval::repository;
use super::defaults::models::LoopSettings;

/// Repository for MCP extension operations
#[derive(Clone, Debug)]
pub struct McpChatRepository {
    pool: PgPool,
}

impl McpChatRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ===== Conversation MCP Settings =====

    /// Get MCP settings for a conversation
    pub async fn get_conversation_settings(
        &self,
        conversation_id: Uuid,
    ) -> Result<Option<ConversationMcpSettings>, AppError> {
        repository::get_conversation_settings(&self.pool, conversation_id).await
    }

    /// Upsert MCP settings for a conversation
    /// `auto_approved_tools`: None = preserve existing DB value; Some(tools) = overwrite
    pub async fn upsert_conversation_settings(
        &self,
        conversation_id: Uuid,
        user_id: Uuid,
        approval_mode: ApprovalMode,
        auto_approved_tools: Option<&[AutoApprovedServer]>,
        disabled_servers: &[DisabledServer],
        loop_settings: &LoopSettings,
    ) -> Result<ConversationMcpSettings, AppError> {
        repository::upsert_conversation_settings(
            &self.pool,
            conversation_id,
            user_id,
            approval_mode,
            auto_approved_tools,
            disabled_servers,
            loop_settings,
        )
        .await
    }

    // ===== Tool Use Approvals =====

    /// Create a pending tool use approval
    pub async fn create_tool_approval(
        &self,
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
        let items = vec![repository::NewToolApproval {
            tool_use_id,
            tool_name,
            tool_input,
            server_id,
            server_name,
        }];
        let mut results = repository::create_tool_approvals(
            &self.pool,
            conversation_id,
            branch_id,
            message_id,
            user_id,
            &items,
        )
        .await?;
        results.pop().ok_or_else(|| AppError::internal_error("create_tool_approval returned no rows"))
    }

    /// Create many pending tool-use approvals in a single round-trip.
    pub async fn create_tool_approvals(
        &self,
        conversation_id: Uuid,
        branch_id: Uuid,
        message_id: Uuid,
        user_id: Uuid,
        items: &[repository::NewToolApproval],
    ) -> Result<Vec<ToolUseApproval>, AppError> {
        repository::create_tool_approvals(
            &self.pool,
            conversation_id,
            branch_id,
            message_id,
            user_id,
            items,
        )
        .await
    }

    /// Get all pending approvals for a branch
    pub async fn get_pending_approvals_for_branch(
        &self,
        branch_id: Uuid,
    ) -> Result<Vec<ToolUseApproval>, AppError> {
        repository::get_pending_approvals_for_branch(&self.pool, branch_id).await
    }

    /// Delete tool use approval record (after execution)
    pub async fn delete_tool_approval(
        &self,
        tool_use_id: String,
        message_id: Uuid,
    ) -> Result<bool, AppError> {
        repository::delete_tool_approval(&self.pool, tool_use_id, message_id).await
    }

    // ===== Per-message server snapshot (replaces messages.mcp_server_ids) =====

    /// Snapshot the list of MCP servers enabled when a user message
    /// was sent. Used by the frontend mcp extension on Edit to restore
    /// the original server selection (edit-fidelity audit trail).
    ///
    /// No-op if `server_ids` is empty. PRIMARY KEY (message_id, server_id)
    /// makes the INSERT idempotent — duplicate calls for the same
    /// message simply re-insert the same rows (or noop on conflict).
    pub async fn insert_message_servers(
        &self,
        message_id: Uuid,
        server_ids: &[Uuid],
    ) -> Result<(), AppError> {
        if server_ids.is_empty() {
            return Ok(());
        }
        // Single multi-row INSERT via UNNEST — one round-trip even for
        // many servers; ON CONFLICT DO NOTHING in case the user
        // somehow re-saves the same message (defensive — shouldn't
        // happen on the create path).
        sqlx::query(
            r#"
            INSERT INTO message_mcp_servers (message_id, server_id)
            SELECT $1, UNNEST($2::uuid[])
            ON CONFLICT (message_id, server_id) DO NOTHING
            "#,
        )
        .bind(message_id)
        .bind(server_ids)
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    /// List the MCP servers that were enabled when this message was
    /// sent. Reverse of `insert_message_servers`. Used by the new
    /// `GET /api/messages/{id}/mcp-servers` endpoint that the
    /// frontend mcp extension hits on message Edit.
    ///
    /// Returns `Ok(None)` when the user doesn't own the conversation
    /// the message belongs to (caller maps to 404). Returns
    /// `Ok(Some(vec![]))` when the user owns it but no servers were
    /// recorded.
    pub async fn list_message_servers_for_user(
        &self,
        message_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<Vec<Uuid>>, AppError> {
        // Ownership check: walk message → branch_messages → branches
        //                  → conversations to confirm the caller owns
        // the conversation. MCP's bridge is allowed to read chat's
        // tables (sibling-module → chat direction is the allowed one;
        // it's chat→mcp that's forbidden).
        let owns: Option<bool> = sqlx::query_scalar!(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM messages m
                INNER JOIN branch_messages bm ON bm.message_id = m.id
                INNER JOIN branches b ON b.id = bm.branch_id
                INNER JOIN conversations c ON c.id = b.conversation_id
                WHERE m.id = $1 AND c.user_id = $2
            )
            "#,
            message_id,
            user_id,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        if !owns.unwrap_or(false) {
            return Ok(None);
        }

        let rows: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT server_id FROM message_mcp_servers WHERE message_id = $1",
        )
        .bind(message_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(Some(rows.into_iter().map(|(id,)| id).collect()))
    }
}
