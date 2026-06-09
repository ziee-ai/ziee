// =====================================================
// LLM Repository Module
// =====================================================
//
// This module manages external LLM model repositories (Hugging Face, GitHub, etc.)
// with authentication support for downloading and accessing models

pub mod connection_health;
pub mod events;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod types;
pub mod utils;

// Re-export main types and router
pub use repository::LlmRepositoryRepository;
pub use routes::llm_repository_router;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

/// Register llm_repository module
#[distributed_slice(MODULE_ENTRIES)]
static LLM_REPOSITORY_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "llm_repository",
    order: 25,
    description: "LLM repository and model source management",
    constructor: || Box::new(LlmRepositoryModule::new()),
};

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

        // Boot health check — probe every enabled LLM repository and
        // auto-disable unreachable ones. Fire-and-forget so it
        // doesn't block boot; the next `cargo run` retries. Mirrors
        // the MCP module's startup probe pattern at `mcp/mod.rs`.
        // Pool is passed explicitly so the function is callable from
        // the integration-test crate too (where the global Repos
        // factory is not initialized).
        let health_pool = (*ctx.db_pool).clone();
        tokio::spawn(async move {
            connection_health::run_startup_health_check(health_pool).await;
        });

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(_pool) = &self.pool {
            router.merge(llm_repository_router())
        } else {
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
