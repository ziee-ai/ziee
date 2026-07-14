// Chunk `health` moved the DB-free module body (the `HealthResponse` wire type,
// the pure `health_check` handler + docs, the `routes()` builder) into the
// `ziee-health` crate. This module keeps the `AppModule`/`MODULE_ENTRIES`
// registration (which names ziee's `module_api`) and re-exports the moved pieces
// as equivalence-preserving shims, so `routes()` + `crate::modules::health::…`
// resolve unchanged and the emitted OpenAPI is byte-identical.
// `routes` re-exports both the module AND the crate-root `routes()` fn (value
// namespace, via ziee-health's `pub use routes::routes`), so `register_routes`'
// call to `routes()` below resolves, mirroring the former `pub use routes::routes;`.
#[allow(unused_imports)]
pub use ziee_health::{handlers, routes, types};

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

/// Register health module
#[distributed_slice(MODULE_ENTRIES)]
static HEALTH_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "health",
    order: 85,
    description: "Health checks and system status",
    constructor: || Box::new(HealthModule::new()),
};

/// Health check module - provides health and readiness endpoints
#[derive(Default)]
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

