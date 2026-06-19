//! Built-in MCP server for live literature search & screening.
//!
//! Registers `lit_search.ziee.internal` as a row in `mcp_servers`
//! (`is_built_in=true`, `transport_type='http'`), pointing at a loopback URL on
//! the same axum app, and serves JSON-RPC at `/api/lit-search/mcp`. The MCP
//! client at `mcp/client/manager.rs` injects the JWT for built-in servers.
//!
//! Two tools:
//! - `literature_search(query, …)`     — UNION across Europe PMC / Crossref /
//!   Semantic Scholar / PubMed / arXiv / CORE → dedup → rank → digest.
//! - `fetch_paper_fulltext(ids, …)`    — open-access full text, cached to disk +
//!   mounted read-only at `/lit` in the sandbox.
//!
//! The server row is registered at boot unless the deploy-level
//! `lit_search.enabled=false` kill switch is set; the chat extension then
//! ATTACHES the tools only when the runtime `lit_search_settings.enabled` toggle
//! is on (the sole attach gate — keyless sources work without config; CORE
//! self-skips when enabled-but-unkeyed, so a "CORE-only, no key" config still
//! attaches the tool and returns an empty result with CORE in `degraded_sources`).
//! Tools reach the model via the `auto_attach_builtin_ids` branch in
//! `mcp/chat_extension`.

use std::error::Error;
use std::sync::Arc;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use uuid::Uuid;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

pub mod chat_extension;
pub mod completeness;
pub mod connectors;
pub mod dedup;
pub mod fulltext;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod ranking;
pub mod repository;
pub mod routes;
pub mod tools;

pub use repository::LitSearchRepository;

/// Deterministic UUID for the built-in lit_search MCP server row.
/// Stable across deployments (mirrors `web_search_server_id`).
pub fn lit_search_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"lit_search.ziee.internal")
}

#[distributed_slice(MODULE_ENTRIES)]
static LIT_SEARCH_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "lit_search",
    // ModuleEntry order (NOT a migration number): after mcp (order 65) so the
    // mcp_servers table exists for the built-in upsert; after web_search (order 96).
    order: 100,
    description: "Built-in MCP server: literature search (literature_search) + OA full text (fetch_paper_fulltext)",
    constructor: || Box::new(LitSearchModule::new()),
};

pub struct LitSearchModule {
    #[allow(dead_code)]
    pool: Option<Arc<PgPool>>,
}

impl LitSearchModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl Default for LitSearchModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for LitSearchModule {
    fn name(&self) -> &'static str {
        "lit_search"
    }

    fn description(&self) -> &'static str {
        "Built-in MCP server: literature search + open-access full text"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // Deploy-level kill switch — ON by default (an absent `lit_search:`
        // config section means enabled). IP-sensitive operators opt OUT with
        // `lit_search: { enabled: false }` so query terms never egress; an
        // admin cannot re-enable it (distinct from the runtime
        // `lit_search_settings.enabled` toggle).
        let enabled = ctx
            .config
            .lit_search
            .as_ref()
            .map(|c| c.enabled)
            .unwrap_or(true);
        if !enabled {
            tracing::info!("lit_search: disabled in config; skipping registration");
            return Ok(());
        }

        // Pin loopback regardless of the configured server host (same helper the
        // other built-in MCP servers use) so the URL can't be redirected.
        let host = crate::modules::code_sandbox::loopback_host(&ctx.config.server.host);
        let loopback_url = format!(
            "http://{host}:{port}/api/lit-search/mcp",
            port = ctx.config.server.port,
        );

        let server_id = lit_search_server_id();
        let pool = ctx.db_pool.clone();
        tokio::spawn(async move {
            let repo = LitSearchRepository::new((*pool).clone());
            match repo.upsert_builtin_server(server_id, &loopback_url).await {
                Ok(()) => tracing::info!(
                    "lit_search: built-in server {server_id} registered at {loopback_url}"
                ),
                Err(e) => tracing::error!("lit_search: upsert_builtin_server failed: {e:?}"),
            }
        });

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::lit_search_router())
    }
}
