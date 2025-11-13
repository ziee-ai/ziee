mod handlers;
mod routes;
mod types;

pub use routes::routes;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use std::sync::Arc;
use std::error::Error;

use crate::module_api::{AppModule, ModuleContext, ModuleEntry, MODULE_ENTRIES};

/// Register health module
#[distributed_slice(MODULE_ENTRIES)]
static HEALTH_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "health",
    order: 85,
    description: "Health checks and system status",
    constructor: || Box::new(HealthModule::new()),
};

/// Health check module - provides health and readiness endpoints
pub struct HealthModule {
    pool: Option<Arc<PgPool>>,
}

impl HealthModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl AppModule for HealthModule {
    fn name(&self) -> &'static str {
        "health"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn description(&self) -> &'static str {
        "Health check and readiness endpoints"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes())
    }
}

impl Default for HealthModule {
    fn default() -> Self {
        Self { pool: None }
    }
}
