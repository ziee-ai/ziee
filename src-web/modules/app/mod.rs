// App module - Application-level endpoints
mod handlers;
mod routes;
mod types;
mod utils;

pub use routes::app_routes;
pub use types::{SetupStatusResponse, SetupAdminRequest};

use aide::axum::ApiRouter;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::module_api::{AppModule as AppModuleTrait, ModuleContext};

/// App module for application-level endpoints
pub struct AppModule {
    pool: Option<Arc<PgPool>>,
}

impl AppModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl AppModuleTrait for AppModule {
    fn name(&self) -> &'static str {
        "app"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(pool) = &self.pool {
            // Create a stateful app router
            let app_router_with_state = ApiRouter::new()
                .nest("/app", app_routes())
                .with_state((**pool).clone());

            // Merge the stateful router into the provided stateless router
            router.merge(app_router_with_state)
        } else {
            // Pool not initialized - this shouldn't happen in normal flow
            tracing::error!("AppModule: Pool not initialized during route registration");
            router
        }
    }
}

impl Default for AppModule {
    fn default() -> Self {
        Self::new()
    }
}
