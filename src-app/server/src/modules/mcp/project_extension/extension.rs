// MCP module's ProjectExtension implementation.
//
// Registers via `#[distributed_slice(PROJECT_EXTENSIONS)]`. The project
// module's `auto_register_project_extensions` picks this up at boot
// without importing the mcp module.
//
// Contributions:
//   1. `register_routes` — mounts `/api/projects/{id}/mcp-settings`.
//   2. `on_conversation_attached` — snapshots project's mcp_settings
//      row → conversation row (INSERT…SELECT on the unified table).
//   3. `on_conversation_detached` — deletes the conversation row.
//   4. `on_project_duplicated` — snapshots src project → dst project.

use aide::axum::ApiRouter;
use async_trait::async_trait;
use linkme::distributed_slice;
use sqlx::{PgPool, Postgres, Transaction};
use std::sync::Arc;
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;
use crate::core::config::Config;
use crate::modules::mcp::settings::McpScope;
use crate::modules::project::core::extension::{
    PROJECT_EXTENSIONS, ProjectExtension, ProjectExtensionEntry,
};

use super::routes::project_mcp_settings_router;

pub struct McpProjectExtension {
    _pool: PgPool,
    _config: Arc<Config>,
}

impl McpProjectExtension {
    pub fn new(pool: PgPool, config: Arc<Config>) -> Self {
        Self {
            _pool: pool,
            _config: config,
        }
    }
}

#[async_trait]
impl ProjectExtension for McpProjectExtension {
    fn name(&self) -> &str {
        "mcp"
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(project_mcp_settings_router())
    }

    async fn on_conversation_attached(
        &self,
        project_id: Uuid,
        conversation_id: Uuid,
        user_id: Uuid,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), AppError> {
        // Try to snapshot the project's persisted MCP row. If the project
        // never customized its defaults (no row), still write a row with
        // the column defaults so the snapshot contract holds: every
        // attached conversation has its OWN frozen-at-attach copy that
        // subsequent project edits don't propagate to.
        let copied = Repos
            .mcp_settings
            .snapshot(
                McpScope::Project(project_id),
                McpScope::Conversation(conversation_id),
                user_id,
                tx,
            )
            .await?;
        if !copied {
            sqlx::query!(
                r#"
                INSERT INTO mcp_settings (
                    conversation_id, user_id,
                    approval_mode, auto_approved_tools, disabled_servers, loop_settings
                )
                VALUES ($1, $2, DEFAULT, DEFAULT, DEFAULT, NULL)
                ON CONFLICT (conversation_id) DO UPDATE SET
                    approval_mode       = EXCLUDED.approval_mode,
                    auto_approved_tools = EXCLUDED.auto_approved_tools,
                    disabled_servers    = EXCLUDED.disabled_servers,
                    loop_settings       = EXCLUDED.loop_settings,
                    updated_at          = NOW()
                "#,
                conversation_id,
                user_id,
            )
            .execute(&mut **tx)
            .await
            .map_err(AppError::database_error)?;
        }
        tracing::debug!(
            project_id = %project_id,
            conversation_id = %conversation_id,
            copied,
            "mcp.project_extension: snapshot project → conversation"
        );
        Ok(())
    }

    async fn on_conversation_detached(
        &self,
        conversation_id: Uuid,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), AppError> {
        // Use the transaction so the delete is atomic with the
        // project_conversations row removal. The repo's `delete` API
        // takes &self.pool, so do the SQL directly here to stay
        // inside the tx.
        let deleted = sqlx::query!(
            "DELETE FROM mcp_settings WHERE conversation_id = $1",
            conversation_id
        )
        .execute(&mut **tx)
        .await
        .map_err(AppError::database_error)?
        .rows_affected();
        tracing::debug!(
            conversation_id = %conversation_id,
            deleted,
            "mcp.project_extension: cleared conversation mcp_settings on detach"
        );
        Ok(())
    }

    async fn on_project_duplicated(
        &self,
        src_project_id: Uuid,
        dst_project_id: Uuid,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), AppError> {
        // Need a user_id for the new row. Look it up from the
        // destination project (which the project module's
        // duplicate_in_tx has already inserted with the user_id from
        // the request auth). The destination project is owned by the
        // calling user — read it back to get the FK.
        let user_id = sqlx::query_scalar!(
            "SELECT user_id FROM projects WHERE id = $1",
            dst_project_id
        )
        .fetch_one(&mut **tx)
        .await
        .map_err(AppError::database_error)?;

        let copied = Repos
            .mcp_settings
            .snapshot(
                McpScope::Project(src_project_id),
                McpScope::Project(dst_project_id),
                user_id,
                tx,
            )
            .await?;
        if copied {
            tracing::debug!(
                src_project_id = %src_project_id,
                dst_project_id = %dst_project_id,
                "mcp.project_extension: cloned mcp_settings into duplicate project"
            );
        }
        Ok(())
    }
}

fn create(pool: PgPool, config: Arc<Config>) -> Arc<dyn ProjectExtension> {
    Arc::new(McpProjectExtension::new(pool, config))
}

#[distributed_slice(PROJECT_EXTENSIONS)]
static MCP_PROJECT_EXTENSION: ProjectExtensionEntry = ProjectExtensionEntry {
    name: "mcp",
    order: 20, // Auth/authorization range (20-39) — settings/access controls.
    factory: create,
};
