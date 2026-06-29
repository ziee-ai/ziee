//! Built-in MCP server for memory tools.
//!
//! Registers `memory.ziee.internal` as a regular row in `mcp_servers`
//! with `is_built_in=true` + `transport_type='http'`, points at a
//! loopback URL on the same axum app, and serves JSON-RPC at
//! `/api/memories/mcp`. The MCP client at `mcp/client/manager.rs`
//! already handles JWT injection for built-in servers — auth flows
//! through there.
//!
//! Three tools exposed to LLMs:
//! - `remember(content, kind?, importance?)`  — persist a memory.
//! - `recall(query, top_k?)`                  — vector-search own memories.
//! - `forget(memory_id)`                      — soft-delete one memory.

use std::error::Error;
use std::sync::Arc;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use uuid::Uuid;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

pub mod handlers;
pub mod repository;
pub mod routes;
pub mod tools;

/// Deterministic UUID for the built-in memory MCP server row.
/// Stable across deployments. Mirrors the `code_sandbox_server_id`
/// pattern.
pub fn memory_mcp_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"memory.ziee.internal")
}

/// Process-global loopback URL for the built-in memory MCP endpoint, captured at
/// module init. Memory is toggled at RUNTIME (DB-only `memory_admin_settings`,
/// unlike the deploy-level Config gates used by lit_search/bio_mcp), so the
/// admin-settings PUT handler must be able to register the built-in server row
/// AFTER startup — which means it needs this URL without a `ModuleContext`.
static MEMORY_MCP_LOOPBACK_URL: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Register (or re-enable) the built-in memory MCP server row so it auto-attaches
/// to tool-capable chats. Called both at startup (when memory is already enabled)
/// and from `update_admin_settings` when an admin enables memory at runtime.
/// Without this runtime path the server row never exists for a deployment that
/// flips memory on after boot, so `auto_attach_builtin_ids` finds nothing and the
/// `remember` tool is never offered.
pub async fn register_builtin_server(pool: &PgPool) -> Result<(), crate::common::AppError> {
    let Some(url) = MEMORY_MCP_LOOPBACK_URL.get() else {
        // init() hasn't run (e.g. memory_mcp module not loaded) — nothing to do.
        return Ok(());
    };
    let repo = repository::MemoryMcpRepository::new(pool.clone());
    repo.upsert_builtin_server(memory_mcp_server_id(), url).await
}

#[distributed_slice(MODULE_ENTRIES)]
static MEMORY_MCP_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "memory_mcp",
    // After mcp (65) so mcp_servers table is initialized, and after
    // memory (80) which owns the user_memories table.
    order: 85,
    description: "Built-in MCP server exposing memory tools (remember/recall/forget)",
    constructor: || Box::new(MemoryMcpModule::new()),
};

pub struct MemoryMcpModule {
    #[allow(dead_code)]
    pool: Option<Arc<PgPool>>,
}

impl MemoryMcpModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl Default for MemoryMcpModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for MemoryMcpModule {
    fn name(&self) -> &'static str {
        "memory_mcp"
    }

    fn description(&self) -> &'static str {
        "Built-in MCP server exposing memory tools (remember/recall/forget)"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // Defense-in-depth: route the built-in MCP URL through the same
        // helper code_sandbox uses (audit R6-#3). The helper PINS
        // loopback regardless of `ctx.config.server.host`, so a
        // misconfigured operator can't make this server's JSON-RPC
        // endpoint resolve to attacker.com — every MCP client call
        // would ship JWT-signed bearer tokens to the wrong place.
        let host = crate::modules::code_sandbox::loopback_host(&ctx.config.server.host);
        let loopback_url = format!(
            "http://{host}:{port}/api/memories/mcp",
            port = ctx.config.server.port,
        );

        // Stash the URL so the runtime admin-settings PUT path can register the
        // server when memory is enabled after boot (see register_builtin_server).
        let _ = MEMORY_MCP_LOOPBACK_URL.set(loopback_url.clone());

        let server_id = memory_mcp_server_id();
        let pool = ctx.db_pool.clone();
        let upsert_url = loopback_url.clone();
        tokio::spawn(async move {
            // Only register the built-in MCP server when memory is
            // enabled in the runtime DB toggle.  This mirrors how
            // lit_search / bio_mcp / code_sandbox gate on deploy-level
            // Config entries, adapted for a DB-only toggle since
            // there is no MemoryConfig in Config.
            match sqlx::query_scalar::<_, Option<bool>>(
                "SELECT enabled FROM memory_admin_settings"
            )
            .fetch_one(&*pool)
            .await
            {
                Ok(Some(true)) => {
                    // enabled — proceed with registration
                }
                Ok(_) => {
                    tracing::info!(
                        "memory_mcp: skipped registration — \
                         memory_admin_settings.enabled is not true"
                    );
                    return;
                }
                Err(e) => {
                    // DB error — log and continue (fail-open so an
                    // intermittent blip doesn't prevent registration).
                    tracing::error!(
                        "memory_mcp: failed to read memory_admin_settings: {e:?}; \
                         proceeding with registration"
                    );
                }
            }

            let repo = repository::MemoryMcpRepository::new((*pool).clone());
            match repo.upsert_builtin_server(server_id, &upsert_url).await {
                Ok(()) => tracing::info!(
                    "memory_mcp: built-in server {server_id} registered at {upsert_url}"
                ),
                Err(e) => tracing::error!(
                    "memory_mcp: upsert_builtin_server failed: {e:?}"
                ),
            }
        });

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::memory_mcp_router())
    }
}
