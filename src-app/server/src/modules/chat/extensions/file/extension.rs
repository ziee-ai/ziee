use linkme::distributed_slice;
use crate::modules::chat::core::extension::{
    ChatExtension, ExtensionEntry, ExtensionMetadata, CHAT_EXTENSIONS,
};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

/// Metadata for the file extension
pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "file",
    order: 20, // After assistant (10), before title (80)
};

/// Request fields that will be auto-merged into SendMessageRequest by the macro system
#[derive(Debug, serde::Deserialize, schemars::JsonSchema, Default)]
pub struct SendMessageRequestFields {
    /// File IDs to attach to this message
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Array of file IDs to attach to the message")]
    pub file_ids: Option<Vec<Uuid>>,
}

/// Extension factory function
pub fn create(pool: PgPool) -> Arc<dyn ChatExtension> {
    Arc::new(super::file_extension::FileExtension::new(pool))
}

/// Self-registration via distributed slice
#[distributed_slice(CHAT_EXTENSIONS)]
static FILE_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};
