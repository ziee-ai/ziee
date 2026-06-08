//! memory_mcp upsert — mirrors code_sandbox's `upsert_builtin_server`.

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

#[derive(Clone, Debug)]
pub struct MemoryMcpRepository {
    pool: PgPool,
}

impl MemoryMcpRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Idempotent upsert of the built-in memory MCP server row.
    ///
    /// Built-ins are immutable via the API: `update_system_mcp_server`
    /// rejects any modification of an `is_built_in` row, so the
    /// `ON CONFLICT DO UPDATE` clause only re-asserts identity columns
    /// (`is_system`, `is_built_in`, `transport_type`, `url`) on each boot —
    /// the loopback `url` carries the live port, which can change between
    /// restarts. The remaining columns (`enabled`, `display_name`,
    /// `description`, `timeout_seconds`, `usage_mode`,
    /// `max_concurrent_sessions`) are deliberately left untouched on conflict.
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
                $1, NULL, 'memory', 'Memory',
                'Built-in memory tools (remember / recall / forget)',
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
