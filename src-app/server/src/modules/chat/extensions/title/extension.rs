// Title extension types for chat module
//
// This file is auto-discovered by the build script and registered with the chat system.
// Title generation doesn't require any request parameters, so SendMessageRequestFields is empty.

use sqlx::PgPool;
use std::sync::Arc;

use crate::modules::chat::core::extension::{ChatExtension, ExtensionMetadata};

/// Extension metadata - defines name and execution order
pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "title",
    order: 80, // Execute late (post-processing) to generate title after message is complete
};

/// Request fields contributed by the title extension
///
/// Title generation happens after LLM response, so no request fields are needed.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema, Default)]
pub struct SendMessageRequestFields {
    // No fields - title generation is automatic and doesn't need configuration
}

/// Response fields contributed by the title extension
///
/// Adds the auto-generated conversation title to the stream chunk
#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct ChatStreamChunkFields {
    /// Auto-generated title (when available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Factory function to create the extension instance
/// Called by the auto-registration system
pub fn create(pool: PgPool) -> Arc<dyn ChatExtension> {
    Arc::new(super::title::TitleGenerationExtension::new(pool))
}
