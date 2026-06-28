// LLM Repository database queries - copied from react-test and refactored for ziee
// Source: react-test/src-tauri/src/database/queries/repositories.rs

use chrono::DateTime;
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::secret::encrypt_secret;
use crate::core::secrets::storage_key;

use super::{
    models::LlmRepository,
    types::{CreateLlmRepositoryRequest, UpdateLlmRepositoryRequest},
};

/// Resolve the auth_config JSON, preferring the encrypted column when
/// storage_key is configured. The encrypted column stores the
/// JSON-serialized auth_config (the same shape as the plaintext column,
/// just encrypted as a whole). Returns the JSON Value, or None when
/// both columns are absent / unparseable.
async fn resolve_auth_config(
    pool: &PgPool,
    encrypted: Option<Vec<u8>>,
    plaintext: Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    if let Some(ref enc) = encrypted
        && let Some(key) = storage_key() {
            match crate::common::secret::decrypt_secret(pool, enc, key).await {
                Ok(json_text) => {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json_text) {
                        return Some(v);
                    }
                    tracing::error!(
                        "Decrypted llm_repositories.auth_config_encrypted is not valid JSON"
                    );
                }
                Err(e) => {
                    tracing::error!(
                        error = ?e,
                        "Failed to decrypt llm_repositories.auth_config_encrypted; \
                         falling back to plaintext column"
                    );
                }
            }
        }
    plaintext
}

// =====================================================
// Repository Struct
// =====================================================

#[derive(Clone, Debug)]
pub struct LlmRepositoryRepository {
    pool: PgPool,
}

impl LlmRepositoryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_by_id(
        &self,
        repository_id: Uuid,
    ) -> Result<Option<LlmRepository>, sqlx::Error> {
        get_llm_repository_by_id(&self.pool, repository_id).await
    }

    pub async fn list(&self) -> Result<Vec<LlmRepository>, sqlx::Error> {
        list_llm_repositories(&self.pool).await
    }

    pub async fn find_by_url(&self, url: &str) -> Result<Option<LlmRepository>, sqlx::Error> {
        find_llm_repository_by_url(&self.pool, url).await
    }

    /// Per-repository credential presence (url -> has_credential) for the
    /// hub-models list hint. Decrypts via the normal read path so it matches the
    /// authoritative `LlmRepository::has_credential()` gate EXACTLY — an
    /// approximate "encrypted blob present == configured" check would let the UI
    /// show a model as configured while the gate then 422s on download. Repos are
    /// few (2 built-in + a handful of custom), so the per-row decrypt is cheap.
    pub async fn list_credential_presence(&self) -> Result<Vec<(String, bool)>, sqlx::Error> {
        Ok(list_llm_repositories(&self.pool)
            .await?
            .into_iter()
            .map(|r| {
                let present = r.has_credential();
                (r.url, present)
            })
            .collect())
    }

    pub async fn create(
        &self,
        request: CreateLlmRepositoryRequest,
    ) -> Result<LlmRepository, sqlx::Error> {
        create_llm_repository(&self.pool, request).await
    }

    pub async fn update(
        &self,
        repository_id: Uuid,
        request: UpdateLlmRepositoryRequest,
    ) -> Result<Option<LlmRepository>, sqlx::Error> {
        update_llm_repository(&self.pool, repository_id, request).await
    }

    pub async fn delete(&self, repository_id: Uuid) -> Result<Result<bool, String>, sqlx::Error> {
        delete_llm_repository(&self.pool, repository_id).await
    }

    /// Persist a connection-probe outcome. See free-function docs.
    pub async fn record_health_check(
        &self,
        repo_id: Uuid,
        status: &str,
        reason: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        record_health_check(&self.pool, repo_id, status, reason).await
    }

    /// Every `enabled = TRUE` repository for the boot-time probe.
    pub async fn list_enabled_for_health_check(&self) -> Result<Vec<LlmRepository>, sqlx::Error> {
        list_enabled_for_health_check(&self.pool).await
    }

    /// Used by the boot probe when the EventBus isn't available yet;
    /// foreground enable-transition reverts go through the normal
    /// update path so they can emit `AutoDisabled`.
    pub async fn disable_for_health_failure(&self, repo_id: Uuid) -> Result<(), sqlx::Error> {
        disable_for_health_failure(&self.pool, repo_id).await
    }

    /// Pool accessor for connection_health (which runs ad-hoc SQL via
    /// the same connection — mirrors how `mcp::connection_health`
    /// reaches the pool through `McpRepository::pool()`).
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

// =====================================================
// Legacy Functions (kept for backwards compatibility)
// =====================================================

