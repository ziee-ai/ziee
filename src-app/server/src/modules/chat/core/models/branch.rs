// Branch DB entity

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Branch entity - Represents a branch in conversation history (for edit/regenerate)
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Branch {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub parent_branch_id: Option<Uuid>,
    pub created_from_message_id: Option<Uuid>,
    /// Distinguishes edit ('user') from regenerate ('assistant') branches.
    /// Used by the frontend to anchor the branch navigator at the correct message after page reload.
    pub fork_level: String,
    pub created_at: DateTime<Utc>,
}
