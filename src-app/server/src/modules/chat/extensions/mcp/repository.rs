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
        repository::create_tool_approval(
            &self.pool,
            conversation_id,
            branch_id,
            message_id,
            user_id,
            tool_use_id,
            tool_name,
            tool_input,
            server_id,
            server_name,
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
}
