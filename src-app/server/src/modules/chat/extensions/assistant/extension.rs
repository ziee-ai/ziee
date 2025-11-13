// Extension implementation
#![allow(dead_code)]

// Assistant extension types for chat module
//
// This extension is registered using linkme distributed slices

use linkme::distributed_slice;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::modules::chat::core::extension::{
    CHAT_EXTENSIONS, ChatExtension, ExtensionEntry, ExtensionMetadata,
};

/// Extension metadata - defines name and execution order
pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "assistant",
    order: 10, // Execute early to inject system messages before other extensions
};

/// Request fields contributed by the assistant extension
#[derive(Debug, serde::Deserialize, schemars::JsonSchema, Default)]
pub struct SendMessageRequestFields {
    /// Optional assistant ID to use for this message.
    /// The assistant's instructions will be injected as a system message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assistant_id: Option<Uuid>,
}

/// Factory function to create the extension instance
/// Called by the auto-registration system
pub fn create(pool: PgPool) -> Arc<dyn ChatExtension> {
    Arc::new(super::assistant::AssistantExtension::new(pool))
}

/// Register this extension with the distributed slice
#[distributed_slice(CHAT_EXTENSIONS)]
static ASSISTANT_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};
