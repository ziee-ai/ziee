//! Memory extension registration for the chat module.
#![allow(dead_code)]

use linkme::distributed_slice;
use sqlx::PgPool;
use std::sync::Arc;

use crate::modules::chat::core::extension::{
    CHAT_EXTENSIONS, ChatExtension, ExtensionEntry, ExtensionMetadata,
};

pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "memory",
    // MUST run BEFORE the MCP extension (order 30): `before_llm_call` sets the
    // `attach_memory_mcp` metadata flag for inline self-save, and the MCP
    // extension reads it (`auto_attach_builtin_ids`) when building the tool list.
    // At the old order 90 the flag was set AFTER MCP had already built its tools,
    // so the `remember` tool was never attached and inline self-save never fired.
    // 25 lands it after assistant (10) / file (20) — so the retrieval + summary
    // system blocks still sit after the primary system prompt — but before MCP
    // (30). Retrieval/summary are order-independent w.r.t. other extensions (they
    // act on persisted branch history, not request-assembly order); only the
    // injected system-block position shifts slightly. `after_llm_call`
    // (extraction + summary refresh) is order-independent.
    order: 25,
};

/// Request fields contributed by the memory extension. Phase 5 adds a
/// per-conversation `memory_mode` override here.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema, Default)]
pub struct SendMessageRequestFields {}

/// SSE event variants contributed by memory. None today; reserved.
pub enum SSEChatStreamEventVariants {}

pub fn create(pool: PgPool, _config: Arc<crate::core::config::Config>) -> Arc<dyn ChatExtension> {
    Arc::new(super::memory::MemoryExtension::new(pool))
}

#[distributed_slice(CHAT_EXTENSIONS)]
static MEMORY_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};
