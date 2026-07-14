// =====================================================
// Hardware Module
// =====================================================
//
// This module provides hardware information and real-time monitoring
// including OS, CPU, Memory, and GPU information via REST and SSE APIs

// Chunk `hardware` moved the DB-free core (wire `types` + the SSE event enum,
// GPU/CPU/mem `detection`, the SSE `monitoring` broadcaster, and the
// `permissions` keys) into the `ziee-hardware` crate. This module keeps the
// aide/axum boundary (`handlers`/`routes`, which bind ziee's concrete
// `RequirePermissions` resolver) + the `AppModule`/`MODULE_ENTRIES`
// registration, and re-exports the moved core as equivalence-preserving shims so
// every `super::{detection,monitoring,permissions,types}::…` path in the
// retained handlers/routes + `main.rs`'s shutdown call resolve unchanged.
pub mod handlers;
pub mod routes;

#[allow(unused_imports)]
pub use ziee_hardware::{detection, monitoring, permissions, types};

// Re-export main router
pub use routes::hardware_router;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

/// Register hardware module
#[distributed_slice(MODULE_ENTRIES)]
static HARDWARE_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "hardware",
    order: 75,
    description: "Hardware detection and GPU management",
    constructor: || Box::new(HardwareModule::new()),
};

/// Hardware module for system monitoring
/// Note: Kept as manual registration due to stateful route requirements
pub struct HardwareModule {
    pool: Option<Arc<PgPool>>,
}

impl HardwareModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl AppModule for HardwareModule {
    fn name(&self) -> &'static str {
        "hardware"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn description(&self) -> &'static str {
        "System hardware information and monitoring"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(_pool) = &self.pool {
            let hardware_module_router = ApiRouter::new().merge(hardware_router());
            router.merge(hardware_module_router)
        } else {
            tracing::error!("HardwareModule: Pool not initialized during route registration");
            router
        }
    }
}

impl Default for HardwareModule {
    fn default() -> Self {
        Self::new()
    }
}
