//! Conversation summarization — rolling per-branch context compaction.
//!
//! Extracted from the memory module in migration 91. Three surfaces:
//!   - REST admin: `GET/PUT /api/summarization/settings` for the
//!     deployment-wide singleton row.
//!   - REST per-conversation: `GET/PUT /api/conversations/{id}/summarization-mode`
//!     for the `inherit`/`on`/`off` override, plus
//!     `GET /api/conversations/{id}/summary` for the in-thread marker.
//!   - Chat extension bridge (`chat_extension/`): silent before/after
//!     hooks that compact history and refresh the summary. Self-registers
//!     with chat via linkme at order 24 (BEFORE memory's order 25 — load-
//!     bearing so the summary block lands before memory's retrieval
//!     injection).
//!
//! Engine (`engine/summarizer.rs`) holds the pure decision logic +
//! prompt assembly + LLM call + summary persistence. Called from the
//! chat extension's before/after hooks AND from the debug-only
//! `/_test/summarization/refresh` test hook.

pub mod chat_extension;
pub mod engine;
pub mod events;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;

pub use repository::SummarizationRepository;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use std::error::Error;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

#[distributed_slice(MODULE_ENTRIES)]
static SUMMARIZATION_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "summarization",
    // After llm_model + conversations so referenced FKs exist. Same
    // ordering tier as memory (80).
    order: 80,
    description: "Conversation summarization (rolling per-branch context compaction)",
    constructor: || Box::new(SummarizationModule::new()),
};

pub struct SummarizationModule;

impl SummarizationModule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SummarizationModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for SummarizationModule {
    fn name(&self) -> &'static str {
        "summarization"
    }

    fn description(&self) -> &'static str {
        "Conversation summarization"
    }

    fn init(&mut self, _ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::summarization_router())
    }
}
