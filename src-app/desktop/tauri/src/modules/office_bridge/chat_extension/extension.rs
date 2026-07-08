//! office_bridge extension registration for the chat module.
//!
//! Desktop-only: registered at boot via `ziee::chat_extension::register_chat_extension`
//! (the runtime seam) from `office_bridge::register_office_bridge`, NOT via a
//! `#[distributed_slice(CHAT_EXTENSIONS)]` static — so a standalone `ziee` server,
//! which never calls the seam, gets no office_bridge chat extension.

use sqlx::PgPool;
use std::sync::Arc;

use ziee::chat_extension::{ChatExtension, ExtensionEntry, ExtensionMetadata};

pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "office_bridge",
    // MUST run BEFORE the MCP extension (order 30): `before_llm_call` sets the
    // `attach_office_bridge_mcp` flag that `auto_attach_builtin_ids` reads when
    // MCP builds the tool list. 23 is a FREE slot (control_mcp=22, summarization=24,
    // memory=25, web_search=26, bio_mcp=27, lit_search=28, citations=29) so it
    // collides with no other extension's order, and it runs before MCP (30). The
    // relative order among these attach-flag extensions is otherwise irrelevant —
    // they each set an independent metadata flag the MCP collector reads. If this
    // ran at ≥30 the flag would be set after MCP already built its tools and the
    // office tools would never attach.
    order: 23,
};

pub fn create(pool: PgPool, config: Arc<ziee::Config>) -> Arc<dyn ChatExtension> {
    // Deploy-level kill switch — ON by default (an absent `office_bridge:` config
    // section means enabled), mirroring `office_bridge::register_office_bridge`. When
    // off, the extension must never attach even if a stale enabled row survives from
    // a prior boot.
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

/// The chat-extension registry entry, registered at boot via the runtime seam.
pub fn extension_entry() -> ExtensionEntry {
    ExtensionEntry {
        name: METADATA.name,
        order: METADATA.order,
        factory: create,
    }
}
