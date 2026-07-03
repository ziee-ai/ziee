// Chat type infrastructure

// Message API request/response types

use serde::{Deserialize, Serialize};

use crate::modules::chat::core::models::{Branch, Message, MessageContent, MessageRole};

/// Message with its content blocks
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MessageWithContent {
    #[serde(flatten)]
    pub message: Message,
    pub contents: Vec<MessageContent>,
}

/// Request to create a new message (for system messages or manual creation)
// Request DTO not yet wired to a handler; kept as part of the message type
// surface (system/manual message creation) rather than deleted.
#[allow(dead_code)]
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateMessageRequest {
    pub role: MessageRole,
}

/// Request to edit an existing message
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditMessageRequest {
    pub content: String,
}

/// Response when editing a message (creates new branch)
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct EditMessageResponse {
    pub message: Message,
    pub branch: Branch,
}
