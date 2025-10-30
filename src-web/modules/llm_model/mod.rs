// LLM Model module
// Following ziee-chat module patterns

use aide::axum::ApiRouter;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::module_api::AppModule;
use crate::ModuleContext;

pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod service;
pub mod storage;
pub mod uploads;

pub use models::*;
pub use permissions::*;
pub use routes::llm_model_router;

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
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(pool) = &self.pool {
            let llm_model_module_router = ApiRouter::new()
                .merge(llm_model_router())
                .with_state((**pool).clone());
            router.merge(llm_model_module_router)
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
