//! skill_mcp upsert — mirrors memory_mcp's / files_mcp's
//! `upsert_builtin_server`. Identity columns are re-asserted on each
//! boot so the loopback `url` always carries the live port; the
//! editable columns (`enabled`, `display_name`, etc.) are deliberately
//! left untouched on conflict.

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

#[derive(Clone, Debug)]
pub struct SkillMcpRepository {
    pool: PgPool,
}

impl SkillMcpRepository {
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
                $1, NULL, 'skill', 'Skills',
                'Built-in skill body access (load_skill / read_skill_file)',
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
