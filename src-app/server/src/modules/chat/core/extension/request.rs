// Chat extension request infrastructure
#![allow(dead_code)]

// Request models for chat streaming
//
// This module defines SendMessageRequest using procedural macros for extension composition.
// Extensions are auto-discovered by scanning for extension.rs files in modules/chat/**/
use macros::compose_chat_extensions;
use uuid::Uuid;

/// Request to send a message in a conversation
///
/// Extensions contribute fields by creating an extension.rs file with a RequestFields struct.
/// The build script automatically discovers and registers these extensions.
#[compose_chat_extensions]
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SendMessageRequest {
    /// User message content (text)
    pub content: String,

    /// Model ID to use for this message (required)
    pub model_id: Uuid,

    /// Branch ID to send message to (required)
    /// This branch will be used as the parent when creating a new branch
    pub branch_id: Uuid,

    /// If set, creates a new branch from this message before sending
    /// The message must belong to the specified branch_id
    /// The new branch becomes the active branch
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_branch_from_message_id: Option<Uuid>,
}
