//! User memory module — per-user fact store with vector retrieval.
//!
//! The Memory feature has three surfaces:
//!   - REST CRUD at `/api/memories` (this module) for the Memories page.
//!   - Chat extension bridge (`chat_extension/`) — silent
//!     extract/retrieve pipeline. Self-registers with chat via linkme.
//!   - Built-in MCP server (`modules/memory_mcp`) exposing
//!     `remember`/`recall`/`forget` tools.
//!
//! Shared dual-use engine (`engine/`) holds extract/summarize/dispatch/
//! prompts — called from the bridge hooks AND directly from
//! `handlers.rs`, `embedding_worker.rs`, and `memory_mcp/handlers.rs`.
//!
//! All three surfaces share the same Postgres tables (`user_memories`,
//! `user_memory_settings`) defined by migration 46.

pub mod chat_extension;
pub mod embedding_worker;
pub mod engine;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod reaper;
pub mod repository;
pub mod routes;

pub use repository::MemoryRepository;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use std::error::Error;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

#[distributed_slice(MODULE_ENTRIES)]
static MEMORY_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "memory",
    // After users / llm_model / mcp so all referenced tables exist.
    order: 80,
    description: "Per-user memory store (Memories REST API)",
    constructor: || Box::new(MemoryModule::new()),
};

pub struct MemoryModule;

impl MemoryModule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MemoryModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for MemoryModule {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn description(&self) -> &'static str {
        "Per-user memory store"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        // Spawn the retention reaper (24h tick). Best-effort: a panic
        // would just stop reaping; chat continues to work.
        let pool = (*ctx.db_pool).clone();
        tokio::spawn(async move { reaper::run_reaper_loop(pool).await });
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::memory_router())
    }
}
