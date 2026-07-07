//! office_bridge persistence: the singleton settings row + the idempotent
//! built-in MCP server upsert.

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

use super::models::OfficeBridgeSettings;

#[derive(Clone, Debug)]
pub struct OfficeBridgeRepository {
    pool: PgPool,
}

impl OfficeBridgeRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Idempotent upsert of the built-in office_bridge MCP server row. Mirrors
    /// `web_search::upsert_builtin_server`: on conflict, only re-assert the
    /// identity columns (the loopback `url` carries the live port).
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
                $1, NULL, 'office_bridge', 'Office Bridge',
                'Built-in bridge to open Microsoft Office documents (Word/Excel/PowerPoint)',
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

    pub async fn get_settings(&self) -> Result<OfficeBridgeSettings, AppError> {
        let row = sqlx::query_as!(
            OfficeBridgeSettings,
            r#"
            SELECT
                enabled,
                port,
                last_connected_at as "last_connected_at: _",
                cert_fingerprint
            FROM office_bridge_settings
            WHERE id = TRUE
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    /// Update the singleton settings row. Each field: `None` = leave.
    pub async fn update_settings(
        &self,
        enabled: Option<bool>,
        port: Option<i32>,
    ) -> Result<OfficeBridgeSettings, AppError> {
        let row = sqlx::query_as!(
            OfficeBridgeSettings,
            r#"
            UPDATE office_bridge_settings SET
                enabled = COALESCE($1, enabled),
                port    = COALESCE($2, port)
            WHERE id = TRUE
            RETURNING
                enabled,
                port,
                last_connected_at as "last_connected_at: _",
                cert_fingerprint
            "#,
            enabled,
            port,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    /// TEST-1 — `upsert_builtin_server` is the boot-time registrar for the
    /// built-in `office_bridge.ziee.internal` row; it runs in a `tokio::spawn`
    /// on EVERY server start (`office_bridge/mod.rs`), so a restart/re-register
    /// must stay consistent: one row keyed on `id`, with the loopback `url`
    /// (whose port could change across restarts) re-asserted via the
    /// `ON CONFLICT (id) DO UPDATE` contract — never a unique-violation, never
    /// a duplicate row.
    ///
    /// DB-gated: soft-skips (mirroring `web_search::repository`'s idempotency
    /// test) when `DATABASE_URL` is unset / unreachable, so `cargo test --lib`
    /// without Postgres stays green.
    #[tokio::test]
    async fn upsert_builtin_server_reregister_is_idempotent_and_reasserts_url() {
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

        let repo = OfficeBridgeRepository::new(pool.clone());
        // A per-test id so parallel in-source tests can't collide on the row
        // (and so we never touch the real boot-registered office_bridge row).
        let server_id = Uuid::new_v4();

        // First boot: inserts the row at port 44300.
        repo.upsert_builtin_server(server_id, "http://127.0.0.1:44300/api/office-bridge/mcp")
            .await
            .expect("first upsert (insert) must succeed");

        // Second boot / re-register, SAME id, DIFFERENT loopback port: must NOT
        // unique-violate, must NOT duplicate, and must re-assert the new url.
        repo.upsert_builtin_server(server_id, "http://127.0.0.1:44399/api/office-bridge/mcp")
            .await
            .expect("second upsert (on-conflict update) must succeed, not error");

        // Exactly one row for the id (idempotent — no duplicate insert).
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mcp_servers WHERE id = $1")
            .bind(server_id)
            .fetch_one(&pool)
            .await
            .expect("count query");
        assert_eq!(
            count, 1,
            "re-register must leave exactly one row for the id, not duplicate"
        );

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
            Some("http://127.0.0.1:44399/api/office-bridge/mcp"),
            "ON CONFLICT DO UPDATE must re-assert the new loopback url"
        );
        assert!(row.is_system, "is_system stays true across re-register");
        assert!(row.is_built_in, "is_built_in stays true across re-register");
        assert_eq!(
            row.transport_type, "http",
            "transport_type re-asserted to http"
        );

        // Cleanup so the per-test row doesn't linger in a shared DB.
        let _ = sqlx::query("DELETE FROM mcp_servers WHERE id = $1")
            .bind(server_id)
            .execute(&pool)
            .await;
    }
}