pub async fn get_llm_repository_by_id(
    pool: &PgPool,
    repository_id: Uuid,
) -> Result<Option<LlmRepository>, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT id, name, url, auth_type, auth_config, auth_config_encrypted, enabled, built_in, created_at, updated_at,
                  last_health_check_at, last_health_check_status, last_health_check_reason
         FROM llm_repositories
         WHERE id = $1"#,
        repository_id
    )
    .fetch_optional(pool)
    .await?;

    let Some(r) = row else {
        return Ok(None);
    };
    let auth_value = resolve_auth_config(pool, r.auth_config_encrypted, r.auth_config).await;
    Ok(Some(LlmRepository {
        id: r.id,
        name: r.name,
        url: r.url,
        auth_type: r.auth_type,
        auth_config: auth_value
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default(),
        enabled: r.enabled,
        built_in: r.built_in,
        created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
        updated_at: DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0).unwrap(),
        last_health_check_at: r
            .last_health_check_at
            .and_then(|t| DateTime::from_timestamp(t.unix_timestamp(), 0)),
        last_health_check_status: r.last_health_check_status,
        last_health_check_reason: r.last_health_check_reason,
    }))
}

pub async fn list_llm_repositories(pool: &PgPool) -> Result<Vec<LlmRepository>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id, name, url, auth_type, auth_config, auth_config_encrypted, enabled, built_in, created_at, updated_at,
                  last_health_check_at, last_health_check_status, last_health_check_reason
         FROM llm_repositories
         ORDER BY built_in DESC, name ASC"#
    )
    .fetch_all(pool)
    .await?;

    let mut repos = Vec::with_capacity(rows.len());
    for r in rows {
        let auth_value = resolve_auth_config(pool, r.auth_config_encrypted, r.auth_config).await;
        repos.push(LlmRepository {
            id: r.id,
            name: r.name,
            url: r.url,
            auth_type: r.auth_type,
            auth_config: auth_value
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default(),
            enabled: r.enabled,
            built_in: r.built_in,
            created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
            updated_at: DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0).unwrap(),
            last_health_check_at: r
                .last_health_check_at
                .and_then(|t| DateTime::from_timestamp(t.unix_timestamp(), 0)),
            last_health_check_status: r.last_health_check_status,
            last_health_check_reason: r.last_health_check_reason,
        });
    }
    Ok(repos)
}

pub async fn create_llm_repository(
    pool: &PgPool,
    request: CreateLlmRepositoryRequest,
) -> Result<LlmRepository, sqlx::Error> {
    let repository_id = Uuid::new_v4();
    // Use to_storage_value (NOT serde_json::to_value) so the secret fields
    // (api_key/password/token) are actually persisted — they are
    // `skip_serializing` on the entity to hide them in API responses.
    let auth_config_json = request
        .auth_config
        .as_ref()
        .map(|c| c.to_storage_value())
        .unwrap_or_else(|| serde_json::json!({}));

    // Encrypt the whole auth_config JSON blob. When storage_key is set,
    // ciphertext goes into auth_config_encrypted (bytea) and the
    // plaintext JSONB column is replaced with an empty `{}` so DB
    // dumps cannot leak credentials. Closes 09-llm-repository F-02
    // and the wiring half of 06-llm-provider F-02 (Critical).
    let serialized = auth_config_json.to_string();
    let (plaintext_value, encrypted_value): (serde_json::Value, Option<Vec<u8>>) =
        match encrypt_secret(pool, &serialized, storage_key()).await {
            Ok(Some(blob)) => (serde_json::json!({}), Some(blob)),
            Ok(None) => (auth_config_json.clone(), None),
            Err(e) => {
                tracing::error!(error = ?e, "Failed to encrypt llm_repositories.auth_config");
                return Err(sqlx::Error::Protocol(
                    "secret encryption failed".to_string(),
                ));
            }
        };

    let row = sqlx::query!(
        r#"INSERT INTO llm_repositories (id, name, url, auth_type, auth_config, auth_config_encrypted, enabled, built_in)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id, name, url, auth_type, auth_config, auth_config_encrypted, enabled, built_in, created_at, updated_at,
                   last_health_check_at, last_health_check_status, last_health_check_reason"#,
        repository_id,
        &request.name,
        &request.url,
        &request.auth_type,
        plaintext_value,
        encrypted_value.as_deref(),
        request.enabled.unwrap_or(true),
        false
    )
    .fetch_one(pool)
    .await?;

    let auth_value = resolve_auth_config(pool, row.auth_config_encrypted, row.auth_config).await;
    Ok(LlmRepository {
        id: row.id,
        name: row.name,
        url: row.url,
        auth_type: row.auth_type,
        auth_config: auth_value
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default(),
        enabled: row.enabled,
        built_in: row.built_in,
        created_at: DateTime::from_timestamp(row.created_at.unix_timestamp(), 0).unwrap(),
        updated_at: DateTime::from_timestamp(row.updated_at.unix_timestamp(), 0).unwrap(),
        last_health_check_at: row
            .last_health_check_at
            .and_then(|t| DateTime::from_timestamp(t.unix_timestamp(), 0)),
        last_health_check_status: row.last_health_check_status,
        last_health_check_reason: row.last_health_check_reason,
    })
}

