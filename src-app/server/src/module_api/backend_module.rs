use aide::axum::ApiRouter;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::core::EventHandler;

/// Core trait that all app modules must implement
pub trait AppModule: Send + Sync {
    /// Unique module name
    fn name(&self) -> &'static str;

    /// Initialize module with context
    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>>;

    /// Register API routes
    fn register_routes(&self, router: ApiRouter) -> ApiRouter;

    /// Register event handlers
    /// Returns list of handlers that will receive application events
    fn register_event_handlers(&self) -> Vec<Arc<dyn EventHandler>> {
        vec![]  // Default: no handlers
    }

    /// Shutdown cleanup
    #[allow(dead_code)]
    fn shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    /// Optional: Module dependencies
    #[allow(dead_code)]
    fn dependencies(&self) -> Vec<&'static str> {
        vec![]
    }
}

/// Context provided to modules during initialization
pub struct ModuleContext {
    pub db_pool: Arc<PgPool>,
}

impl ModuleContext {
    pub fn new(db_pool: Arc<PgPool>) -> Self {
        Self { db_pool }
    }
}
