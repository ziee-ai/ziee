//! citations extension registration for the chat module.
#![allow(dead_code)]

use linkme::distributed_slice;
use sqlx::PgPool;
use std::sync::Arc;

use crate::modules::chat::core::extension::{
    CHAT_EXTENSIONS, ChatExtension, ExtensionEntry, ExtensionMetadata,
};

pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "citations",
    // MUST run BEFORE the MCP extension (order 30): `before_llm_call` sets the
    // `attach_citations_mcp` flag that the MCP extension reads when building the
    // tool list. 29 lands it after web_search (26) / bio (27) / lit_search (28),
    // before MCP (30).
    order: 29,
};

pub fn create(pool: PgPool, _config: Arc<crate::core::config::Config>) -> Arc<dyn ChatExtension> {
    Arc::new(super::citations::CitationsExtension::new(pool))
}

#[distributed_slice(CHAT_EXTENSIONS)]
static CITATIONS_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};
