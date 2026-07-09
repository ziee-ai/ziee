//! knowledge_base extension registration for the chat module.

use linkme::distributed_slice;
use sqlx::PgPool;
use std::sync::Arc;

use crate::modules::chat::core::extension::{
    CHAT_EXTENSIONS, ChatExtension, ExtensionEntry, ExtensionMetadata,
};

pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "knowledge_base",
    // MUST run BEFORE the MCP extension (order 30). 23 is free (24 collides with
    // summarization; 25 memory, 26 web_search, 27 bio, 28 lit_search, 29
    // citations).
    order: 23,
};

pub fn create(pool: PgPool, _config: Arc<crate::core::config::Config>) -> Arc<dyn ChatExtension> {
    Arc::new(super::knowledge_base::KnowledgeBaseExtension::new(pool))
}

#[distributed_slice(CHAT_EXTENSIONS)]
static KNOWLEDGE_BASE_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};
