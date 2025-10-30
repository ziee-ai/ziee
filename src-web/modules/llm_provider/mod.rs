// =====================================================
// LLM Provider Module
// =====================================================
//
// This module manages LLM provider configurations (OpenAI, Anthropic, Local, etc.)
// with support for API keys, proxy settings, and user group assignments

pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod utils;
pub mod types;

// Re-export main types and router
pub use models::*;
pub use permissions::all_permissions;
pub use repository::LlmProviderRepository;
pub use routes::llm_provider_router;
pub use types::*;

use aide::axum::ApiRouter;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::module_api::{AppModule, ModuleContext};

/// LLM Provider module for managing provider configurations
pub struct LlmProviderModule {
    pool: Option<Arc<PgPool>>,
}

impl LlmProviderModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl AppModule for LlmProviderModule {
    fn name(&self) -> &'static str {
        "llm_provider"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(pool) = &self.pool {
            // Create repository once at module level
            let llm_provider_repo = repository::LlmProviderRepository::new((**pool).clone());

            // Create LLM provider router with both state (for permission checks) and extensions (for repository)
            let llm_provider_module_router = ApiRouter::new()
                .merge(llm_provider_router())
                .with_state((**pool).clone())
                .layer(axum::Extension(llm_provider_repo));

            // Merge the stateful router into the provided stateless router
            router.merge(llm_provider_module_router)
        } else {
            // Pool not initialized - this shouldn't happen in normal flow
            tracing::error!("LlmProviderModule: Pool not initialized during route registration");
            router
        }
    }
}

impl Default for LlmProviderModule {
    fn default() -> Self {
        Self::new()
    }
}
