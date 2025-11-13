// Message models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::content::MessageContent;

/// Message role in conversation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl MessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::System => "system",
        }
    }
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for MessageRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(Self::User),
            "assistant" => Ok(Self::Assistant),
            "system" => Ok(Self::System),
            _ => Err(format!("Invalid message role: {}", s)),
        }
    }
}

/// Message entity - Represents a single message in a conversation
/// Messages belong to branches via the branch_messages junction table
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Message {
    pub id: Uuid,
    pub role: String,
    pub originated_from_id: Uuid,  // Original message ID in edit lineage
    pub edit_count: i32,  // Number of edits in this lineage
    pub created_at: DateTime<Utc>,
}

impl Message {
    /// Get the role as an enum
    pub fn role_enum(&self) -> Result<MessageRole, String> {
        self.role.parse()
    }
}

/// Message with its content blocks
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MessageWithContent {
    #[serde(flatten)]
    pub message: Message,
    pub contents: Vec<MessageContent>,
}

/// Request to create a new message (for system messages or manual creation)
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateMessageRequest {
    pub role: MessageRole,
}

/// Request to edit an existing message
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditMessageRequest {
    pub content: String,
}

/// Branch-Message junction table entity
/// Represents the many-to-many relationship between branches and messages
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BranchMessage {
    pub id: Uuid,
    pub branch_id: Uuid,
    pub message_id: Uuid,
    pub is_clone: bool,  // true if message was cloned from another branch
    pub created_at: DateTime<Utc>,
}

/// Response when editing a message (creates new branch)
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct EditMessageResponse {
    pub message: Message,
    pub branch: crate::modules::chat::core::models::Branch,
}
