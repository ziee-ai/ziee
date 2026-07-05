//! lit_search extension registration for the chat module.

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

pub fn create(pool: PgPool, config: Arc<crate::core::config::Config>) -> Arc<dyn ChatExtension> {
    // Deploy-level kill switch — ON by default (an absent `lit_search:` config
    // section means enabled), mirroring `lit_search::mod::init`. When off, the
    // extension must never attach even if a stale enabled row survives from a
    // prior boot.
    let config_enabled = config
        .lit_search
        .as_ref()
        .map(|c| c.enabled)
        .unwrap_or(true);
    Arc::new(super::lit_search::LitSearchExtension::new(
        pool,
        config_enabled,
    ))
}

#[distributed_slice(CHAT_EXTENSIONS)]
static LIT_SEARCH_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};
