// =====================================================
// Hardware Module
// =====================================================
//
// This module provides hardware information and real-time monitoring
// including OS, CPU, Memory, and GPU information via REST and SSE APIs

pub mod api;
pub mod detection;
pub mod handlers;
pub mod monitoring;
pub mod permissions;
pub mod routes;
pub mod types;

// Re-export main types and router
pub use routes::hardware_router;
pub use types::{HardwareInfo, HardwareInfoResponse, HardwareUsageUpdate};

use aide::axum::ApiRouter;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::module_api::{AppModule, ModuleContext};

/// Hardware module for system monitoring
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

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(pool) = &self.pool {
            // Create a stateful hardware router
            let hardware_module_router = ApiRouter::new()
                .merge(hardware_router())
                .with_state((**pool).clone());

            // Merge the stateful router into the provided stateless router
            router.merge(hardware_module_router)
        } else {
            // Pool not initialized - this shouldn't happen in normal flow
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
