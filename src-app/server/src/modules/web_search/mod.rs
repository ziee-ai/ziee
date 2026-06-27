//! Built-in MCP server for web search + page fetch.
//!
//! Registers `web_search.ziee.internal` as a row in `mcp_servers`
//! (`is_built_in=true`, `transport_type='http'`), pointing at a loopback URL on
//! the same axum app, and serves JSON-RPC at `/api/web-search/mcp`. The MCP
//! client at `mcp/client/manager.rs` injects the JWT for built-in servers.
//!
//! Two tools:
//! - `web_search(query, max_results?)` — query the configured provider chain.
//! - `fetch_url(url)`                  — fetch a page → clean markdown.
//!
//! The server row is ALWAYS registered; the chat extension only ATTACHES the
//! tools to a request when web search is enabled and ≥1 provider in the chain
//! is configured (see `chat_extension`). Tools reach the model only via the
//! `auto_attach_builtin_ids` branch in `mcp/chat_extension/mcp.rs`.

use std::error::Error;
use std::sync::Arc;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use uuid::Uuid;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

pub mod chat_extension;
pub mod fetch;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod providers;
pub mod repository;
pub mod routes;
pub mod tools;

pub use repository::WebSearchRepository;

/// Deterministic UUID for the built-in web_search MCP server row.
/// Stable across deployments (mirrors `memory_mcp_server_id`).
pub fn web_search_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"web_search.ziee.internal")
}

#[distributed_slice(MODULE_ENTRIES)]
static WEB_SEARCH_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "web_search",
    // After mcp (65) so mcp_servers exists. 96 is the next free order
    // (app=90, server_update=92, sync=95).
    order: 96,
    description: "Built-in MCP server exposing web search + page fetch (web_search / fetch_url)",
    constructor: || Box::new(WebSearchModule::new()),
};

pub struct WebSearchModule {
    #[allow(dead_code)]
    pool: Option<Arc<PgPool>>,
}

impl WebSearchModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl Default for WebSearchModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for WebSearchModule {
    fn name(&self) -> &'static str {
        "web_search"
    }

    fn description(&self) -> &'static str {
        "Built-in MCP server exposing web search + page fetch"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // Deploy-level kill switch — ON by default (an absent `web_search:`
        // config section means enabled). IP-sensitive operators opt OUT with
        // `web_search: { enabled: false }` so query terms never egress; an
        // admin cannot re-enable it (distinct from the runtime
        // `web_search_settings.enabled` toggle). Mirrors lit_search.
        let enabled = ctx
            .config
            .web_search
            .as_ref()
            .map(|c| c.enabled)
            .unwrap_or(true);
        if !enabled {
            tracing::info!("web_search: disabled in config; skipping registration");
            return Ok(());
        }

        // Pin loopback regardless of the configured server host (same helper
        // code_sandbox/memory_mcp use) so the built-in MCP URL can never be
        // redirected to a non-loopback host.
        let host = crate::modules::code_sandbox::loopback_host(&ctx.config.server.host);
        let loopback_url = format!(
            "http://{host}:{port}/api/web-search/mcp",
            port = ctx.config.server.port,
        );

        let server_id = web_search_server_id();
        let pool = ctx.db_pool.clone();
        tokio::spawn(async move {
            let repo = repository::WebSearchRepository::new((*pool).clone());
            match repo.upsert_builtin_server(server_id, &loopback_url).await {
                Ok(()) => tracing::info!(
                    "web_search: built-in server {server_id} registered at {loopback_url}"
                ),
                Err(e) => tracing::error!("web_search: upsert_builtin_server failed: {e:?}"),
            }
        });

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::web_search_router())
    }
}
