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
    // After title (80) — both fire-and-forget in after_llm_call. Order
    // matters mostly for before_llm_call: memory injects a retrieval
    // system block; we want it AFTER assistant (which sets the primary
    // system prompt) but BEFORE other content extensions.
    order: 90,
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
