mod models;
mod permissions;
mod repository;
mod handlers;
mod routes;

pub use models::*;
pub use permissions::*;
pub use repository::*;

use crate::module_api::{AppModule, ModuleContext};
use aide::axum::ApiRouter;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

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

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(pool) = &self.pool {
            // Create stateful routers
            let mcp_router_with_state = ApiRouter::new()
                .merge(routes::user_routes())
                .merge(routes::admin_routes())
                .with_state((**pool).clone());

            // Merge into the provided router
            router.merge(mcp_router_with_state)
        } else {
            tracing::error!("McpModule: Pool not initialized during route registration");
            router
        }
    }
}
