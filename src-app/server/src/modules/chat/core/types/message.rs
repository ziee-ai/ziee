// Chat type infrastructure

// Message API request/response types

use serde::{Deserialize, Serialize};

use crate::modules::chat::core::models::{Branch, Message, MessageContent};

/// Message with its content blocks
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MessageWithContent {
    #[serde(flatten)]
    pub message: Message,
    pub contents: Vec<MessageContent>,
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
