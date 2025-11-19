// MCP extension repository wrapper for chat

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

use super::approval::models::{ApprovalMode, ConversationMcpSettings, ToolUseApproval};
use super::approval::repository;

/// Repository for MCP extension operations
#[derive(Clone, Debug)]
pub struct McpChatRepository {
    pool: PgPool,
}

impl McpChatRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ===== Helper Functions =====

    /// Normalize auto_approved_tools JSONB to canonical string format
    pub async fn normalize_auto_approved_tools(
        &self,
        auto_approved_tools: &serde_json::Value,
    ) -> Result<Vec<String>, AppError> {
        repository::normalize_auto_approved_tools(&self.pool, auto_approved_tools).await
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
    pub async fn upsert_conversation_settings(
        &self,
        conversation_id: Uuid,
        user_id: Uuid,
        approval_mode: ApprovalMode,
        auto_approved_tools: serde_json::Value,
    ) -> Result<ConversationMcpSettings, AppError> {
        repository::upsert_conversation_settings(
            &self.pool,
            conversation_id,
            user_id,
            approval_mode,
            auto_approved_tools,
        )
        .await
    }

    /// Delete MCP settings for a conversation
    pub async fn delete_conversation_settings(
        &self,
        conversation_id: Uuid,
    ) -> Result<bool, AppError> {
        repository::delete_conversation_settings(&self.pool, conversation_id).await
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

    /// Get pending approvals for a message
    pub async fn get_pending_approvals_for_message(
        &self,
        message_id: Uuid,
    ) -> Result<Vec<ToolUseApproval>, AppError> {
        repository::get_pending_approvals_for_message(&self.pool, message_id).await
    }

    /// Get all pending approvals for a branch
    pub async fn get_pending_approvals_for_branch(
        &self,
        branch_id: Uuid,
    ) -> Result<Vec<ToolUseApproval>, AppError> {
        repository::get_pending_approvals_for_branch(&self.pool, branch_id).await
    }

    /// Approve a tool use
    pub async fn approve_tool_use(
        &self,
        tool_use_id: String,
        message_id: Uuid,
        approved_by: Uuid,
        note: Option<String>,
    ) -> Result<ToolUseApproval, AppError> {
        repository::approve_tool_use(&self.pool, tool_use_id, message_id, approved_by, note).await
    }

    /// Deny a tool use
    pub async fn deny_tool_use(
        &self,
        tool_use_id: String,
        message_id: Uuid,
        approved_by: Uuid,
        note: Option<String>,
    ) -> Result<ToolUseApproval, AppError> {
        repository::deny_tool_use(&self.pool, tool_use_id, message_id, approved_by, note).await
    }

    /// Cancel all pending approvals for a branch
    pub async fn cancel_pending_approvals_for_branch(
        &self,
        branch_id: Uuid,
    ) -> Result<u64, AppError> {
        repository::cancel_pending_approvals_for_branch(&self.pool, branch_id).await
    }

    /// Batch approve multiple tool uses
    pub async fn batch_approve_tool_uses(
        &self,
        message_id: Uuid,
        tool_use_ids: Vec<String>,
        approved_by: Uuid,
    ) -> Result<Vec<ToolUseApproval>, AppError> {
        repository::batch_approve_tool_uses(&self.pool, message_id, tool_use_ids, approved_by)
            .await
    }

    /// Get approval by tool_use_id and message_id
    pub async fn get_tool_approval(
        &self,
        tool_use_id: String,
        message_id: Uuid,
    ) -> Result<Option<ToolUseApproval>, AppError> {
        repository::get_tool_approval(&self.pool, tool_use_id, message_id).await
    }
}
