// Assistant module
// Manages user assistants and system-wide template assistants

use aide::axum::ApiRouter;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::core::EventHandler;
use crate::module_api::AppModule;
use crate::ModuleContext;

pub mod models;
pub mod permissions;
pub mod repository;
pub mod handlers;
pub mod routes;
pub mod events;
pub mod event_handlers;

// Re-export database entities from models
pub use models::{Assistant, CreateAssistantRequest, UpdateAssistantRequest, AssistantListResponse, ModelParameters};

// Re-export permissions
pub use permissions::*;

// Re-export repository functions
pub use repository::*;

// Re-export router
pub use routes::assistant_router;

// Re-export events
pub use events::AssistantEvent;

/// Assistant Module
/// Manages user-created assistants and system-wide template assistants
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

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(pool) = &self.pool {
            // Create assistant router with pool state
            let assistant_module_router = ApiRouter::new()
                .merge(assistant_router())
                .with_state((**pool).clone());

            router.merge(assistant_module_router)
        } else {
            tracing::error!("AssistantModule: Pool not initialized during route registration");
            router
        }
    }

    fn register_event_handlers(&self) -> Vec<Arc<dyn EventHandler>> {
        vec![
            event_handlers::CloneTemplateAssistantsHandler::new(),
        ]
    }
}

impl Default for AssistantModule {
    fn default() -> Self {
        Self::new()
    }
}
