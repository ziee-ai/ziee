//! Summarization extension registration for the chat module.

use linkme::distributed_slice;
use sqlx::PgPool;
use std::sync::Arc;

use crate::modules::chat::core::extension::{
    CHAT_EXTENSIONS, ChatExtension, ExtensionEntry, ExtensionMetadata,
};

pub const METADATA: ExtensionMetadata = ExtensionMetadata {
    name: "summarization",
    // 24 — load-bearing: runs BEFORE the memory extension (order 25)
    // so the summary block is in place by the time memory's hooks fire.
    // Memory's `inject_core_memory_blocks` inserts at position 0 (and
    // its vector-retrieval block appends to the latest user message);
    // running memory FIRST would shift the indices summarization counts
    // when pruning the condensed prefix, so the order is invariant —
    // not a "summary block sits above memory" claim, which the actual
    // final layout doesn't honour anyway.
    order: 24,
};

// Reserved extension-contract placeholder: every chat extension declares the
// request fields it contributes (empty here; see mcp/file for populated versions).
#[allow(dead_code)]
#[derive(Debug, serde::Deserialize, schemars::JsonSchema, Default)]
pub struct SendMessageRequestFields {}

#[allow(dead_code)] // reserved extension-contract placeholder; no summarization SSE events today
pub enum SSEChatStreamEventVariants {}

pub fn create(pool: PgPool, _config: Arc<crate::core::config::Config>) -> Arc<dyn ChatExtension> {
    Arc::new(super::summarization::SummarizationExtension::new(pool))
}

#[distributed_slice(CHAT_EXTENSIONS)]
static SUMMARIZATION_EXTENSION: ExtensionEntry = ExtensionEntry {
    name: METADATA.name,
    order: METADATA.order,
    factory: create,
};

#[cfg(test)]
mod tests {
    use super::METADATA as SUMMARIZATION;
    use crate::modules::memory::chat_extension::extension::METADATA as MEMORY;

    /// Load-bearing invariant: the summarization extension MUST run strictly
    /// before the memory extension. Memory inserts core-memory blocks at
    /// position 0 / appends a retrieval block; if memory ran first it would
    /// shift the message indices summarization counts when pruning the
    /// condensed prefix. Guard the two `order` constants so a future re-number
    /// of either can't silently invert them.
    #[test]
    fn summarization_runs_before_memory() {
        assert!(
            SUMMARIZATION.order < MEMORY.order,
            "summarization (order {}) must run before memory (order {})",
            SUMMARIZATION.order,
            MEMORY.order
        );
    }
}
