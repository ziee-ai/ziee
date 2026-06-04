// File module's ProjectExtension implementation.
//
// Registers via `#[distributed_slice(PROJECT_EXTENSIONS)]`. The project
// module's `auto_register_project_extensions` picks this up at boot
// without importing the file module.
//
// Three contributions:
//   1. `register_routes` — mounts `/api/projects/{id}/files*` (URL space
//      belongs to the project module by convention; this is the file
//      module renting it).
//   2. `on_project_duplicated` — clones the `project_files` rows so a
//      duplicated project carries the same file attachments.
//   3. `collect_chat_knowledge` — resolves attached files into
//      provider-routed ContentBlocks for chat-time injection (the
//      project chat extension delegates to this via the registry
//      fan-out so it never references the file module directly).

use aide::axum::ApiRouter;
use ai_providers::ContentBlock;
use async_trait::async_trait;
use linkme::distributed_slice;
use sqlx::{PgPool, Postgres, Transaction};
use std::sync::Arc;
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;
use crate::core::config::Config;
use crate::modules::file::models::File as FileEntity;
use crate::modules::file::project_extension::framing::wrap_project_file_blocks;
use crate::modules::file::project_extension::routes::project_files_router;
use crate::modules::file::provider_routing::process_file_blocks;
use crate::modules::project::core::extension::{
    PROJECT_EXTENSIONS, ProjectExtension, ProjectExtensionEntry,
};

pub struct FileProjectExtension {
    pool: PgPool,
    _config: Arc<Config>,
}

impl FileProjectExtension {
    pub fn new(pool: PgPool, config: Arc<Config>) -> Self {
        Self {
            pool,
            _config: config,
        }
    }
}

#[async_trait]
impl ProjectExtension for FileProjectExtension {
    fn name(&self) -> &str {
        "file"
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(project_files_router())
    }

    async fn on_project_duplicated(
        &self,
        src_project_id: Uuid,
        dst_project_id: Uuid,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), AppError> {
        let copied = Repos
            .project_files
            .clone_for_project(tx, src_project_id, dst_project_id)
            .await?;
        if copied > 0 {
            tracing::debug!(
                src_project_id = %src_project_id,
                dst_project_id = %dst_project_id,
                copied,
                "file.project_extension: cloned project_files rows into duplicate"
            );
        }
        Ok(())
    }

    async fn collect_chat_knowledge(
        &self,
        project_id: Uuid,
        user_id: Uuid,
        provider_id: Uuid,
        provider_type: &str,
    ) -> Result<Vec<ContentBlock>, AppError> {
        let file_ids = Repos.project_files.list_file_ids(project_id).await?;
        if file_ids.is_empty() {
            return Ok(Vec::new());
        }

        tracing::debug!(
            project_id = %project_id,
            file_count = file_ids.len(),
            "file.project_extension: resolving knowledge files into ContentBlocks"
        );

        let mut blocks: Vec<ContentBlock> = Vec::new();
        for file_id in file_ids {
            // Look up the filename so we can build a meaningful wrapper.
            // Defense-in-depth ownership check happens inside
            // process_file_blocks; we only need the filename here.
            let filename = Repos
                .file
                .get_by_id(file_id)
                .await?
                .map(|f: FileEntity| f.filename)
                .unwrap_or_else(|| format!("file-{file_id}"));

            let resolved = process_file_blocks(
                &self.pool,
                file_id,
                provider_id,
                provider_type,
                user_id,
            )
            .await?;
            blocks.extend(wrap_project_file_blocks(&filename, resolved));
        }
        Ok(blocks)
    }
}

fn create(pool: PgPool, config: Arc<Config>) -> Arc<dyn ProjectExtension> {
    Arc::new(FileProjectExtension::new(pool, config))
}

#[distributed_slice(PROJECT_EXTENSIONS)]
static FILE_PROJECT_EXTENSION: ProjectExtensionEntry = ProjectExtensionEntry {
    name: "file",
    order: 40, // Content-extension range (40-59) — knowledge kinds.
    factory: create,
};
