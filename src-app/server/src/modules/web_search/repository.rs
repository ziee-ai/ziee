//! web_search persistence: the singleton settings row, the per-provider
//! config/key rows, and the idempotent built-in MCP server upsert.

use std::collections::HashMap;

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::common::secret::{encrypt_secret, mask_secret, resolve_optional_secret};
use crate::core::secrets::storage_key;

use super::models::{UserProviderKeyEntry, WebSearchSettings};

/// A provider's stored config with the API key DECRYPTED (for internal
/// dispatch + the configured-state check). Never serialized to the API.
#[derive(Debug, Clone)]
pub struct WebSearchProviderRow {
    pub provider: String,
    pub api_key: Option<String>,
    pub config: Value,
}

#[derive(Clone, Debug)]
pub struct WebSearchRepository {
    pool: PgPool,
}

impl WebSearchRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Idempotent upsert of the built-in web_search MCP server row. Mirrors
    /// `memory_mcp::upsert_builtin_server`: on conflict, only re-assert the
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
                $1, NULL, 'web_search', 'Web Search',
                'Built-in web search + page fetch (web_search / fetch_url)',
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

    pub async fn get_settings(&self) -> Result<WebSearchSettings, AppError> {
        let row = sqlx::query_as!(
            WebSearchSettings,
            r#"
            SELECT
                enabled,
                provider_chain,
                max_results,
                fetch_max_bytes,
                fetch_max_chars,
                request_timeout_secs,
                updated_at as "updated_at: _"
            FROM web_search_settings
            WHERE id = TRUE
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    pub async fn update_settings(
        &self,
        enabled: Option<bool>,
        provider_chain: Option<Vec<String>>,
        max_results: Option<i32>,
        fetch_max_bytes: Option<i64>,
        fetch_max_chars: Option<i32>,
        request_timeout_secs: Option<i32>,
    ) -> Result<WebSearchSettings, AppError> {
        let row = sqlx::query_as!(
            WebSearchSettings,
            r#"
            UPDATE web_search_settings SET
                enabled              = COALESCE($1, enabled),
                provider_chain       = COALESCE($2::text[], provider_chain),
                max_results          = COALESCE($3, max_results),
                fetch_max_bytes      = COALESCE($4, fetch_max_bytes),
                fetch_max_chars      = COALESCE($5, fetch_max_chars),
                request_timeout_secs = COALESCE($6, request_timeout_secs),
                updated_at           = NOW()
            WHERE id = TRUE
            RETURNING
                enabled,
                provider_chain,
                max_results,
                fetch_max_bytes,
                fetch_max_chars,
                request_timeout_secs,
                updated_at as "updated_at: _"
            "#,
            enabled,
            provider_chain.as_deref(),
            max_results,
            fetch_max_bytes,
            fetch_max_chars,
            request_timeout_secs,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    /// All configured providers, API keys decrypted for internal use.
    pub async fn list_providers(&self) -> Result<Vec<WebSearchProviderRow>, AppError> {
        let rows = sqlx::query!(
            r#"
            SELECT provider, api_key, api_key_encrypted, config as "config!: Value"
            FROM web_search_providers
            ORDER BY provider
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let api_key = resolve_optional_secret(&self.pool, r.api_key_encrypted, r.api_key).await;
            out.push(WebSearchProviderRow {
                provider: r.provider,
                api_key,
                config: r.config,
            });
        }
        Ok(out)
    }

    /// Upsert one provider's config/key.
    /// `api_key`: None = leave; Some(None) = clear; Some(Some(k)) = set.
    /// `config`: None = leave; Some(v) = replace.
    pub async fn upsert_provider(
        &self,
        provider: &str,
        api_key: Option<Option<String>>,
        config: Option<Value>,
    ) -> Result<(), AppError> {
        let key_provided = api_key.is_some();
        let (plaintext, encrypted): (Option<String>, Option<Vec<u8>>) = match api_key {
            Some(Some(k)) => match encrypt_secret(&self.pool, &k, storage_key()).await? {
                Some(blob) => (None, Some(blob)),
                None => (Some(k), None), // dev fallback: no storage key configured
            },
            // Clear, or leave (the $key_provided flag gates the UPDATE arm).
            Some(None) | None => (None, None),
        };

        sqlx::query!(
            r#"
            INSERT INTO web_search_providers (provider, api_key, api_key_encrypted, config, created_at, updated_at)
            VALUES ($1, $2, $3, COALESCE($5::jsonb, '{}'::jsonb), NOW(), NOW())
            ON CONFLICT (provider) DO UPDATE SET
                api_key           = CASE WHEN $4 THEN $2 ELSE web_search_providers.api_key END,
                api_key_encrypted = CASE WHEN $4 THEN $3 ELSE web_search_providers.api_key_encrypted END,
                config            = COALESCE($5::jsonb, web_search_providers.config),
                updated_at        = NOW()
            "#,
            provider,
            plaintext,
            encrypted,
            key_provided,
            config,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    // ── Per-user provider keys ────────────────────────────────────────────────
    // Mirror `llm_provider::UserKeyRepository`: a user's own key for a provider,
    // resolved FIRST at search time with the deployment key as the fallback.

    /// All of a user's provider keys, decrypted, as a `provider -> key` map for
    /// request-time resolution. One query (no N+1 across the chain).
    pub async fn list_user_keys_raw(
        &self,
        user_id: Uuid,
    ) -> Result<HashMap<String, String>, AppError> {
        let rows = sqlx::query!(
            r#"
            SELECT provider, api_key, api_key_encrypted FROM user_web_search_provider_keys
            WHERE user_id = $1
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        let mut map = HashMap::with_capacity(rows.len());
        for r in rows {
            if let Some(key) =
                resolve_optional_secret(&self.pool, r.api_key_encrypted, r.api_key).await
            {
                map.insert(r.provider, key);
            }
        }
        Ok(map)
    }

    /// Save or update a user's key for a provider (encrypts when a storage key is
    /// configured; dev-mode plaintext otherwise). Race-safe via the
    /// `(user_id, provider)` unique index + `ON CONFLICT`.
    pub async fn upsert_user_key(
        &self,
        user_id: Uuid,
        provider: &str,
        api_key: &str,
    ) -> Result<(), AppError> {
        let (plaintext, encrypted): (Option<&str>, Option<Vec<u8>>) =
            match encrypt_secret(&self.pool, api_key, storage_key()).await? {
                Some(blob) => (None, Some(blob)),
                None => (Some(api_key), None),
            };

        sqlx::query!(
            r#"
            INSERT INTO user_web_search_provider_keys (user_id, provider, api_key, api_key_encrypted)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (user_id, provider)
            DO UPDATE SET api_key = EXCLUDED.api_key,
                          api_key_encrypted = EXCLUDED.api_key_encrypted,
                          updated_at = NOW()
            "#,
            user_id,
            provider,
            plaintext,
            encrypted.as_deref()
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    /// Delete a user's key for a provider.
    pub async fn delete_user_key(&self, user_id: Uuid, provider: &str) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            DELETE FROM user_web_search_provider_keys
            WHERE user_id = $1 AND provider = $2
            "#,
            user_id,
            provider
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    /// A user's stored keys in MASKED form (`provider -> first-4 + ***`) — the
    /// only shape ever returned to the API. Never emits plaintext.
    pub async fn list_user_keys_masked(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<UserProviderKeyEntry>, AppError> {
        let rows = sqlx::query!(
            r#"
            SELECT provider, api_key, api_key_encrypted FROM user_web_search_provider_keys
            WHERE user_id = $1
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        let mut entries = Vec::with_capacity(rows.len());
        for r in rows {
            let resolved =
                resolve_optional_secret(&self.pool, r.api_key_encrypted, r.api_key).await;
            entries.push(UserProviderKeyEntry {
                provider: r.provider,
                masked_key: mask_secret(resolved.as_deref()),
            });
        }
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    /// `upsert_builtin_server` is the boot-time registrar for the built-in
    /// `web_search.ziee.internal` row; it runs in a `tokio::spawn` on EVERY
    /// server start (`web_search/mod.rs:95-103`), so a restart/re-register must
    /// stay consistent: one row keyed on `id`, with the loopback `url` (whose
    /// ephemeral port changes across restarts) re-asserted via the audited
    /// `ON CONFLICT (id) DO UPDATE` contract (repository.rs:58-63) — never a
    /// unique-violation, never a duplicate row.
    ///
    /// DB-gated: soft-skips (mirroring the suite's env-gated real-stack tests,
    /// e.g. `memory::reaper`) when `DATABASE_URL` is unset / unreachable, so
    /// `cargo test --lib` without Postgres stays green; runs for real wherever
    /// `DATABASE_URL` points at a migrated DB.
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

        let repo = WebSearchRepository::new(pool.clone());
        // A per-test id so parallel in-source tests can't collide on the row
        // (and so we never touch the real boot-registered web_search row).
        let server_id = Uuid::new_v4();

        // First boot: inserts the row at port 41111.
        repo.upsert_builtin_server(server_id, "http://127.0.0.1:41111/api/web-search/mcp")
            .await
            .expect("first upsert (insert) must succeed");

        // Second boot / re-register, SAME id, DIFFERENT loopback port (the real
        // cross-restart case): must NOT unique-violate, must NOT duplicate, and
        // must re-assert the new url.
        repo.upsert_builtin_server(server_id, "http://127.0.0.1:42222/api/web-search/mcp")
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

        // The conflict branch re-asserted the new loopback url + identity flags
        // (the columns the ON CONFLICT clause sets).
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
            Some("http://127.0.0.1:42222/api/web-search/mcp"),
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
