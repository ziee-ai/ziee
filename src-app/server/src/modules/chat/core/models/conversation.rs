// Conversation models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::branch::Branch;

/// Conversation entity - Represents a chat conversation with an AI assistant
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Conversation {
    pub id: Uuid,
    pub user_id: Uuid,
    /// Optional model ID for display/history purposes
    /// Actual model selection happens per-message via SendMessageRequest
    pub model_id: Option<Uuid>,
    pub title: Option<String>,
    pub active_branch_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Conversation with its active branch
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ConversationWithBranch {
    #[serde(flatten)]
    pub conversation: Conversation,
    pub branch: Branch,
}

/// Request to create a new conversation
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateConversationRequest {
    /// Optional model ID for display/history purposes
    /// Actual model selection happens per-message via SendMessageRequest
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Request to update conversation metadata
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateConversationRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Conversation response with additional metadata
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ConversationResponse {
    #[serde(flatten)]
    pub conversation: Conversation,
    pub message_count: i64,
}
