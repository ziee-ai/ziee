//! Built-in MCP server exposing installed workflows as opaque tools.
//!
//! Registers `workflow.ziee.internal` in `mcp_servers`
//! (`is_built_in=true`, `transport_type='http'`, loopback url) and
//! serves JSON-RPC at `/api/workflows/mcp`. Mirrors `skill_mcp` /
//! `memory_mcp` / `files_mcp` registration. The MCP client at
//! `mcp/client/manager.rs` injects a short-lived JWT +
//! `x-conversation-id` for built-in servers, so the handler
//! authenticates the user (gated on `workflows::execute`) AND scopes
//! the run to the originating conversation.
//!
//! From the LLM's view a workflow is ONE opaque tool call: the internal
//! `llm` / `sandbox` steps never enter the conversation history. The
//! tool handler spawns the runner (the same `runner::spawn_run` path the
//! REST `/run` handler uses), blocks until the run reaches a terminal
//! status, and returns the resolved `outputs[]` via
//! `tools::format_outputs_for_mcp` (plan §4.7).
//!
//! Three surfaces:
//! - `tools/list` — one `wf_<slug>` tool per accessible installed
//!   workflow whose `enabled = TRUE` (input schema derived from
//!   `workflow.inputs[]`; 128-char composed-name cap enforced).
//! - `tools/call` — spawn + await + `format_outputs_for_mcp`.
//! - `resources/list` + `resources/read` — outputs / artifacts / logs
//!   for the user's recent runs under the `ziee://workflow-runs/...`
//!   URI scheme (plan §4.7).

use std::error::Error;
use std::sync::Arc;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use uuid::Uuid;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

pub mod handlers;
pub mod repository;
pub mod resources;
pub mod routes;
pub mod tools;

pub use repository::WorkflowMcpRepository;

/// Deterministic UUID for the built-in workflow MCP server row. Stable
/// across deployments. Mirrors `skill_mcp_server_id` /
/// `memory_mcp_server_id` / `files_mcp_server_id`.
pub fn workflow_mcp_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"workflow.ziee.internal")
}

#[distributed_slice(MODULE_ENTRIES)]
static WORKFLOW_MCP_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "workflow_mcp",
    // After workflow (82 — owns the workflows + workflow_runs tables +
    // the runner) and mcp (65 — owns the mcp_servers table + client).
    // Same band as skill_mcp (87) / memory_mcp (85) / files_mcp (86).
    order: 88,
    description: "Built-in MCP server exposing workflows as opaque tools",
    constructor: || Box::new(WorkflowMcpModule::new()),
};

pub struct WorkflowMcpModule {
    #[allow(dead_code)]
    pool: Option<Arc<PgPool>>,
}

impl WorkflowMcpModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl Default for WorkflowMcpModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for WorkflowMcpModule {
    fn name(&self) -> &'static str {
        "workflow_mcp"
    }

    fn description(&self) -> &'static str {
        "Built-in MCP server exposing workflows as opaque tools"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // Pin loopback (defense in depth — the JWTs the MCP client signs
        // for built-in servers MUST NOT leave the host).
        let host = crate::modules::code_sandbox::loopback_host(&ctx.config.server.host);
        let loopback_url = format!(
            "http://{host}:{port}/api/workflows/mcp",
            port = ctx.config.server.port,
        );

        let server_id = workflow_mcp_server_id();
        let pool = ctx.db_pool.clone();
        let upsert_url = loopback_url.clone();
        tokio::spawn(async move {
            let repo = repository::WorkflowMcpRepository::new((*pool).clone());
            match repo.upsert_builtin_server(server_id, &upsert_url).await {
                Ok(()) => tracing::info!(
                    "workflow_mcp: built-in server {server_id} registered at {upsert_url}"
                ),
                Err(e) => {
                    tracing::error!("workflow_mcp: upsert_builtin_server failed: {e:?}")
                }
            }
        });

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::workflow_mcp_router())
    }
}
