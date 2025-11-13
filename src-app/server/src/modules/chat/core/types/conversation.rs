// Chat type infrastructure
#![allow(dead_code)]

// Conversation API request/response types

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::modules::chat::core::models::{Branch, Conversation};

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
