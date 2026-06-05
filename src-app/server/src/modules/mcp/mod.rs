pub mod chat_extension;
pub mod client;
pub mod connection_health;
pub mod elicitation;
pub mod events;
pub(crate) mod handlers;
mod models;
mod permissions;
pub mod project_extension;
mod repository;
mod routes;
pub mod runtime_types;
pub mod sampling;
pub mod settings;
mod types;
mod utils;

pub use models::*;
pub use repository::*;
pub use types::*;
// Re-exports for cross-module use (hub install paths). Named so the
// public surface stays explicit — bumping the whole `repository` /
// `permissions` modules to `pub mod` would leak internals.
pub use permissions::McpServersAdminCreate;
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
}

impl Default for McpModule {
    fn default() -> Self {
        Self::new()
    }
}
