pub mod chat_extension;
pub mod client;
pub mod connection_health;
pub mod elicitation;
mod event_handlers;
pub mod events;
pub(crate) mod handlers;
mod models;
pub(crate) mod permissions;
pub mod project_extension;
mod repository;
pub mod resource_link;
mod routes;
pub mod runtime_types;
pub mod sampling;
pub mod settings;
mod types;
pub mod user_policy;
mod utils;

pub use models::*;
pub use repository::*;
pub use types::*;
// Re-exports for cross-module use (hub install paths + the
// code_sandbox environments REST shim). Named so the public surface
// stays explicit — bumping the whole `repository` / `permissions`
// modules to `pub mod` would leak internals.
pub use permissions::{McpServersAdminCreate, McpServersAdminRead};
pub(crate) use repository::validate_transport_config;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};
use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

/// Register mcp module
#[distributed_slice(MODULE_ENTRIES)]
static MCP_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "mcp",
    order: 65,
    description: "Model Context Protocol server management",
    constructor: || Box::new(McpModule::new()),
};

/// Note: Kept as manual registration due to stateful route requirements
pub struct McpModule {
    pool: Option<Arc<PgPool>>,
}

impl McpModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl AppModule for McpModule {
    fn name(&self) -> &'static str {
        "mcp"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn description(&self) -> &'static str {
        "Model Context Protocol (MCP) server management"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // Extract embedded UV and Bun binaries on first startup
        tracing::info!("MCP: Ensuring embedded binaries (UV, Bun) are extracted");
        utils::embedded::ensure_binaries_extracted()
            .map_err(|e| format!("Failed to extract MCP embedded binaries: {}", e))?;
        tracing::info!("MCP: Embedded binaries ready");

        // Boot health check — probe every enabled non-built-in MCP
        // server and auto-disable unreachable ones. Fire-and-forget
        // so it doesn't block boot; the next `cargo run` retries.
        let health_pool = (*ctx.db_pool).clone();
        tokio::spawn(async move {
            connection_health::run_startup_health_check(health_pool).await;
        });

        // Boot-time sanity check on the MCP user policy. If the
        // policy allows stdio for users but `code_sandbox` is
        // disabled in this deployment, user stdio create/update calls
        // will 422 with `MCP_SANDBOX_DISABLED` and any pre-existing
        // user stdio rows (force-migrated to sandboxed by migration
        // 84) won't connect. Surface this misconfiguration loudly at
        // boot so the operator catches it before users do.
        let policy_pool = (*ctx.db_pool).clone();
        tokio::spawn(async move {
            match user_policy::load(&policy_pool).await {
                Ok(p) if p.allowed_transports.iter().any(|t| t == "stdio")
                    && crate::modules::code_sandbox::config::get_state().is_none() =>
                {
                    // ERROR-level because this is a misconfiguration
                    // that BREAKS user stdio MCP servers — every
                    // create/update 422s and every connect refuses.
                    // The operator needs to see this in logs even at
                    // info-or-warn filters.
                    tracing::error!(
                        allowed_transports = ?p.allowed_transports,
                        "MCP user policy includes 'stdio' but code_sandbox is \
                         DISABLED in this deployment. User stdio MCP servers \
                         WILL FAIL to connect. Either enable code_sandbox in \
                         config and restart, or PUT /api/mcp/user-policy to \
                         remove 'stdio' from allowed_transports."
                    );
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "MCP user policy boot check skipped (load failed)");
                }
            }
        });

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(_pool) = &self.pool {
            let mcp_router_with_state = ApiRouter::new()
                .merge(routes::user_routes())
                .merge(routes::admin_routes())
                .merge(elicitation::routes::elicitation_routes());
            router.merge(mcp_router_with_state)
        } else {
            tracing::error!("McpModule: Pool not initialized during route registration");
            router
        }
    }

    fn register_event_handlers(&self) -> Vec<Arc<dyn crate::core::events::EventHandler>> {
        vec![event_handlers::McpSessionCleanupHandler::new()]
    }
}

impl Default for McpModule {
    fn default() -> Self {
        Self::new()
    }
}
