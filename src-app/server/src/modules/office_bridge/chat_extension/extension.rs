//! office_bridge extension registration for the chat module.

use linkme::distributed_slice;
use sqlx::PgPool;
use std::sync::Arc;

use crate::modules::chat::core::extension::{
    CHAT_EXTENSIONS, ChatExtension, ExtensionEntry, ExtensionMetadata,
};

pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "office_bridge",
    // MUST run BEFORE the MCP extension (order 30): `before_llm_call` sets the
    // `attach_office_bridge_mcp` flag that `auto_attach_builtin_ids` reads when
    // MCP builds the tool list. 29 lands it after assistant/file/memory/
    // web_search (26) / bio_mcp (27) / lit_search (28), before MCP (30) —
    // distinct from the other attach-flag extensions so the registration order
    // is deterministic. If it ran at ≥30 the flag would be set after MCP already
    // built its tools and the office tools would never attach.
    order: 29,
};

pub fn create(pool: PgPool, config: Arc<crate::core::config::Config>) -> Arc<dyn ChatExtension> {
    // Deploy-level kill switch — ON by default (an absent `office_bridge:` config
    // section means enabled), mirroring `office_bridge::mod::init`. When off, the
    // extension must never attach even if a stale enabled row survives from a
    // prior boot.
    let config_enabled = config
        .office_bridge
        .as_ref()
        .map(|c| c.enabled)
        .unwrap_or(true);
    Arc::new(super::office_bridge::OfficeBridgeExtension::new(
        pool,
        config_enabled,
    ))
}

#[distributed_slice(CHAT_EXTENSIONS)]
static OFFICE_BRIDGE_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};
