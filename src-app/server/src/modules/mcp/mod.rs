pub mod events;
mod handlers;
mod models;
mod permissions;
mod repository;
mod routes;
mod types;

pub use models::*;
pub use repository::*;
pub use types::*;

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
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(_pool) = &self.pool {
            let mcp_router_with_state = ApiRouter::new()
                .merge(routes::user_routes())
                .merge(routes::admin_routes());
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
