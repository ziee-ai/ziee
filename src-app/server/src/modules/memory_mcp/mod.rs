//! Built-in MCP server for memory tools.
//!
//! Registers `memory.ziee.internal` as a regular row in `mcp_servers`
//! with `is_built_in=true` + `transport_type='http'`, points at a
//! loopback URL on the same axum app, and serves JSON-RPC at
//! `/api/memory-mcp`. The MCP client at `mcp/client/manager.rs`
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

pub use repository::MemoryMcpRepository;

/// Deterministic UUID for the built-in memory MCP server row.
/// Stable across deployments. Mirrors the `code_sandbox_server_id`
/// pattern.
pub fn memory_mcp_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"memory.ziee.internal")
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

        // Same loopback-pinning rationale as code_sandbox: the URL
        // never points outside the server itself.
        let host = "127.0.0.1";
        let loopback_url = format!(
            "http://{host}:{port}/api/memory-mcp",
            port = ctx.config.server.port,
        );

        let server_id = memory_mcp_server_id();
        let pool = ctx.db_pool.clone();
        let upsert_url = loopback_url.clone();
        tokio::spawn(async move {
            let repo = repository::MemoryMcpRepository::new((*pool).clone());
            if let Err(e) = repo.upsert_builtin_server(server_id, &upsert_url).await {
                tracing::error!("memory_mcp: upsert_builtin_server failed: {e:?}");
            } else {
                tracing::info!(
                    "memory_mcp: built-in server {server_id} registered at {upsert_url}"
                );
            }
        });

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::memory_mcp_router())
    }
}
