// Conversation DB entity

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

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
    /// Optional project ID. NULL = "unfiled" conversation. When set, the
    /// chat/project extension injects the project's instructions + files
    /// + MCP defaults into every send. `ON DELETE SET NULL` on the FK —
    /// deleting the project preserves the conversation.
    pub project_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
