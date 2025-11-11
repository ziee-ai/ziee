// =====================================================
// LLM Repository Module
// =====================================================
//
// This module manages external LLM model repositories (Hugging Face, GitHub, etc.)
// with authentication support for downloading and accessing models

pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod utils;
pub mod types;

// Re-export main types and router
pub use models::*;
pub use types::*;
pub use permissions::all_permissions;
pub use repository::LlmRepositoryRepository;
pub use routes::llm_repository_router;

use aide::axum::ApiRouter;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::module_api::{AppModule, ModuleContext};

/// LLM Repository module for managing model repositories
pub struct LlmRepositoryModule {
    pool: Option<Arc<PgPool>>,
}

impl LlmRepositoryModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl AppModule for LlmRepositoryModule {
    fn name(&self) -> &'static str {
        "llm_repository"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(pool) = &self.pool {
            // Create repository once at module level
            let llm_repo = repository::LlmRepositoryRepository::new((**pool).clone());

            // Create LLM repository router with both state (for permission checks) and extensions (for repository)
            let llm_repo_module_router = ApiRouter::new()
                .merge(llm_repository_router())
                .with_state((**pool).clone())
                .layer(axum::Extension(llm_repo));

            // Merge the stateful router into the provided stateless router
            router.merge(llm_repo_module_router)
        } else {
            // Pool not initialized - this shouldn't happen in normal flow
            tracing::error!("LlmRepositoryModule: Pool not initialized during route registration");
            router
        }
    }
}

impl Default for LlmRepositoryModule {
    fn default() -> Self {
        Self::new()
    }
}
