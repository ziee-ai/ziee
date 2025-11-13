// Assistant module
// Manages user assistants and system-wide template assistants

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::ModuleContext;
use crate::core::EventHandler;
use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleEntry};

pub mod event_handlers;
pub mod events;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod types;

// Re-export database entities from models

// Re-export API types

// Re-export permissions

// Re-export repository functions
pub use repository::*;

// Re-export router
pub use routes::assistant_router;

// Re-export events

/// Register assistant module
#[distributed_slice(MODULE_ENTRIES)]
static ASSISTANT_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "assistant",
    order: 45,
    description: "AI assistant management and interactions",
    constructor: || Box::new(AssistantModule::new()),
};

/// Assistant Module
/// Manages user-created assistants and system-wide template assistants
/// Note: Kept as manual registration due to event handler requirements
pub struct AssistantModule {
    pool: Option<Arc<PgPool>>,
}

impl AssistantModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl AppModule for AssistantModule {
    fn name(&self) -> &'static str {
        "assistant"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn description(&self) -> &'static str {
        "Assistant management and template system"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(_pool) = &self.pool {
            let assistant_module_router = ApiRouter::new().merge(assistant_router());
            router.merge(assistant_module_router)
        } else {
            tracing::error!("AssistantModule: Pool not initialized during route registration");
            router
        }
    }

    fn register_event_handlers(&self) -> Vec<Arc<dyn EventHandler>> {
        vec![event_handlers::CloneTemplateAssistantsHandler::new()]
    }
}

impl Default for AssistantModule {
    fn default() -> Self {
        Self::new()
    }
}
