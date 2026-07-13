//! Built-in MCP server that lets the chat model OPERATE ziee itself.
//!
//! Registers `control.ziee.internal` as a row in `mcp_servers`
//! (`is_built_in=true`, `transport_type='http'`, loopback url) and serves
//! JSON-RPC at `/api/control/mcp`. Three tools — `list_capabilities` /
//! `describe_capability` / `invoke_capability` — expose ziee's own 300+
//! permission-gated REST operations to the model as a precise, authorized
//! control surface.
//!
//! Security model (see the plan + `CODING_GUIDELINES.md` §11): the JSON-RPC
//! handler gates on `control::use`; each tool is filtered to the caller's
//! permissions (the model never sees a forbidden op); `invoke_capability`
//! dispatches to the REAL REST route over loopback carrying the caller's JWT, so
//! the target route re-authorizes from the DB — no authz is reimplemented. The
//! deploy-level `control_mcp.enabled` kill-switch gates init + route
//! registration + the row upsert (§16). Mutating calls always require approval
//! (`mcp/chat_extension/mcp.rs`), so this is NOT on the approval-bypass list.

use std::error::Error;
use std::sync::Arc;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use uuid::Uuid;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

pub mod catalog;
pub mod chat_extension;
pub mod handlers;
pub mod permissions;
pub mod policy;
pub mod repository;
pub mod routes;
pub mod tools;

/// Deterministic UUID for the built-in control MCP server row. Stable across
/// deployments. Mirrors `files_mcp_server_id` / `web_search_server_id`.
pub fn control_mcp_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"control.ziee.internal")
}

/// True when the deploy-level kill-switch permits the control surface. Absent
/// config section means enabled.
fn is_enabled(ctx: &ModuleContext) -> bool {
    crate::module_api::app_config(ctx)
        .control_mcp
        .as_ref()
        .map(|c| c.enabled)
        .unwrap_or(true)
}

#[distributed_slice(MODULE_ENTRIES)]
static CONTROL_MCP_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "control_mcp",
    // After mcp (65, so the mcp_servers table + client exist) for the built-in
    // upsert. 88 lands it after files_mcp (86).
    order: 88,
    description: "Built-in MCP server exposing app-control tools (list/describe/invoke capabilities)",
    constructor: || Box::new(ControlMcpModule::new()),
};

pub struct ControlMcpModule {
    pool: Option<Arc<PgPool>>,
    /// Deploy kill-switch snapshot, set in `init`. Gates `register_routes` so a
    /// disabled deployment doesn't even expose the endpoint (§16).
    enabled: bool,
}

impl ControlMcpModule {
    pub fn new() -> Self {
        Self {
            pool: None,
            enabled: true,
        }
    }
}

impl Default for ControlMcpModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for ControlMcpModule {
    fn name(&self) -> &'static str {
        "control_mcp"
    }

    fn description(&self) -> &'static str {
        "Built-in MCP server exposing app-control tools (list/describe/invoke capabilities)"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());
        self.enabled = is_enabled(ctx);

        if !self.enabled {
            tracing::info!("control_mcp: disabled in config; skipping registration");
            return Ok(());
        }

        // Pin loopback (same defense as the other built-ins) so the JWT-bearing
        // MCP client never ships tokens off-host, and so the invoke dispatch
        // targets this same process.
        let host = crate::modules::code_sandbox::loopback_host(&ctx.config.server.host);
        let port = ctx.config.server.port;
        let loopback_url = format!("http://{host}:{port}/api/control/mcp");
        // Base for the invoke dispatch (the openapi paths already include /api).
        handlers::set_base_url(format!("http://{host}:{port}"));

        let server_id = control_mcp_server_id();
        let pool = ctx.db_pool.clone();
        let upsert_url = loopback_url.clone();
        tokio::spawn(async move {
            let repo = repository::ControlMcpRepository::new((*pool).clone());
            match repo.upsert_builtin_server(server_id, &upsert_url).await {
                Ok(()) => tracing::info!(
                    "control_mcp: built-in server {server_id} registered at {upsert_url}"
                ),
                Err(e) => tracing::error!("control_mcp: upsert_builtin_server failed: {e:?}"),
            }
        });

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        // §16: the deploy kill-switch guards route registration too.
        if !self.enabled {
            return router;
        }
        router.merge(routes::control_mcp_router())
    }
}
