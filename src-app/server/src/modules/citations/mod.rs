//! Built-in MCP server for citation management + verification.
//!
//! Registers `citations.ziee.internal` as a row in `mcp_servers`
//! (`is_built_in=true`, `transport_type='http'`), pointing at a loopback URL on
//! the same axum app, serving JSON-RPC at `/api/citations/mcp`.
//!
//! Capability: a persistent, verified, CSL-JSON bibliography (user library +
//! per-project reference lists). The novel piece is **verification** — every
//! DOI/PMID must resolve to a real record ("never invent a citation"). Resolve
//! is self-contained (doi.org content-negotiation + NCBI ID-Converter +
//! Crossref title search), reusing only `lit_search::dedup::normalize_doi`;
//! formatting reuses the embedded pandoc (citeproc).
//!
//! Tools (batch-first): lookup_citations / add_citations / verify_citations /
//! list_citations / format_citations / remove_citations.

use std::error::Error;
use std::sync::Arc;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use uuid::Uuid;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

pub mod chat_extension;
pub mod csl;
pub mod dedup;
pub mod format;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod resolve;
pub mod rest;
pub mod routes;
pub mod tools;
pub mod verify;

/// Deterministic UUID for the built-in citations MCP server row.
/// Stable across deployments (mirrors `web_search_server_id`).
pub fn citations_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"citations.ziee.internal")
}

#[distributed_slice(MODULE_ENTRIES)]
static CITATIONS_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "citations",
    // After mcp (65) so mcp_servers exists; after lit_search (100) /
    // tool_result_mcp (102) whose code this module reuses.
    order: 103,
    description: "Built-in MCP server for citation management + verification",
    constructor: || Box::new(CitationsModule::new()),
};

pub struct CitationsModule {
    #[allow(dead_code)]
    pool: Option<Arc<PgPool>>,
}

impl CitationsModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl Default for CitationsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for CitationsModule {
    fn name(&self) -> &'static str {
        "citations"
    }

    fn description(&self) -> &'static str {
        "Built-in MCP server for citation management + verification"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // Pin loopback regardless of the configured host (same helper
        // web_search/memory_mcp use) so the built-in MCP URL can never be
        // redirected to a non-loopback host.
        let host = crate::modules::code_sandbox::loopback_host(&ctx.config.server.host);
        let loopback_url = format!(
            "http://{host}:{port}/api/citations/mcp",
            port = ctx.config.server.port,
        );

        let server_id = citations_server_id();
        let pool = ctx.db_pool.clone();
        tokio::spawn(async move {
            let repo = repository::CitationsRepository::new((*pool).clone());
            match repo.upsert_builtin_server(server_id, &loopback_url).await {
                Ok(()) => tracing::info!(
                    "citations: built-in server {server_id} registered at {loopback_url}"
                ),
                Err(e) => tracing::error!("citations: upsert_builtin_server failed: {e:?}"),
            }
        });

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::citations_router())
    }
}
