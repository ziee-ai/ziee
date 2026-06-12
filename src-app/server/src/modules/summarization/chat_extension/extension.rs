//! Summarization extension registration for the chat module.
#![allow(dead_code)]

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

#[derive(Debug, serde::Deserialize, schemars::JsonSchema, Default)]
pub struct SendMessageRequestFields {}

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
