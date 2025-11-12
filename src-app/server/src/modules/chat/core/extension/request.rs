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

    /// Optional parent message ID (for threading)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<Uuid>,

    /// Model name to use for the chat request (passed from handler)
    #[serde(skip)]
    pub model_name: Option<String>,
}
