//! Built-in MCP server for LLM-initiated elicitation (`ask_user`).
//!
//! Registers `elicitation.ziee.internal` as a regular `mcp_servers` row
//! with `is_built_in=true` + `transport_type='http'`, pointed at a
//! loopback URL on the same axum app, serving JSON-RPC at
//! `/api/elicitation/mcp`. It exposes ONE tool:
//!
//! - `ask_user(message, schema)` — pause the turn and ask the human user
//!   for structured input (multiple-choice via `enum`, or validated input
//!   via `format`/`pattern`), then resume with their answer.
//!
//! Unlike the other built-ins, `ask_user` does NOT execute over the
//! loopback: the chat tool-loop (`mcp/chat_extension/helpers.rs::execute_tool`)
//! INTERCEPTS it and drives the existing elicitation pipeline inline
//! (registry → `mcpElicitationRequired` SSE → DB content block →
//! `POST /api/mcp/elicitation/{id}/respond`), because only the chat-stream
//! context has the live `sse_tx`. The loopback handler here only needs to
//! answer `initialize` + `tools/list` (so the tool is discoverable and the
//! session connects); its `tools/call` is a never-hit fallback.

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

pub use repository::ElicitationMcpRepository;

/// Deterministic UUID for the built-in elicitation MCP server row.
/// Stable across deployments (mirrors `memory_mcp_server_id`).
pub fn elicitation_mcp_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"elicitation.ziee.internal")
}

#[distributed_slice(MODULE_ENTRIES)]
static ELICITATION_MCP_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "elicitation_mcp",
    // After mcp (65) so the mcp_servers table exists; placed at the END of the
    // built-in MCP band (memory_mcp 85 / files_mcp 86 / skill_mcp 87 /
    // workflow_mcp 88) so the built-in servers stay grouped + collision-free.
    order: 89,
    description: "Built-in MCP server exposing the ask_user elicitation tool",
    constructor: || Box::new(ElicitationMcpModule::new()),
};

pub struct ElicitationMcpModule {
    #[allow(dead_code)]
    pool: Option<Arc<PgPool>>,
}

impl ElicitationMcpModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl Default for ElicitationMcpModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for ElicitationMcpModule {
    fn name(&self) -> &'static str {
        "elicitation_mcp"
    }

    fn description(&self) -> &'static str {
        "Built-in MCP server exposing the ask_user elicitation tool"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // PIN loopback regardless of `ctx.config.server.host` (same helper
        // memory_mcp/files_mcp use) so the built-in MCP URL can't resolve
        // off-box and leak JWT-signed bearer tokens.
        let host = crate::modules::code_sandbox::loopback_host(&ctx.config.server.host);
        let loopback_url = format!(
            "http://{host}:{port}/api/elicitation/mcp",
            port = ctx.config.server.port,
        );

        let server_id = elicitation_mcp_server_id();
        let pool = ctx.db_pool.clone();
        let upsert_url = loopback_url.clone();
        tokio::spawn(async move {
            let repo = repository::ElicitationMcpRepository::new((*pool).clone());
            match repo.upsert_builtin_server(server_id, &upsert_url).await {
                Ok(()) => tracing::info!(
                    "elicitation_mcp: built-in server {server_id} registered at {upsert_url}"
                ),
                Err(e) => tracing::error!(
                    "elicitation_mcp: upsert_builtin_server failed: {e:?}"
                ),
            }
        });

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::elicitation_mcp_router())
    }
}
