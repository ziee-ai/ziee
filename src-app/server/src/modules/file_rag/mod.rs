//! Document RAG (`file_rag`) — semantic + full-text retrieval over the
//! extracted full text of project/conversation files.
//!
//! Surfaces:
//!   - Ingest (`ingest`) — background chunk + embed, triggered from the
//!     `file` module's four head-change sites.
//!   - Retrieval (`retrieval`) — the `files_mcp` `semantic_search` tool calls
//!     into here (vector ⊕ FTS hybrid with RRF, FTS-only fallback).
//!   - Admin (`handlers`/`routes`) — deployment settings + re-embed + backfill.
//!
//! Reuses the `memory` module's embedding dispatch
//! (`memory::engine::dispatch::embed_batch`) and the halfvec/HNSW + FTS
//! retrieval patterns. Default is ON (FTS from day one); the vector arm
//! activates once an admin configures an embedding model.

pub mod chunking;
pub mod embed_worker;
pub mod handlers;
pub mod ingest;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod retrieval;
pub mod routes;

pub use repository::FileRagRepository;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use std::error::Error;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

#[distributed_slice(MODULE_ENTRIES)]
static FILE_RAG_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "file_rag",
    // After file (31), memory (80), memory_mcp (85), files_mcp (86) so all
    // referenced tables exist and `memory::engine::dispatch` is reachable.
    order: 87,
    description: "Document RAG over project/conversation files (semantic_search)",
    constructor: || Box::new(FileRagModule::new()),
};

pub struct FileRagModule;

impl FileRagModule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileRagModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for FileRagModule {
    fn name(&self) -> &'static str {
        "file_rag"
    }

    fn description(&self) -> &'static str {
        "Document RAG over project/conversation files"
    }

    fn init(&mut self, _ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        // Bounded, idempotent boot-time backfill: index pre-existing files that
        // have extracted text but no chunks (only when enabled — `run_backfill`
        // self-gates). Wait for the repository factory to be initialized (module
        // init can race it; `run_backfill` would otherwise panic on
        // `Repos.*`), then a short grace so it doesn't compete with boot. Safe
        // to re-run every boot (self-heals failed indexing).
        tokio::spawn(async {
            for _ in 0..600 {
                if crate::core::is_repos_initialized() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            if !crate::core::is_repos_initialized() {
                tracing::warn!(
                    "file_rag: repositories not initialized after 60s; skipping boot backfill"
                );
                return;
            }
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            ingest::run_backfill().await;
        });
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::file_rag_router())
    }
}
