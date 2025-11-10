// Hub module
// Manages marketplace data for models, assistants, and MCP servers

use aide::axum::ApiRouter;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::module_api::AppModule;
use crate::ModuleContext;

pub mod models;
pub mod types;
pub mod handlers;
pub mod routes;
pub mod permissions;
pub mod hub_manager;
pub mod repository;

// Re-export models
pub use models::{HubModel, HubAssistant, HubMCPServer};

// Re-export permissions
pub use permissions::*;

// Re-export hub manager
pub use hub_manager::HubManager;

// Re-export router
pub use routes::hub_router;

/// Hub Module
/// Manages marketplace data for models, assistants, and MCP servers
pub struct HubModule {
    pool: Option<Arc<PgPool>>,
}

impl HubModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl AppModule for HubModule {
    fn name(&self) -> &'static str {
        "hub"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // Initialize hub manager and copy embedded files on startup
        let app_data_dir = crate::core::get_app_data_dir();
        let hub_manager = hub_manager::HubManager::new(app_data_dir)?;
        hub_manager.initialize()?;

        tracing::info!("Hub module initialized with embedded data");

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(pool) = &self.pool {
            let hub_module_router = ApiRouter::new()
                .merge(hub_router())
                .with_state((**pool).clone());

            router.merge(hub_module_router)
        } else {
            tracing::error!("HubModule: Pool not initialized during route registration");
            router
        }
    }
}

impl Default for HubModule {
    fn default() -> Self {
        Self::new()
    }
}
