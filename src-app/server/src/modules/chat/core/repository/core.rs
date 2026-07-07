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

    /// Create a new conversation.
    pub async fn create_conversation(
        &self,
        user_id: Uuid,
        model_id: Option<Uuid>,
        title: Option<String>,
    ) -> Result<Conversation, AppError> {
        conversations::create_conversation(&self.pool, user_id, model_id, title).await
    }

    /// List the user's conversations with optional content `search` + `sort`
    /// (see `conversations::list_conversations`).
    pub async fn list_conversations(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
        search: Option<&str>,
        sort: Option<&str>,
    ) -> Result<Vec<ConversationResponse>, AppError> {
        conversations::list_conversations(&self.pool, user_id, limit, offset, search, sort).await
    }

    /// Count the user's conversations (server-side total, honoring the same
    /// optional content `search` filter so paginated totals stay consistent).
    pub async fn count_conversations(
        &self,
        user_id: Uuid,
        search: Option<&str>,
    ) -> Result<i64, AppError> {
        conversations::count_conversations(&self.pool, user_id, search).await
    }

    /// Update a conversation's metadata (title only).
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
        fork_level: &str,
    ) -> Result<Branch, AppError> {
        branches::create_branch(
            &self.pool,
            conversation_id,
            parent_branch_id,
            created_from_message_id,
            fork_level,
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

    /// Delete a single message. See `messages::delete_message` for the
    /// rationale on the absence of descendant semantics (04-chat F-03).
    pub async fn delete_message(&self, message_id: Uuid) -> Result<u64, AppError> {
        messages::delete_message(&self.pool, message_id).await
    }

    /// Create a new branch from a specific message
    pub async fn create_branch_from_message(
        &self,
        conversation_id: Uuid,
        parent_branch_id: Uuid,
        message_id: Uuid,
        fork_level: &str,
    ) -> Result<Branch, AppError> {
        messages::create_branch_from_message(
            &self.pool,
            conversation_id,
            parent_branch_id,
            message_id,
            fork_level,
        )
        .await
    }

    /// Create a new message
    pub async fn create_message(
        &self,
        branch_id: Uuid,
        role: &str,
        model_id: Option<Uuid>,
    ) -> Result<crate::modules::chat::core::models::Message, AppError> {
        messages::create_message(&self.pool, branch_id, role, model_id).await
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

    /// Append content to a message, assigning the next `sequence_order` atomically
    /// (MAX+1 in the INSERT). Use this for plain appends instead of computing an
    /// index from a cached count, which can drift from the DB and collide.
    pub async fn append_content(
        &self,
        message_id: Uuid,
        content_type: &str,
        data: MessageContentData,
    ) -> Result<MessageContent, AppError> {
        contents::append_content(&self.pool, message_id, content_type, data).await
    }


    /// Append content with a pre-registered UUID, assigning the next
    /// `sequence_order` atomically (MAX+1). Id-preserving sibling of `append_content`.
    pub async fn append_content_with_id(
        &self,
        id: Uuid,
        message_id: Uuid,
        content_type: &str,
        data: MessageContentData,
    ) -> Result<MessageContent, AppError> {
        contents::append_content_with_id(&self.pool, id, message_id, content_type, data).await
    }

    /// Update the JSONB content of an existing content block (e.g. to update elicitation status)
    pub async fn update_content_json(
        &self,
        content_id: Uuid,
        new_content: serde_json::Value,
    ) -> Result<(), AppError> {
        contents::update_content_json(&self.pool, content_id, new_content).await
    }

    /// Cancel any pending elicitation_request content blocks for the given message.
    /// Called when the streaming task ends to mark stale pending rows as cancelled.
    pub async fn cancel_pending_elicitations(&self, message_id: Uuid) -> Result<(), AppError> {
        contents::cancel_pending_elicitations(&self.pool, message_id).await
    }
}
