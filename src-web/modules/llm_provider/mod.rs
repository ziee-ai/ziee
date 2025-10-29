// =====================================================
// LLM Provider Module
// =====================================================
//
// This module manages LLM provider configurations (OpenAI, Anthropic, Local, etc.)
// with support for API keys, proxy settings, and user group assignments

pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod service;

// Re-export main types and router
pub use models::*;
pub use permissions::all_permissions;
pub use routes::llm_provider_router;

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
            // Create a stateful LLM provider router
            let llm_provider_module_router = ApiRouter::new()
                .merge(llm_provider_router())
                .with_state((**pool).clone());

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
