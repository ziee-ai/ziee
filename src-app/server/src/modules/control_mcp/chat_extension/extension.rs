//! control extension registration for the chat module.
#![allow(dead_code)]

use std::sync::Arc;

use linkme::distributed_slice;
use sqlx::PgPool;

use crate::modules::chat::core::extension::{
    CHAT_EXTENSIONS, ChatExtension, ExtensionEntry, ExtensionMetadata,
};

pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "control",
    // MUST run BEFORE the MCP extension (order 30): `before_llm_call` sets the
    // `attach_control_mcp` metadata flag, which the MCP extension reads in
    // `auto_attach_builtin_ids` when building the tool list. Orders 24-29 are
    // taken (summarization/memory/web_search/bio/lit_search/citations); 22 is a
    // free slot that still precedes MCP (30).
    order: 22,
};

pub fn create(_pool: PgPool, config: Arc<crate::core::config::Config>) -> Arc<dyn ChatExtension> {
    // Deploy kill-switch: `control_mcp.enabled` (default true). Absent config
    // section means enabled.
    let enabled = config.control_mcp.as_ref().map(|c| c.enabled).unwrap_or(true);
    Arc::new(super::control::ControlExtension::new(enabled))
}

#[distributed_slice(CHAT_EXTENSIONS)]
static CONTROL_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};
