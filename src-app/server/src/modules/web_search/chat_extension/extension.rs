//! web_search extension registration for the chat module.
#![allow(dead_code)]

use linkme::distributed_slice;
use sqlx::PgPool;
use std::sync::Arc;

use crate::modules::chat::core::extension::{
    CHAT_EXTENSIONS, ChatExtension, ExtensionEntry, ExtensionMetadata,
};

pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "web_search",
    // MUST run BEFORE the MCP extension (order 30): `before_llm_call` sets the
    // `attach_web_search_mcp` metadata flag, which the MCP extension reads in
    // `auto_attach_builtin_ids` when building the tool list. 26 lands it after
    // assistant (10) / file (20) / memory (25), before MCP (30). If it ran at
    // ≥30 the flag would be set after MCP already built its tools and the
    // web_search tools would never attach.
    order: 26,
};

pub fn create(pool: PgPool, _config: Arc<crate::core::config::Config>) -> Arc<dyn ChatExtension> {
    Arc::new(super::web_search::WebSearchExtension::new(pool))
}

#[distributed_slice(CHAT_EXTENSIONS)]
static WEB_SEARCH_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};
