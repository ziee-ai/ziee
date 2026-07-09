//! js_tool upsert — mirrors memory_mcp's `upsert_builtin_server`.

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

#[derive(Clone, Debug)]
pub struct JsToolRepository {
    pool: PgPool,
}

impl JsToolRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Idempotent upsert of the built-in `run_js` MCP server row. On conflict
    /// only re-asserts identity + the live loopback port (mirrors memory_mcp);
    /// admin-editable columns (enabled/display_name/…) are left untouched.
    pub async fn upsert_builtin_server(&self, server_id: Uuid, loopback_url: &str) -> Result<(), AppError> {
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
                $1, NULL, 'run_js', 'Programmatic Tool Calling',
                'Built-in run_js tool: write JavaScript that calls your tools in an embedded runtime and returns only a final value',
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
