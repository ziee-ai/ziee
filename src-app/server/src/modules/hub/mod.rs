// Hub module
// Manages marketplace data for models, assistants, and MCP servers

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::core::EventHandler;
use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

pub mod event_handlers;
pub mod events;
pub mod handlers;
pub mod hub_manager;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod types;

// Re-export models

// Re-export events

// Re-export permissions

// Re-export hub manager

// Re-export repository
pub use repository::HubRepository;

// Re-export router
pub use routes::hub_router;

/// Register hub module
#[distributed_slice(MODULE_ENTRIES)]
static HUB_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "hub",
    order: 70,
    description: "Module hub and discovery",
    constructor: || Box::new(HubModule::new()),
};

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
        if let Some(_pool) = &self.pool {
            let hub_module_router = ApiRouter::new().merge(hub_router());

            router.merge(hub_module_router)
        } else {
            tracing::error!("HubModule: Pool not initialized during route registration");
            router
        }
    }

    fn register_event_handlers(&self) -> Vec<Arc<dyn EventHandler>> {
        vec![event_handlers::CleanupHubEntitiesHandler::new()]
    }
}

impl Default for HubModule {
    fn default() -> Self {
        Self::new()
    }
}
