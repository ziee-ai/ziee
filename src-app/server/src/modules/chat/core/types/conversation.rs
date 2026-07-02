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

/// Request to create a new conversation.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateConversationRequest {
    /// Optional model ID for display/history purposes
    /// Actual model selection happens per-message via SendMessageRequest
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Request to update conversation metadata.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateConversationRequest {
    /// Title update: None = don't update, Some(None) = clear to null, Some(Some(value)) = set value
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_nullable_field"
    )]
    pub title: Option<Option<String>>,
}

/// Custom deserializer to distinguish between missing field and explicit null
fn deserialize_nullable_field<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    use serde::Deserialize;

    // This deserializes to Option<T>:
    // - Missing field -> Ok(None)
    // - "field": null -> Ok(Some(None))
    // - "field": value -> Ok(Some(Some(value)))
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}

/// Conversation response with additional metadata
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ConversationResponse {
    #[serde(flatten)]
    pub conversation: Conversation,
    pub message_count: i64,
}

/// Paginated list of conversations. `total` is the full server-side count
/// (not the page size), so the client can render "Showing N of TOTAL" and
/// gate its Load-More affordance. Mirrors `ProjectListResponse`.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ConversationListResponse {
    pub conversations: Vec<ConversationResponse>,
    pub total: i64,
}
