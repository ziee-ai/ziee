// LLM Model module
// Following ziee-chat module patterns

use aide::axum::ApiRouter;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::module_api::AppModule;
use crate::ModuleContext;

pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod utils;
pub mod storage;
pub mod types;

// Re-export database entities from models
pub use models::{
    DeviceType, DownloadInstance, DownloadPhase, DownloadProgressData, DownloadRequestData,
    DownloadStatus, EngineType, FileFormat, LlamaCppSettings, LlmModel, LlmRepository,
    MistralRsCommand, MistralRsSettings, ModelCapabilities, ModelEngineSettings, ModelFile,
    ModelParameters, SourceInfo,
};

// Re-export API types from types module
pub use types::{
    CreateDownloadInstanceRequest, CreateLlmModelRequest, DownloadInstanceListResponse,
    ListModelsQuery, LlmModelListResponse, UpdateDownloadProgressRequest,
    UpdateDownloadStatusRequest, UpdateLlmModelRequest,
};

// Re-export other public items
pub use permissions::*;
pub use repository::{DownloadInstanceRepository, LlmModelRepository};
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
            // Create repositories once at module level
            let model_repo = repository::LlmModelRepository::new((**pool).clone());
            let download_repo = repository::DownloadInstanceRepository::new((**pool).clone());

            // Create LLM model router with both state (for permission checks) and extensions (for repositories)
            let llm_model_module_router = ApiRouter::new()
                .merge(llm_model_router())
                .with_state((**pool).clone())
                .layer(axum::Extension(model_repo))
                .layer(axum::Extension(download_repo));

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
