//! web_search persistence: the singleton settings row, the per-provider
//! config/key rows, and the idempotent built-in MCP server upsert.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::common::secret::{encrypt_secret, resolve_optional_secret};
use crate::core::secrets::storage_key;

use super::models::WebSearchSettings;

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
}
