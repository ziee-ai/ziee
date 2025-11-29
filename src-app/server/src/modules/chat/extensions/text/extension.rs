use linkme::distributed_slice;
use crate::modules::chat::core::extension::{
    ChatExtension, ExtensionEntry, ExtensionMetadata, CHAT_EXTENSIONS,
};
use sqlx::PgPool;
use std::sync::Arc;

/// Metadata for the text extension
pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "text",
    order: 5, // Before file (20), before assistant (10), before title (80)
};

/// MessageContentData variants contributed by text extension
/// These will be auto-merged into MessageContentData by the composition macro
#[allow(dead_code)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum MessageContentDataVariants {
    /// Plain text content
    Text {
        text: String,
    },

    /// Thinking/reasoning content (Claude-style extended thinking)
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<crate::modules::chat::extensions::text::types::ThinkingMetadata>,
    },
}

/// Extension factory function
pub fn create(pool: PgPool) -> Arc<dyn ChatExtension> {
    Arc::new(super::text::TextExtension::new(pool))
}

/// Self-registration via distributed slice
#[distributed_slice(CHAT_EXTENSIONS)]
static TEXT_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};
