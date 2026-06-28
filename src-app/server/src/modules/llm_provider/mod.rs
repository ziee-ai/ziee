// =====================================================
// LLM Provider Module
// =====================================================
//
// This module manages LLM provider configurations (OpenAI, Anthropic, Local, etc.)
// with support for API keys, proxy settings, and user group assignments

pub mod events;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repositories;
pub mod routes;
pub mod types;
pub mod user_extension;
pub mod utils;

// Re-export main types and router
pub use repositories::admin::LlmProviderRepository;
pub use repositories::user::UserKeyRepository;
pub use routes::llm_provider_router;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

/// Register llm_provider module
#[distributed_slice(MODULE_ENTRIES)]
static LLM_PROVIDER_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "llm_provider",
    order: 30,
    description: "LLM provider configuration and management",
    constructor: || Box::new(LlmProviderModule::new()),
};

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
        if let Some(_pool) = &self.pool {
            router.merge(llm_provider_router())
        } else {
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
