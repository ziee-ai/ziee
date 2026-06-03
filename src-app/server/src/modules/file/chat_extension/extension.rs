//! Chat-extension registration for the file module.
//!
//! Self-registers via linkme — chat picks it up at link time from
//! the `CHAT_EXTENSIONS` distributed slice. No central registry to
//! update. Moved here from `chat/extensions/file/extension.rs` as
//! part of the bridge extraction (chat knows nothing about files).

use linkme::distributed_slice;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::modules::chat::core::extension::{
    CHAT_EXTENSIONS, ChatExtension, ExtensionEntry, ExtensionMetadata,
};

/// Metadata for the file extension
pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "file",
    order: 20, // After assistant (10), before title (80)
};

/// Request fields that will be auto-merged into SendMessageRequest by the macro system
/// Note: Not directly constructed - used by compose_send_message_request macro
#[allow(dead_code)]
#[derive(Debug, serde::Deserialize, schemars::JsonSchema, Default)]
pub struct SendMessageRequestFields {
    /// File IDs to attach to this message
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Array of file IDs to attach to the message")]
    pub file_ids: Option<Vec<Uuid>>,
}

/// MessageContentData variants contributed by file extension
/// These will be auto-merged into MessageContentData by the composition
/// macro. Paths must be fully-qualified (`crate::...`) because the
/// macro copies the tokens verbatim into `modules/chat/core/models/content.rs`
/// where `super::` wouldn't resolve to file's types.
#[allow(dead_code)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum MessageContentDataVariants {
    /// Image content with source
    Image {
        source: crate::modules::file::chat_extension::types::ImageSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        alt_text: Option<String>,
    },

    /// File attachment content
    FileAttachment {
        file_id: Uuid,
        filename: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        file_size: i64,
    },
}

/// Extension factory function
pub fn create(pool: PgPool, _config: Arc<crate::core::config::Config>) -> Arc<dyn ChatExtension> {
    Arc::new(super::file::FileExtension::new(pool))
}

/// Self-registration via distributed slice
#[distributed_slice(CHAT_EXTENSIONS)]
static FILE_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};
