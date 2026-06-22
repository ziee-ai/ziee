//! lit_search persistence: the singleton settings row, per-connector config/key
//! rows, and the idempotent built-in MCP server upsert. (The full-text cache
//! index methods live alongside in `fulltext::cache`.)

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::common::secret::{encrypt_secret, resolve_optional_secret};
use crate::core::secrets::storage_key;

use super::models::LitSearchSettings;

/// A connector's stored config with the API key DECRYPTED (for internal dispatch
/// + the configured-state check). Never serialized to the API.
#[derive(Debug, Clone)]
pub struct LitConnectorRow {
    pub connector: String,
    pub api_key: Option<String>,
    pub config: Value,
}

#[derive(Clone, Debug)]
pub struct LitSearchRepository {
    pool: PgPool,
}

impl LitSearchRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Idempotent upsert of the built-in lit_search MCP server row. On conflict
    /// only re-asserts identity columns (the loopback `url` carries the live port).
    pub async fn upsert_builtin_server(
        &self,
        server_id: Uuid,
        loopback_url: &str,
    ) -> Result<(), AppError> {
        // Wrapped in a transaction to match the web_search peer's
        // upsert_builtin_server exactly (matches-peers invariant).
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
                $1, NULL, 'lit_search', 'Literature Search',
                'Built-in scholarly literature search + screening (literature_search / fetch_paper_fulltext / dedup_records / verify_quote / fetch_references)',
                true, true, true,
                'http', $2, '{}'::jsonb,
                60, false, 'auto', 4,
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

    pub async fn get_settings(&self) -> Result<LitSearchSettings, AppError> {
        let row = sqlx::query_as!(
            LitSearchSettings,
            r#"
            SELECT
                enabled,
                enabled_connectors,
                max_results,
                per_source_limit,
                request_timeout_secs,
                completeness_estimate_enabled,
                updated_at as "updated_at: _"
            FROM lit_search_settings
            WHERE id = TRUE
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_settings(
        &self,
        enabled: Option<bool>,
        enabled_connectors: Option<Vec<String>>,
        max_results: Option<i32>,
        per_source_limit: Option<i32>,
        request_timeout_secs: Option<i32>,
        completeness_estimate_enabled: Option<bool>,
    ) -> Result<LitSearchSettings, AppError> {
        let row = sqlx::query_as!(
            LitSearchSettings,
            r#"
            UPDATE lit_search_settings SET
                enabled                       = COALESCE($1, enabled),
                enabled_connectors            = COALESCE($2::text[], enabled_connectors),
                max_results                   = COALESCE($3, max_results),
                per_source_limit              = COALESCE($4, per_source_limit),
                request_timeout_secs          = COALESCE($5, request_timeout_secs),
                completeness_estimate_enabled = COALESCE($6, completeness_estimate_enabled),
                updated_at                    = NOW()
            WHERE id = TRUE
            RETURNING
                enabled,
                enabled_connectors,
                max_results,
                per_source_limit,
                request_timeout_secs,
                completeness_estimate_enabled,
                updated_at as "updated_at: _"
            "#,
            enabled,
            enabled_connectors.as_deref(),
            max_results,
            per_source_limit,
            request_timeout_secs,
            completeness_estimate_enabled,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    /// All configured connectors, API keys decrypted for internal use.
    pub async fn list_connectors(&self) -> Result<Vec<LitConnectorRow>, AppError> {
        let rows = sqlx::query!(
            r#"
            SELECT connector, api_key, api_key_encrypted, config as "config!: Value"
            FROM lit_search_connectors
            ORDER BY connector
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let api_key = resolve_optional_secret(&self.pool, r.api_key_encrypted, r.api_key).await;
            out.push(LitConnectorRow {
                connector: r.connector,
                api_key,
                config: r.config,
            });
        }
        Ok(out)
    }

    /// Upsert one connector's config/key. `api_key`: None = leave; Some(None) =
    /// clear; Some(Some(k)) = set. `config`: None = leave; Some(v) = replace.
    pub async fn upsert_connector(
        &self,
        connector: &str,
        api_key: Option<Option<String>>,
        config: Option<Value>,
    ) -> Result<(), AppError> {
        let key_provided = api_key.is_some();
        let (plaintext, encrypted): (Option<String>, Option<Vec<u8>>) = match api_key {
            Some(Some(k)) => match encrypt_secret(&self.pool, &k, storage_key()).await? {
                Some(blob) => (None, Some(blob)),
                None => (Some(k), None), // dev fallback: no storage key configured
            },
            Some(None) | None => (None, None),
        };

        sqlx::query!(
            r#"
            INSERT INTO lit_search_connectors (connector, api_key, api_key_encrypted, config, created_at, updated_at)
            VALUES ($1, $2, $3, COALESCE($5::jsonb, '{}'::jsonb), NOW(), NOW())
            ON CONFLICT (connector) DO UPDATE SET
                api_key           = CASE WHEN $4 THEN $2 ELSE lit_search_connectors.api_key END,
                api_key_encrypted = CASE WHEN $4 THEN $3 ELSE lit_search_connectors.api_key_encrypted END,
                config            = COALESCE($5::jsonb, lit_search_connectors.config),
                updated_at        = NOW()
            "#,
            connector,
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
