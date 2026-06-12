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
    // so the summary block lands first; memory's retrieval block is
    // appended to compacted history. Reordering would let memory
    // inject before compaction, breaking the [System*, SummaryBlock,
    // MemoryBlock, RecentTurns] assembly invariant.
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
