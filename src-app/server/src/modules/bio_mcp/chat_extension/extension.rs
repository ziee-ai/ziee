//! Chat-extension registration for the bio_mcp module.
//!
//! Self-registers via linkme — chat picks it up at link time from the
//! `CHAT_EXTENSIONS` distributed slice. No central registry to update.

use linkme::distributed_slice;
use sqlx::PgPool;
use std::sync::Arc;

use crate::modules::chat::core::extension::{
    CHAT_EXTENSIONS, ChatExtension, ExtensionEntry, ExtensionMetadata,
};

/// Metadata for the bio_mcp extension.
pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "bio_mcp",
    // After memory (25), BEFORE mcp (30) — the `attach_bio_mcp` flag must
    // be set before the mcp extension reads it in `auto_attach_builtin_ids`.
    order: 27,
};

/// Extension factory function.
pub fn create(pool: PgPool, config: Arc<crate::core::config::Config>) -> Arc<dyn ChatExtension> {
    // Deploy-level kill switch — ON by default (an absent `bio_mcp:` config
    // section means enabled), mirroring `bio_mcp::mod::init`. When off, the
    // extension must never attach even if a stale enabled row survives from a
    // prior boot.
    let config_enabled = config.bio_mcp.as_ref().map(|c| c.enabled).unwrap_or(true);
    Arc::new(super::bio::BioMcpExtension::new(pool, config_enabled))
}

#[distributed_slice(CHAT_EXTENSIONS)]
static BIO_MCP_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};
