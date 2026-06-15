//! workflow_mcp upsert — mirrors skill_mcp's / memory_mcp's
//! `upsert_builtin_server`. Identity columns are re-asserted on each
//! boot so the loopback `url` always carries the live port; the
//! editable columns (`enabled`, `display_name`, etc.) are deliberately
//! left untouched on conflict.
//!
//! (This is the file the plan refers to as `server.rs` — the MCP-server
//! registration row. The mirrored built-in modules keep it in
//! `repository.rs`, so we follow the same convention.)

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

#[derive(Clone, Debug)]
pub struct WorkflowMcpRepository {
    pool: PgPool,
}

impl WorkflowMcpRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

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
                $1, NULL, 'workflow', 'Workflows',
                'Built-in workflow execution (one tool per installed workflow)',
                true, true, true,
                'http', $2, '{}'::jsonb,
                30, false, 'auto', 8,
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
