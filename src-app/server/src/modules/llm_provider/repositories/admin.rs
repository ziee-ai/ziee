// Provider repository

// LLM Provider database queries - copied from react-test and refactored for ziee
// Source: react-test/src-tauri/src/database/queries/providers.rs and user_group_providers.rs

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::super::models::LlmProvider;
use super::super::types::{CreateLlmProviderRequest, UpdateLlmProviderRequest};
use crate::common::secret::{encrypt_secret, resolve_optional_secret};
use crate::core::secrets::storage_key;
// Group-related helpers (get_provider_groups, assign_to_group,
// remove_from_group, get_for_group, get_for_user, user_has_access_to_provider)
// moved to `user_extension/repository.rs` (llm_provider↔user/Group
// inversion). Access via `Repos.user_group_llm_provider.*`.

/// Convert a `time::OffsetDateTime` (sqlx return type) to
/// `chrono::DateTime<Utc>` with full nanosecond precision and without
/// `unwrap()`. Closes 06-llm-provider F-11 (Medium): the previous
/// `from_timestamp(.., 0).unwrap_or_else(Utc::now)` truncated sub-second precision
/// AND panicked on out-of-range timestamps. Falls back to the unix
/// epoch on the (currently-impossible) overflow path so the row still
/// renders rather than 500-ing the whole response.
fn to_chrono(ts: time::OffsetDateTime) -> DateTime<Utc> {
    DateTime::from_timestamp_nanos(ts.unix_timestamp_nanos() as i64)
}

/// Read-time injection for local providers: replace the stored
/// `base_url` (which is NULL for locals) with the live URL derived
/// from the server's listen config + `LOCAL_PROXY_PATH`. This is
/// the single source of truth for chat code's outbound call. If
/// `LOCAL_PROXY_PATH` ever changes (e.g. `/v1` → `/v2`), all
/// existing local provider rows pick up the new URL on the next
/// read — zero migrations needed.
///
/// Called from every read site (get_by_id, list, list_local_providers).
fn inject_runtime_fields(p: &mut LlmProvider) {
    if p.provider_type == "local" {
        let (host, port, api_prefix) = crate::core::get_server_addr();
        p.base_url = Some(
            crate::modules::llm_local_runtime::proxy::derive_proxy_url(
                &host,
                port,
                &api_prefix,
            ),
        );
    }
}

// =====================================================
// Repository Struct
// =====================================================

#[derive(Clone, Debug)]
pub struct LlmProviderRepository {
    pool: PgPool,
}

impl LlmProviderRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_by_id(&self, provider_id: Uuid) -> Result<Option<LlmProvider>, sqlx::Error> {
        get_llm_provider_by_id(&self.pool, provider_id).await
    }

    pub async fn list(&self) -> Result<Vec<LlmProvider>, sqlx::Error> {
        list_llm_providers(&self.pool).await
    }

    pub async fn create(
        &self,
        request: CreateLlmProviderRequest,
    ) -> Result<LlmProvider, sqlx::Error> {
        create_llm_provider(&self.pool, request).await
    }

    pub async fn update(
        &self,
        provider_id: Uuid,
        request: UpdateLlmProviderRequest,
    ) -> Result<Option<LlmProvider>, sqlx::Error> {
        update_llm_provider(&self.pool, provider_id, request).await
    }

    pub async fn delete(&self, provider_id: Uuid) -> Result<Result<bool, String>, sqlx::Error> {
        delete_llm_provider(&self.pool, provider_id).await
    }

    // Group-related methods (get_provider_groups, assign_to_group,
    // remove_from_group, get_for_group, get_for_user,
    // user_has_access_to_provider) moved to
    // `user_extension::UserGroupLlmProviderRepository`. Access via
    // `Repos.user_group_llm_provider.*`.

    pub async fn list_local_providers(&self) -> Result<Vec<LlmProvider>, sqlx::Error> {
        list_local_providers(&self.pool).await
    }
}

// =====================================================
// Legacy Functions (kept for backwards compatibility)
// =====================================================

pub async fn get_llm_provider_by_id(
    pool: &PgPool,
    provider_id: Uuid,
) -> Result<Option<LlmProvider>, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT id, name, provider_type, enabled, api_key, api_key_encrypted, base_url, built_in, proxy_settings, created_at, updated_at,
                  default_runtime_version_id
         FROM llm_providers
         WHERE id = $1"#,
        provider_id
    )
    .fetch_optional(pool)
    .await?;

    let Some(r) = row else {
        return Ok(None);
    };
    // Decrypt-with-fallback: prefer the bytea column, fall back to
    // plaintext for rows written before A5 / when storage_key is
    // unconfigured. See common::secret::resolve_optional_secret.
    let api_key = resolve_optional_secret(pool, r.api_key_encrypted, r.api_key).await;
    let mut p = LlmProvider {
        id: r.id,
        name: r.name,
        provider_type: r.provider_type,
        enabled: r.enabled,
        api_key,
        base_url: r.base_url,
        built_in: r.built_in,
        proxy_settings: r
            .proxy_settings
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default(),
        created_at: to_chrono(r.created_at),
        updated_at: to_chrono(r.updated_at),
        default_runtime_version_id: r.default_runtime_version_id,
    };
    inject_runtime_fields(&mut p);
    Ok(Some(p))
}