pub async fn update_llm_repository(
    pool: &PgPool,
    repository_id: Uuid,
    request: UpdateLlmRepositoryRequest,
) -> Result<Option<LlmRepository>, sqlx::Error> {
    // Snapshot the PRE-update row ONCE, before mutating any column, so the
    // auth_config merge base and the auth_type-switch detection both see the
    // ORIGINAL auth_type (the auth_type column is rewritten below). This also
    // avoids re-reading + re-decrypting the row inside the auth_config branch.
    let existing = get_llm_repository_by_id(pool, repository_id).await?;

    // Apply all column updates atomically so a mid-update failure (e.g. after
    // auth_type is written but before auth_config) can't leave the row in an
    // inconsistent half-updated state. The independent secret-encryption call
    // below stays on `pool` — it's a stateless pgcrypto computation that doesn't
    // read the row under update, so it needn't share this transaction.
    let mut tx = pool.begin().await?;

    // Replace COALESCE with separate conditional updates
    if let Some(name) = &request.name {
        sqlx::query!(
            "UPDATE llm_repositories SET name = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            name,
            repository_id
        )
        .execute(&mut *tx)
        .await?;
    }

    if let Some(url) = &request.url {
        sqlx::query!(
            "UPDATE llm_repositories SET url = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            url,
            repository_id
        )
        .execute(&mut *tx)
        .await?;
    }

    if let Some(auth_type) = &request.auth_type {
        sqlx::query!(
            "UPDATE llm_repositories SET auth_type = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            auth_type,
            repository_id
        )
        .execute(&mut *tx)
        .await?;
    }

    if let Some(auth_config) = &request.auth_config {
        // Merge the incoming (partial) auth_config OVER the currently-stored one
        // so a partial update (e.g. changing only auth_test_api_endpoint or
        // toggling auth_type) does NOT wipe a previously-saved secret — the API/UI
        // treat an omitted api_key/password/token as "keep existing". Then
        // to_storage_value() (NOT serde_json::to_value) persists the secrets
        // instead of dropping them via their `skip_serializing`.
        let merged = match &existing {
            Some(existing) => {
                let merged = auth_config.merge_over(&existing.auth_config);
                // On an auth_type SWITCH (compared against the ORIGINAL auth_type
                // snapshotted before the column was rewritten), drop secret fields
                // belonging to the previous type so dead credential material
                // doesn't linger in the stored blob.
                match &request.auth_type {
                    Some(new_type) if *new_type != existing.auth_type => {
                        merged.pruned_for(new_type)
                    }
                    _ => merged,
                }
            }
            None => auth_config.clone(),
        };
        let auth_config_json = merged.to_storage_value();
        let serialized = auth_config_json.to_string();
        let (plaintext_value, encrypted_value): (serde_json::Value, Option<Vec<u8>>) =
            match encrypt_secret(pool, &serialized, storage_key()).await {
                Ok(Some(blob)) => (serde_json::json!({}), Some(blob)),
                Ok(None) => (auth_config_json.clone(), None),
                Err(e) => {
                    tracing::error!(
                        error = ?e,
                        "Failed to encrypt llm_repositories.auth_config on update"
                    );
                    return Err(sqlx::Error::Protocol(
                        "secret encryption failed".to_string(),
                    ));
                }
            };

        sqlx::query!(
            "UPDATE llm_repositories \
             SET auth_config = $1, auth_config_encrypted = $2, updated_at = CURRENT_TIMESTAMP \
             WHERE id = $3",
            plaintext_value,
            encrypted_value.as_deref(),
            repository_id
        )
        .execute(&mut *tx)
        .await?;
    }

    if let Some(enabled) = request.enabled {
        sqlx::query!(
            "UPDATE llm_repositories SET enabled = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            enabled,
            repository_id
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    // Fetch and return the updated repository
    get_llm_repository_by_id(pool, repository_id).await
}

pub async fn delete_llm_repository(
    pool: &PgPool,
    repository_id: Uuid,
) -> Result<Result<bool, String>, sqlx::Error> {
    // First check if repository exists and if it's built-in
    let built_in_result = sqlx::query_scalar!(
        "SELECT built_in FROM llm_repositories WHERE id = $1",
        repository_id
    )
    .fetch_optional(pool)
    .await?;

    match built_in_result {
        Some(built_in) => {
            if built_in {
                Ok(Err("Cannot delete built-in repository".to_string()))
            } else {
                let result =
                    sqlx::query!("DELETE FROM llm_repositories WHERE id = $1", repository_id)
                        .execute(pool)
                        .await?;
                Ok(Ok(result.rows_affected() > 0))
            }
        }
        None => Ok(Ok(false)),
    }
}

pub async fn find_llm_repository_by_url(
    pool: &PgPool,
    url: &str,
) -> Result<Option<LlmRepository>, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT id, name, url, auth_type, auth_config, auth_config_encrypted, enabled, built_in, created_at, updated_at,
                  last_health_check_at, last_health_check_status, last_health_check_reason
         FROM llm_repositories
         WHERE url = $1"#,
        url
    )
    .fetch_optional(pool)
    .await?;

    let Some(r) = row else {
        return Ok(None);
    };
    let auth_value = resolve_auth_config(pool, r.auth_config_encrypted, r.auth_config).await;
    Ok(Some(LlmRepository {
        id: r.id,
        name: r.name,
        url: r.url,
        auth_type: r.auth_type,
        auth_config: auth_value
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default(),
        enabled: r.enabled,
        built_in: r.built_in,
        created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
        updated_at: DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0).unwrap(),
        last_health_check_at: r
            .last_health_check_at
            .and_then(|t| DateTime::from_timestamp(t.unix_timestamp(), 0)),
        last_health_check_status: r.last_health_check_status,
        last_health_check_reason: r.last_health_check_reason,
    }))
}

