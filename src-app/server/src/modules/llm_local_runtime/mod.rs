// =====================================================
// Local LLM Runtime Module
// =====================================================
//
// This module manages local LLM runtime instances (llama.cpp, mistral.rs)
// with support for local execution and SSH remote deployment

pub mod binary_manager;
pub mod deployment;
pub mod events;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod runtime_version;
pub mod utils;

// Re-export main types and router
pub use binary_manager::BinaryManager;
pub use events::LlmLocalRuntimeEvent;
pub use repository::LocalRuntimeRepository;
pub use routes::llm_local_runtime_router;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use once_cell::sync::OnceCell;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};
use deployment::manager::DeploymentManager;

// Global deployment manager singleton
static DEPLOYMENT_MANAGER: OnceCell<Arc<DeploymentManager>> = OnceCell::new();

/// Get the global deployment manager instance
pub fn get_deployment_manager() -> Arc<DeploymentManager> {
    DEPLOYMENT_MANAGER
        .get()
        .expect("DeploymentManager not initialized - module init() must be called first")
        .clone()
}

/// Register llm_local_runtime module
#[distributed_slice(MODULE_ENTRIES)]
static LLM_LOCAL_RUNTIME_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "llm_local_runtime",
    order: 32,
    description: "Local LLM runtime instance management (llama.cpp, mistral.rs)",
    constructor: || Box::new(LlmLocalRuntimeModule::new()),
};

/// Local LLM Runtime module for managing runtime instances
pub struct LlmLocalRuntimeModule {
    pool: Option<Arc<PgPool>>,
}

impl LlmLocalRuntimeModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl AppModule for LlmLocalRuntimeModule {
    fn name(&self) -> &'static str {
        "llm_local_runtime"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // Initialize the deployment manager with the database pool
        let deployment_manager = DeploymentManager::new((*ctx.db_pool).clone())?;
        DEPLOYMENT_MANAGER
            .set(Arc::new(deployment_manager))
            .map_err(|_| "DeploymentManager already initialized")?;

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        if let Some(_pool) = &self.pool {
            router.merge(llm_local_runtime_router())
        } else {
            tracing::error!("LlmLocalRuntimeModule: Pool not initialized during route registration");
            router
        }
    }
}

impl Default for LlmLocalRuntimeModule {
    fn default() -> Self {
        Self::new()
    }
}
