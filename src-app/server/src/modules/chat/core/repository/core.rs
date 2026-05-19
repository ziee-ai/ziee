// Core chat repository operations

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::core::models::{Branch, Conversation, MessageContent, MessageContentData};
use crate::modules::chat::core::types::{ConversationResponse, EditMessageRequest, EditMessageResponse, MessageWithContent};

use super::{branches, contents, conversations, messages};

/// Repository for core chat database operations
#[derive(Clone, Debug)]
pub struct ChatCoreRepository {
    pool: PgPool,
}

impl ChatCoreRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ===== Conversation Operations =====

    /// Get a conversation by ID and verify ownership
    pub async fn get_conversation(
        &self,
        conversation_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<Conversation>, AppError> {
        conversations::get_conversation(&self.pool, conversation_id, user_id).await
    }

    /// Update conversation state (active model and branch)
    pub async fn update_conversation_state(
        &self,
        conversation_id: Uuid,
        user_id: Uuid,
        model_id: Uuid,
        branch_id: Option<Uuid>,
    ) -> Result<(), AppError> {
        conversations::update_conversation_state(
            &self.pool,
            conversation_id,
            user_id,
            model_id,
            branch_id,
        )
        .await
    }

    /// Create a new conversation
    pub async fn create_conversation(
        &self,
        user_id: Uuid,
        model_id: Option<Uuid>,
        title: Option<String>,
    ) -> Result<Conversation, AppError> {
        conversations::create_conversation(&self.pool, user_id, model_id, title).await
    }

    /// List conversations for a user
    pub async fn list_conversations(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ConversationResponse>, AppError> {
        conversations::list_conversations(&self.pool, user_id, limit, offset).await
    }

    /// Update a conversation
    pub async fn update_conversation(
        &self,
        id: Uuid,
        user_id: Uuid,
        title: Option<Option<String>>,
    ) -> Result<Option<Conversation>, AppError> {
        conversations::update_conversation(&self.pool, id, user_id, title).await
    }

    /// Delete a conversation
    pub async fn delete_conversation(&self, id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
        conversations::delete_conversation(&self.pool, id, user_id).await
    }

    // ===== Branch Operations =====

    /// Get a branch by ID
    pub async fn get_branch(&self, branch_id: Uuid) -> Result<Option<Branch>, AppError> {
        branches::get_branch(&self.pool, branch_id).await
    }

    /// Create a new branch
    pub async fn create_branch(
        &self,
        conversation_id: Uuid,
        parent_branch_id: Uuid,
        created_from_message_id: Uuid,
    ) -> Result<Branch, AppError> {
        branches::create_branch(
            &self.pool,
            conversation_id,
            parent_branch_id,
            created_from_message_id,
        )
        .await
    }

    /// List branches in a conversation
    pub async fn list_branches(&self, conversation_id: Uuid) -> Result<Vec<Branch>, AppError> {
        branches::list_branches(&self.pool, conversation_id).await
    }

    /// Set the active branch for a conversation
    pub async fn set_active_branch(
        &self,
        conversation_id: Uuid,
        branch_id: Uuid,
    ) -> Result<(), AppError> {
        branches::set_active_branch(&self.pool, conversation_id, branch_id).await
    }

    // ===== Message Operations =====

    /// Get a message with its content blocks
    pub async fn get_message_with_content(
        &self,
        message_id: Uuid,
    ) -> Result<Option<MessageWithContent>, AppError> {
        messages::get_message_with_content(&self.pool, message_id).await
    }

    /// Get conversation history for a branch
    pub async fn get_conversation_history(
        &self,
        branch_id: Uuid,
    ) -> Result<Vec<MessageWithContent>, AppError> {
        messages::get_conversation_history(&self.pool, branch_id).await
    }

    /// Verify that a message exists and user owns the conversation containing it
    pub async fn verify_message_ownership(
        &self,
        message_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<Conversation>, AppError> {
        messages::verify_message_ownership(&self.pool, message_id, user_id).await
    }

    /// Edit a message
    pub async fn edit_message(
        &self,
        message_id: Uuid,
        conversation_id: Uuid,
        request: EditMessageRequest,
        current_branch_id: Uuid,
    ) -> Result<EditMessageResponse, AppError> {
        messages::edit_message(&self.pool, message_id, conversation_id, request, current_branch_id)
            .await
    }

    /// Delete a message and its descendants
    pub async fn delete_message_and_descendants(
        &self,
        message_id: Uuid,
    ) -> Result<u64, AppError> {
        messages::delete_message_and_descendants(&self.pool, message_id).await
    }

    /// Create a new branch from a specific message
    pub async fn create_branch_from_message(
        &self,
        conversation_id: Uuid,
        parent_branch_id: Uuid,
        message_id: Uuid,
    ) -> Result<Branch, AppError> {
        messages::create_branch_from_message(
            &self.pool,
            conversation_id,
            parent_branch_id,
            message_id,
        )
        .await
    }

    /// Create a new message with optional context (model, assistant, MCP servers used)
    pub async fn create_message(
        &self,
        branch_id: Uuid,
        role: &str,
        model_id: Option<Uuid>,
        assistant_id: Option<Uuid>,
        mcp_server_ids: Option<Vec<Uuid>>,
    ) -> Result<crate::modules::chat::core::models::Message, AppError> {
        messages::create_message(&self.pool, branch_id, role, model_id, assistant_id, mcp_server_ids).await
    }

    /// Get a message by ID
    pub async fn get_message(
        &self,
        message_id: Uuid,
    ) -> Result<Option<crate::modules::chat::core::models::Message>, AppError> {
        messages::get_message(&self.pool, message_id).await
    }

    // ===== Content Operations =====

    /// Create content for a message
    pub async fn create_content(
        &self,
        message_id: Uuid,
        content_type: &str,
        data: MessageContentData,
        index: i32,
    ) -> Result<MessageContent, AppError> {
        contents::create_content(&self.pool, message_id, content_type, data, index).await
    }
}
