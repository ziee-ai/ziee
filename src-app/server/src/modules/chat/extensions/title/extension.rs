// Extension implementation

// Title extension types for chat module
//
// This extension is registered using linkme distributed slices
// Title generation doesn't require any request parameters, so SendMessageRequestFields is empty.

use linkme::distributed_slice;
use sqlx::PgPool;
use std::sync::Arc;

use crate::modules::chat::core::extension::{
    CHAT_EXTENSIONS, ChatExtension, ExtensionEntry, ExtensionMetadata,
};

/// Extension metadata - defines name and execution order
pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "title",
    order: 80, // Execute late (post-processing) to generate title after message is complete
};

/// Request fields contributed by the title extension
///
/// Title generation happens after LLM response, so no request fields are needed.
// Reserved extension-contract placeholder: every chat extension declares the
// request fields it contributes (see mcp/file for populated versions). Empty +
// not yet aggregated, so keep it explicitly rather than deleting the convention.
#[allow(dead_code)]
#[derive(Debug, serde::Deserialize, schemars::JsonSchema, Default)]
pub struct SendMessageRequestFields {
    // No fields - title generation is automatic and doesn't need configuration
}

/// Data for the TitleUpdated SSE event
#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct SSEChatStreamTitleUpdatedData {
    /// The auto-generated title
    pub title: String,
}

/// SSE event variants contributed by the title extension
///
/// These variants will be composed into the main SSEChatStreamEvent enum
// Reserved extension-contract placeholder (the aggregation into the main
// SSEChatStreamEvent enum isn't wired yet); the payload type
// `SSEChatStreamTitleUpdatedData` above is the one actually emitted.
#[allow(dead_code)]
pub enum SSEChatStreamEventVariants {
    /// Title generation complete event
    TitleUpdated(SSEChatStreamTitleUpdatedData),
}

/// Factory function to create the extension instance
/// Called by the auto-registration system
pub fn create(pool: PgPool, _config: Arc<crate::core::config::Config>) -> Arc<dyn ChatExtension> {
    Arc::new(super::title::TitleGenerationExtension::new(pool))
}

/// Register this extension with the distributed slice
#[distributed_slice(CHAT_EXTENSIONS)]
static TITLE_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};