pub async fn list_llm_providers(pool: &PgPool) -> Result<Vec<LlmProvider>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id, name, provider_type, enabled, api_key, api_key_encrypted, base_url, built_in, proxy_settings, created_at, updated_at,
                  default_runtime_version_id
         FROM llm_providers
         ORDER BY built_in DESC, name ASC"#
    )
    .fetch_all(pool)
    .await?;

    let mut providers = Vec::with_capacity(rows.len());
    for r in rows {
        let api_key = resolve_optional_secret(pool, r.api_key_encrypted, r.api_key).await;
        let mut p = LlmProvider {
            id: r.id,
            name: r.name,
            provider_type: r.provider_type,
            enabled: r.enabled,
            api_key,
            base_url: r.base_url,
            built_in: r.built_in,
            proxy_settings: r
                .proxy_settings
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default(),
            created_at: to_chrono(r.created_at),
            updated_at: to_chrono(r.updated_at),
            default_runtime_version_id: r.default_runtime_version_id,
        };
        inject_runtime_fields(&mut p);
        providers.push(p);
    }
    Ok(providers)
}

pub async fn list_local_providers(pool: &PgPool) -> Result<Vec<LlmProvider>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id, name, provider_type, enabled, api_key, api_key_encrypted, base_url, built_in, proxy_settings, created_at, updated_at,
                  default_runtime_version_id
         FROM llm_providers
         WHERE provider_type = 'local' AND enabled = true
         ORDER BY name ASC"#
    )
    .fetch_all(pool)
    .await?;

    let mut providers = Vec::with_capacity(rows.len());
    for r in rows {
        let api_key = resolve_optional_secret(pool, r.api_key_encrypted, r.api_key).await;
        let mut p = LlmProvider {
            id: r.id,
            name: r.name,
            provider_type: r.provider_type,
            enabled: r.enabled,
            api_key,
            base_url: r.base_url,
            built_in: r.built_in,
            proxy_settings: r
                .proxy_settings
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default(),
            created_at: to_chrono(r.created_at),
            updated_at: to_chrono(r.updated_at),
            default_runtime_version_id: r.default_runtime_version_id,
        };
        inject_runtime_fields(&mut p);
        providers.push(p);
    }
    Ok(providers)
}

pub async fn create_llm_provider(
    pool: &PgPool,
    request: CreateLlmProviderRequest,
) -> Result<LlmProvider, sqlx::Error> {
    let provider_id = Uuid::new_v4();
    let proxy_settings_json = serde_json::to_value(request.proxy_settings.unwrap_or_default())
        .unwrap_or(serde_json::json!({}));

    // Encrypt the api_key at rest when a storage_key is configured.
    // Storage layout (closes 06-llm-provider F-02 Critical):
    //   - encrypted column: `api_key_encrypted` BYTEA (preferred)
    //   - plaintext column: `api_key` TEXT (compat / not-yet-backfilled)
    // When storage_key is configured we write ONLY to the encrypted
    // column (plaintext stays NULL). When it's absent we write the
    // plaintext column (and a `tracing::warn` fires once at boot per
    // core::secrets::init_storage_key).
    let raw_key: Option<&str> = request
        .api_key
        .as_deref()
        .and_then(|k| if k.trim().is_empty() { None } else { Some(k) });

    let (plaintext_key, encrypted_key): (Option<&str>, Option<Vec<u8>>) = match raw_key {
        Some(key) => match encrypt_secret(pool, key, storage_key()).await {
            Ok(Some(blob)) => (None, Some(blob)),
            Ok(None) => (Some(key), None),
            Err(e) => {
                tracing::error!(error = ?e, "Failed to encrypt provider api_key; aborting create");
                return Err(sqlx::Error::Protocol(
                    "secret encryption failed".to_string(),
                ));
            }
        },
        None => (None, None),
    };

    let row = sqlx::query!(
        r#"INSERT INTO llm_providers (id, name, provider_type, enabled, api_key, api_key_encrypted, base_url, built_in, proxy_settings)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING id, name, provider_type, enabled, api_key, api_key_encrypted, base_url, built_in, proxy_settings, created_at, updated_at, default_runtime_version_id"#,
        provider_id,
        &request.name,
        &request.provider_type,
        request.enabled.unwrap_or(false),
        plaintext_key,
        encrypted_key.as_deref(),
        request.base_url.as_deref(),
        false, // Custom providers are never built-in
        proxy_settings_json
    )
    .fetch_one(pool)
    .await?;

    let resolved_api_key = resolve_optional_secret(pool, row.api_key_encrypted, row.api_key).await;

    let mut p = LlmProvider {
        id: row.id,
        name: row.name,
        provider_type: row.provider_type,
        enabled: row.enabled,
        api_key: resolved_api_key,
        base_url: row.base_url,
        built_in: row.built_in,
        proxy_settings: row
            .proxy_settings
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default(),
        default_runtime_version_id: row.default_runtime_version_id,
        created_at: to_chrono(row.created_at),
        updated_at: to_chrono(row.updated_at),
    };
    inject_runtime_fields(&mut p);
    Ok(p)
}

