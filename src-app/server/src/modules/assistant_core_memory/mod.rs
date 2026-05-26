//! Per-assistant always-in-context memory blocks (Letta-style).
//!
//! `assistant_core_memory` rows are uniquely keyed by
//! `(assistant_id, user_id, block_label)`. The memory chat extension's
//! retriever prepends a small system fragment containing all of the
//! caller's core-memory blocks for the assistant in use.
//!
//! Scaffolded in Phase 6; CRUD endpoints + a `read_for_chat` helper
//! that the retriever calls. Summarization (migration 49 — conversation_summaries)
//! is a sibling concept and lives in `summarizer.rs` once wired.

pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;

pub use repository::AssistantCoreMemoryRepository;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use std::error::Error;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

#[distributed_slice(MODULE_ENTRIES)]
static ASSISTANT_CORE_MEMORY_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "assistant_core_memory",
    // After assistant (which holds the FK target) and after memory (80).
    order: 82,
    description: "Per-assistant always-in-context memory blocks",
    constructor: || Box::new(AssistantCoreMemoryModule::new()),
};

pub struct AssistantCoreMemoryModule;

impl AssistantCoreMemoryModule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AssistantCoreMemoryModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for AssistantCoreMemoryModule {
    fn name(&self) -> &'static str {
        "assistant_core_memory"
    }

    fn description(&self) -> &'static str {
        "Per-assistant core memory blocks"
    }

    fn init(&mut self, _ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::assistant_core_memory_router())
    }
}
