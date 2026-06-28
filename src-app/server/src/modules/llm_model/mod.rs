// LLM Model module
// Following ziee module patterns

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::ModuleContext;
use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleEntry};

pub mod events;
pub mod handlers;
pub mod model_files;
pub mod models;
pub mod permissions;
pub mod prune;
pub mod repository;
pub mod routes;
pub mod storage;
pub mod types;
pub mod utils;

// Re-export database entities from models
pub use models::ModelParameters;

// Re-export API types from types module

// Re-export other public items
pub use repository::{DownloadInstanceRepository, LlmModelRepository};
pub use routes::llm_model_router;

/// Register llm_model module
#[distributed_slice(MODULE_ENTRIES)]
static LLM_MODEL_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "llm_model",
    order: 35,
    description: "LLM model management and downloads",
    constructor: || Box::new(LlmModelModule::new()),
};

/// LLM Model Module
pub struct LlmModelModule {
    pool: Option<Arc<PgPool>>,
}

impl LlmModelModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl AppModule for LlmModelModule {
    fn name(&self) -> &'static str {
        "llm_model"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // Boot-time retention/eviction loop: prunes terminal download_instances
        // rows (>7d) and evicts stale git/LFS/engine cache entries (>30d,
        // engine binaries still referenced by a runtime version are kept).
        // Fire-and-forget, like the mcp tool-call prune loop.
        let prune_pool = (*ctx.db_pool).clone();
        tokio::spawn(async move {
            prune::run_prune_loop(prune_pool).await;
        });
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(_pool) = &self.pool {
            router.merge(llm_model_router())
        } else {
            tracing::error!("LlmModelModule: Pool not initialized during route registration");
            router
        }
    }
}

impl Default for LlmModelModule {
    fn default() -> Self {
        Self::new()
    }
}
