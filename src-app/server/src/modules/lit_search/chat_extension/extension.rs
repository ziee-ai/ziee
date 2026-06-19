//! lit_search extension registration for the chat module.
#![allow(dead_code)]

use linkme::distributed_slice;
use sqlx::PgPool;
use std::sync::Arc;

use crate::modules::chat::core::extension::{
    CHAT_EXTENSIONS, ChatExtension, ExtensionEntry, ExtensionMetadata,
};

pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "lit_search",
    // MUST run BEFORE the MCP extension (order 30): `before_llm_call` sets the
    // `attach_lit_search_mcp` flag that `auto_attach_builtin_ids` reads when MCP
    // builds the tool list. 28 lands it after assistant/file/memory/web_search
    // (26) and bio_mcp (27), before MCP (30) — distinct from bio_mcp's 27 so the
    // two attach-flag extensions have a deterministic registration order.
    order: 28,
};

pub fn create(pool: PgPool, _config: Arc<crate::core::config::Config>) -> Arc<dyn ChatExtension> {
    Arc::new(super::lit_search::LitSearchExtension::new(pool))
}

#[distributed_slice(CHAT_EXTENSIONS)]
static LIT_SEARCH_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};
