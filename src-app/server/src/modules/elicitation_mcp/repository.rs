//! elicitation_mcp upsert — mirrors memory_mcp's `upsert_builtin_server`.

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

#[derive(Clone, Debug)]
pub struct ElicitationMcpRepository {
    pool: PgPool,
}

impl ElicitationMcpRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Idempotent upsert of the built-in elicitation MCP server row.
    ///
    /// Like the other zero-config built-ins (files/memory), this row is
    /// immutable via the API. The `ON CONFLICT DO UPDATE` clause only
    /// re-asserts identity columns + the loopback `url` (its port can change
    /// across restarts); the rest is left untouched on conflict.
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
                $1, NULL, 'elicitation', 'Elicitation',
                'Built-in user elicitation (ask_user)',
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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    /// `upsert_builtin_server` is the boot-time registrar for the built-in
    /// elicitation row; it runs on EVERY server start, so it must be idempotent
    /// (one row keyed on `id`, with the loopback `url` re-asserted because the
    /// ephemeral port changes across restarts — the audited `ON CONFLICT (id)
    /// DO UPDATE` contract at repository.rs:46-51).
    ///
    /// DB-gated: soft-skips (mirroring the suite's env-gated real-stack tests)
    /// when `DATABASE_URL` is unset / unreachable, so `cargo test --lib` without
    /// Postgres stays green; runs for real wherever `DATABASE_URL` points at a
    /// migrated DB.
    #[tokio::test]
    async fn upsert_builtin_server_is_idempotent_and_reasserts_url() {
        let url = match std::env::var("DATABASE_URL") {
            Ok(u) => u,
            Err(_) => {
                eprintln!("skip: DATABASE_URL unset — no DB to exercise the upsert against");
                return;
            }
        };
        let pool = match PgPoolOptions::new().max_connections(2).connect(&url).await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("skip: DB unreachable ({e})");
                return;
            }
        };

        let repo = ElicitationMcpRepository::new(pool.clone());
        // A per-test id so parallel in-source tests can't collide on the row.
        let server_id = Uuid::new_v4();

        // First boot: inserts the row at port 41111.
        repo.upsert_builtin_server(server_id, "http://127.0.0.1:41111/mcp")
            .await
            .expect("first upsert (insert) must succeed");

        // Second boot, SAME id, DIFFERENT loopback port (the real cross-restart
        // case): must NOT unique-violate, must NOT duplicate, must re-assert url.
        repo.upsert_builtin_server(server_id, "http://127.0.0.1:42222/mcp")
            .await
            .expect("second upsert (on-conflict update) must succeed, not error");

        // Exactly one row for the id (idempotent — no duplicate insert).
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM mcp_servers WHERE id = $1")
                .bind(server_id)
                .fetch_one(&pool)
                .await
                .expect("count query");
        assert_eq!(count, 1, "upsert must leave exactly one row for the id, not duplicate");

        // The conflict branch re-asserted the new loopback url + identity flags.
        let row = sqlx::query!(
            r#"SELECT url, is_system, is_built_in, transport_type
               FROM mcp_servers WHERE id = $1"#,
            server_id
        )
        .fetch_one(&pool)
        .await
        .expect("fetch upserted row");
        assert_eq!(
            row.url.as_deref(),
            Some("http://127.0.0.1:42222/mcp"),
            "ON CONFLICT DO UPDATE must re-assert the new loopback url"
        );
        assert!(row.is_system, "is_system re-asserted");
        assert!(row.is_built_in, "is_built_in re-asserted");
        assert_eq!(row.transport_type, "http", "transport_type re-asserted");

        // Cleanup so the per-test row doesn't linger in a shared DB.
        let _ = sqlx::query("DELETE FROM mcp_servers WHERE id = $1")
            .bind(server_id)
            .execute(&pool)
            .await;
    }
}
