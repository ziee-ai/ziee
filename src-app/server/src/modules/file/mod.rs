// File module

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;

use crate::module_api::{AppModule, ModuleEntry, MODULE_ENTRIES};
use crate::ModuleContext;

pub mod available_files;
pub mod chat_extension;
pub mod config;
pub mod handlers;
pub mod ingest;
pub mod models;
pub mod permissions;
pub mod processing;
pub mod project_extension;
pub mod provider_routing;
pub mod repository;
pub mod routes;
pub mod storage;
pub mod sync;
pub mod versioning;
pub mod types;
pub mod utils;

// Re-export repository for global Repos access
pub use repository::FileRepository;

use routes::file_router;
use storage::manager::init_file_storage;

// Self-registration via distributed slice
#[distributed_slice(MODULE_ENTRIES)]
static FILE_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "file",
    order: 31, // After llm_provider, before llm_model
    description: "File upload, storage, and management",
    constructor: || Box::new(FileModule::new()),
};

pub struct FileModule {
    pool: Option<Arc<PgPool>>,
}

impl FileModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl AppModule for FileModule {
    fn name(&self) -> &'static str {
        "file"
    }

    fn description(&self) -> &'static str {
        "File upload, storage, and management"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        // Initialize JWT config for file downloads
        config::init_jwt_config(Arc::new(ctx.config.jwt.clone()));

        // Extract embedded binaries on first start (one-time operation)
        tracing::info!("Ensuring embedded binaries are extracted");
        utils::embedded::ensure_binaries_extracted()
            .map_err(|e| format!("Failed to extract embedded binaries: {}", e))?;
        tracing::info!("Embedded binaries ready");

        // Initialize file storage - use app_data_dir/files
        let app_data_dir = crate::core::get_app_data_dir();
        let storage_path = app_data_dir.join("files");
        init_file_storage(storage_path.to_str().unwrap_or("./data/files"));

        tracing::info!("File module initialized with storage path: {:?}", storage_path);
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(file_router())
    }
}

impl Default for FileModule {
    fn default() -> Self {
        Self::new()
    }
}
