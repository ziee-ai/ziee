use aide::axum::ApiRouter;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::core::config::Config;
use crate::core::EventHandler;

/// Core trait that all app modules must implement
pub trait AppModule: Send + Sync {
    /// Unique module name
    fn name(&self) -> &'static str;

    /// Module version
    #[allow(dead_code)]
    fn version(&self) -> &'static str {
        "1.0.0" // Default version
    }

    /// Module description
    #[allow(dead_code)]
    fn description(&self) -> &'static str {
        "" // Default: no description
    }

    /// Initialize module with context
    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>>;

    /// Register API routes
    fn register_routes(&self, router: ApiRouter) -> ApiRouter;

    /// Register event handlers
    /// Returns list of handlers that will receive application events
    fn register_event_handlers(&self) -> Vec<Arc<dyn EventHandler>> {
        vec![] // Default: no handlers
    }
}

/// Context provided to modules during initialization
pub struct ModuleContext {
    pub db_pool: Arc<PgPool>,
    pub config: Arc<Config>,
}

impl ModuleContext {
    pub fn new(db_pool: Arc<PgPool>, config: Arc<Config>) -> Self {
        Self { db_pool, config }
    }
}
