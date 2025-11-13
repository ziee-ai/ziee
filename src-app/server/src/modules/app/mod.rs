// App module - Application-level endpoints
mod handlers;
mod repository;
mod routes;
mod types;
mod utils;

pub use repository::AppRepository;
pub use routes::app_routes;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::module_api::{AppModule as AppModuleTrait, MODULE_ENTRIES, ModuleContext, ModuleEntry};

/// Register app module
#[distributed_slice(MODULE_ENTRIES)]
static APP_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "app",
    order: 90,
    description: "General application routes and information",
    constructor: || Box::new(AppModule::new()),
};

/// App module for application-level endpoints
/// Note: Kept as manual registration due to complex state handling requirements
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

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn description(&self) -> &'static str {
        "Application-level endpoints and setup"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(_pool) = &self.pool {
            let app_router_with_state = ApiRouter::new().nest("/app", app_routes());
            router.merge(app_router_with_state)
        } else {
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