// =====================================================
// Connection-health methods (migration 83)
// =====================================================

/// Persist the outcome of a connection probe. Called from four sites
/// in `connection_health.rs`: boot startup check, create-flow probe,
/// update-flow enable-transition probe, and the explicit form-based
/// test path. Stamps `last_health_check_at = CURRENT_TIMESTAMP` so the UI can
/// render a relative timestamp on the Alert.
///
/// Status must be one of `"healthy"` | `"unhealthy"` — the CHECK
/// constraint also accepts `"untested"`, but we never write that
/// value here (it's the column default). `reason` is required on
/// unhealthy and ignored on healthy (callers should pass `None`).
pub async fn record_health_check(
    pool: &PgPool,
    repo_id: Uuid,
    status: &str,
    reason: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE llm_repositories
           SET last_health_check_at = CURRENT_TIMESTAMP,
               last_health_check_status = $1,
               last_health_check_reason = $2
           WHERE id = $3"#,
        status,
        reason,
        repo_id,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Every enabled repository, in scan order for the boot probe. Unlike
/// MCP we do NOT exclude `built_in` rows — the seed HuggingFace +
/// GitHub repos are exactly the rows we want to probe (they share the
/// same connectivity codepath as user-added rows; no separate runtime
/// owns them).
pub async fn list_enabled_for_health_check(
    pool: &PgPool,
) -> Result<Vec<LlmRepository>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id, name, url, auth_type, auth_config, auth_config_encrypted, enabled, built_in, created_at, updated_at,
                  last_health_check_at, last_health_check_status, last_health_check_reason
         FROM llm_repositories
         WHERE enabled = TRUE
         ORDER BY built_in DESC, name ASC"#
    )
    .fetch_all(pool)
    .await?;

    let mut repos = Vec::with_capacity(rows.len());
    for r in rows {
        let auth_value = resolve_auth_config(pool, r.auth_config_encrypted, r.auth_config).await;
        repos.push(LlmRepository {
            id: r.id,
            name: r.name,
            url: r.url,
            auth_type: r.auth_type,
            auth_config: auth_value
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default(),
            enabled: r.enabled,
            built_in: r.built_in,
            created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
            updated_at: DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0).unwrap(),
            last_health_check_at: r
                .last_health_check_at
                .and_then(|t| DateTime::from_timestamp(t.unix_timestamp(), 0)),
            last_health_check_status: r.last_health_check_status,
            last_health_check_reason: r.last_health_check_reason,
        });
    }
    Ok(repos)
}

/// Set `enabled = FALSE` directly, bypassing the normal update
/// pipeline. Used by the boot path which runs BEFORE the EventBus
/// exists — emitting `AutoDisabled` there would be a no-op + log a
/// "no handlers" warning. The enable-transition path on the
/// foreground request handler instead goes through the normal update
/// + emits the event so the UI's list reloads in real time.
pub async fn disable_for_health_failure(
    pool: &PgPool,
    repo_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE llm_repositories
           SET enabled = FALSE, updated_at = CURRENT_TIMESTAMP
           WHERE id = $1"#,
        repo_id,
    )
    .execute(pool)
    .await?;
    Ok(())
}
