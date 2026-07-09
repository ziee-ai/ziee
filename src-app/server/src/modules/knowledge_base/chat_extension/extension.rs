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

#[cfg(test)]
mod order_tests {
    use super::METADATA;

    // TEST-17 (ITEM-21): the KB chat extension runs at order 23 — BEFORE the MCP
    // collector (30) so its attach flag is seen, and clear of the neighboring
    // built-ins (24 summarization … 29 citations).
    #[test]
    fn extension_order_is_23() {
        assert_eq!(METADATA.order, 23);
    }
}
