//! control_mcp persistence: the idempotent built-in MCP server upsert.
//!
//! Mirrors `web_search::repository::upsert_builtin_server` / `memory_mcp`: on
//! conflict, only re-assert the identity columns (the loopback `url` carries the
//! live port).

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

#[derive(Clone, Debug)]
pub struct ControlMcpRepository {
    pool: PgPool,
}

impl ControlMcpRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Idempotent upsert of the built-in control MCP server row.
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
                $1, NULL, 'control', 'App Control',
                'Built-in app-control tools (list_capabilities / describe_capability / invoke_capability)',
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
