//! files_mcp upsert — mirrors memory_mcp's `upsert_builtin_server`.

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

#[derive(Clone, Debug)]
pub struct FilesMcpRepository {
    pool: PgPool,
}

impl FilesMcpRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Register/refresh the built-in `files` MCP server row. Privileged
    /// built-in: `is_system + is_built_in + enabled`, `user_id = NULL`. Unlike
    /// memory/code_sandbox it is NOT exposed through the group-gated
    /// `list_accessible` path — the chat extension auto-attaches it by id, so it
    /// needs no `user_group_mcp_servers` grant.
    pub async fn upsert_builtin_server(
        &self,
        server_id: Uuid,
        loopback_url: &str,
    ) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::database_error)?;
        sqlx::query!(
            r#"
            INSERT INTO mcp_servers (
                id, user_id, name, display_name, description,
                enabled, is_system, is_built_in,
                transport_type, url, headers,
                timeout_seconds, supports_sampling, usage_mode, max_concurrent_sessions,
                created_at, updated_at
            ) VALUES (
                $1, NULL, 'files', 'Files',
                'Built-in agentic file access (list_files / read_file / grep_files)',
                true, true, true,
                'http', $2, '{}'::jsonb,
                30, false, 'auto', 4,
                NOW(), NOW()
            )
            ON CONFLICT (id) DO UPDATE SET
                is_system = EXCLUDED.is_system,
                is_built_in = EXCLUDED.is_built_in,
                transport_type = EXCLUDED.transport_type,
                url = EXCLUDED.url,
                updated_at = NOW()
            "#,
            server_id,
            loopback_url
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
        tx.commit().await.map_err(AppError::database_error)?;
        Ok(())
    }
}
