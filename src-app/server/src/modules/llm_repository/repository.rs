// LLM Repository database queries - copied from react-test and refactored for ziee-chat
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
    if let Some(ref enc) = encrypted {
        if let Some(key) = storage_key() {
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
}

// =====================================================
// Legacy Functions (kept for backwards compatibility)
// =====================================================

pub async fn get_llm_repository_by_id(
    pool: &PgPool,
    repository_id: Uuid,
) -> Result<Option<LlmRepository>, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT id, name, url, auth_type, auth_config, auth_config_encrypted, enabled, built_in, created_at, updated_at
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
    }))
}

pub async fn list_llm_repositories(pool: &PgPool) -> Result<Vec<LlmRepository>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id, name, url, auth_type, auth_config, auth_config_encrypted, enabled, built_in, created_at, updated_at
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
        });
    }
    Ok(repos)
}

pub async fn create_llm_repository(
    pool: &PgPool,
    request: CreateLlmRepositoryRequest,
) -> Result<LlmRepository, sqlx::Error> {
    let repository_id = Uuid::new_v4();
    let auth_config_json =
        serde_json::to_value(&request.auth_config).unwrap_or(serde_json::json!({}));

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
         RETURNING id, name, url, auth_type, auth_config, auth_config_encrypted, enabled, built_in, created_at, updated_at"#,
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
    })
}

pub async fn update_llm_repository(
    pool: &PgPool,
    repository_id: Uuid,
    request: UpdateLlmRepositoryRequest,
) -> Result<Option<LlmRepository>, sqlx::Error> {
    // Replace COALESCE with separate conditional updates
    if let Some(name) = &request.name {
        sqlx::query!(
            "UPDATE llm_repositories SET name = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            name,
            repository_id
        )
        .execute(pool)
        .await?;
    }

    if let Some(url) = &request.url {
        sqlx::query!(
            "UPDATE llm_repositories SET url = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            url,
            repository_id
        )
        .execute(pool)
        .await?;
    }

    if let Some(auth_type) = &request.auth_type {
        sqlx::query!(
            "UPDATE llm_repositories SET auth_type = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            auth_type,
            repository_id
        )
        .execute(pool)
        .await?;
    }

    if let Some(auth_config) = &request.auth_config {
        let auth_config_json = serde_json::to_value(auth_config).unwrap_or(serde_json::json!({}));
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
        .execute(pool)
        .await?;
    }

    if let Some(enabled) = request.enabled {
        sqlx::query!(
            "UPDATE llm_repositories SET enabled = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            enabled,
            repository_id
        )
        .execute(pool)
        .await?;
    }

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
        r#"SELECT id, name, url, auth_type, auth_config, auth_config_encrypted, enabled, built_in, created_at, updated_at
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
    }))
}