pub async fn update_llm_provider(
    pool: &PgPool,
    provider_id: Uuid,
    request: UpdateLlmProviderRequest,
) -> Result<Option<LlmProvider>, sqlx::Error> {
    // If no updates provided, return existing record
    if request.name.is_none()
        && request.enabled.is_none()
        && request.api_key.is_none()
        && request.base_url.is_none()
        && request.proxy_settings.is_none()
    {
        return get_llm_provider_by_id(pool, provider_id).await;
    }

    // Separate update for each optional field
    if let Some(name) = &request.name {
        sqlx::query!(
            "UPDATE llm_providers SET name = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            name,
            provider_id
        )
        .execute(pool)
        .await?;
    }

    if let Some(enabled) = request.enabled {
        sqlx::query!(
            "UPDATE llm_providers SET enabled = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            enabled,
            provider_id
        )
        .execute(pool)
        .await?;
    }

    if let Some(api_key) = &request.api_key {
        // Mirror create_llm_provider's dual-column strategy. When the
        // storage_key is configured, write the encrypted column and
        // explicitly NULL out the plaintext column so a previously-
        // plaintext row is migrated on next update. Closes
        // 06-llm-provider F-02 on the update path.
        let raw_key: Option<&str> = if api_key.trim().is_empty() {
            None
        } else {
            Some(api_key.as_str())
        };

        let (plaintext_key, encrypted_key): (Option<&str>, Option<Vec<u8>>) = match raw_key {
            Some(key) => match encrypt_secret(pool, key, storage_key()).await {
                Ok(Some(blob)) => (None, Some(blob)),
                Ok(None) => (Some(key), None),
                Err(e) => {
                    tracing::error!(error = ?e, "Failed to encrypt provider api_key on update");
                    return Err(sqlx::Error::Protocol(
                        "secret encryption failed".to_string(),
                    ));
                }
            },
            None => (None, None),
        };

        sqlx::query!(
            "UPDATE llm_providers \
             SET api_key = $1, api_key_encrypted = $2, updated_at = CURRENT_TIMESTAMP \
             WHERE id = $3",
            plaintext_key,
            encrypted_key.as_deref(),
            provider_id
        )
        .execute(pool)
        .await?;
    }

    if let Some(base_url) = &request.base_url {
        sqlx::query!(
            "UPDATE llm_providers SET base_url = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            Some(base_url),
            provider_id
        )
        .execute(pool)
        .await?;
    }

    if let Some(proxy_settings) = &request.proxy_settings {
        let proxy_settings_json =
            serde_json::to_value(proxy_settings).unwrap_or(serde_json::json!({}));
        sqlx::query!(
            "UPDATE llm_providers SET proxy_settings = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            proxy_settings_json,
            provider_id
        )
        .execute(pool)
        .await?;
    }

    // Return updated record
    get_llm_provider_by_id(pool, provider_id).await
}

pub async fn delete_llm_provider(
    pool: &PgPool,
    provider_id: Uuid,
) -> Result<Result<bool, String>, sqlx::Error> {
    // SECURITY: the original SELECT-then-DELETE was racy — a concurrent
    // UPDATE that flipped `built_in` to false (or the row being
    // recreated under the same UUID) between the two queries could
    // bypass the built-in guard. Atomic single-statement DELETE with
    // the `built_in = false` predicate eliminates the window. Closes
    // 06-llm-provider F-10 (Medium). The split return value
    // (Err("Cannot delete…") vs Ok(true/false)) is preserved so the
    // handler's existing error-shape doesn't change.
    let row = sqlx::query!(
        "DELETE FROM llm_providers WHERE id = $1 AND built_in = false RETURNING id",
        provider_id
    )
    .fetch_optional(pool)
    .await?;

    if row.is_some() {
        return Ok(Ok(true));
    }

    // Nothing deleted — distinguish "not found" from "exists but
    // built-in" with a second read-only query.
    let still_exists = sqlx::query_scalar!(
        "SELECT built_in FROM llm_providers WHERE id = $1",
        provider_id
    )
    .fetch_optional(pool)
    .await?;

    match still_exists {
        Some(true) => Ok(Err("Cannot delete built-in provider".to_string())),
        Some(false) => Ok(Ok(false)), // raced with another delete
        None => Ok(Ok(false)),         // provider not found
    }
}
