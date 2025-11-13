// Provider repository
#![allow(dead_code)]

// LLM Provider database queries - copied from react-test and refactored for ziee-chat
// Source: react-test/src-tauri/src/database/queries/providers.rs and user_group_providers.rs

use chrono::DateTime;
use sqlx::PgPool;
use uuid::Uuid;

use super::models::LlmProvider;
use super::types::{CreateLlmProviderRequest, UpdateLlmProviderRequest};
use crate::modules::user::models::Group;

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

    pub async fn get_provider_groups(&self, provider_id: Uuid) -> Result<Vec<Group>, sqlx::Error> {
        get_llm_provider_groups(&self.pool, provider_id).await
    }

    pub async fn assign_to_group(
        &self,
        provider_id: Uuid,
        group_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        assign_provider_to_group(&self.pool, provider_id, group_id).await
    }

    pub async fn remove_from_group(
        &self,
        group_id: Uuid,
        provider_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        remove_provider_from_group(&self.pool, group_id, provider_id).await
    }

    pub async fn get_for_group(&self, group_id: Uuid) -> Result<Vec<LlmProvider>, sqlx::Error> {
        get_providers_for_group(&self.pool, group_id).await
    }

    pub async fn get_for_user(&self, user_id: Uuid) -> Result<Vec<LlmProvider>, sqlx::Error> {
        get_providers_for_user(&self.pool, user_id).await
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
        r#"SELECT id, name, provider_type, enabled, api_key, base_url, built_in, proxy_settings, created_at, updated_at
         FROM llm_providers
         WHERE id = $1"#,
        provider_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| LlmProvider {
        id: r.id,
        name: r.name,
        provider_type: r.provider_type,
        enabled: r.enabled,
        api_key: r.api_key,
        base_url: r.base_url,
        built_in: r.built_in,
        proxy_settings: r
            .proxy_settings
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default(),
        created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
        updated_at: DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0).unwrap(),
    }))
}

pub async fn list_llm_providers(pool: &PgPool) -> Result<Vec<LlmProvider>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id, name, provider_type, enabled, api_key, base_url, built_in, proxy_settings, created_at, updated_at
         FROM llm_providers
         ORDER BY built_in DESC, name ASC"#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| LlmProvider {
            id: r.id,
            name: r.name,
            provider_type: r.provider_type,
            enabled: r.enabled,
            api_key: r.api_key,
            base_url: r.base_url,
            built_in: r.built_in,
            proxy_settings: r
                .proxy_settings
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default(),
            created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
            updated_at: DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0).unwrap(),
        })
        .collect())
}

pub async fn create_llm_provider(
    pool: &PgPool,
    request: CreateLlmProviderRequest,
) -> Result<LlmProvider, sqlx::Error> {
    let provider_id = Uuid::new_v4();
    let proxy_settings_json = serde_json::to_value(&request.proxy_settings.unwrap_or_default())
        .unwrap_or(serde_json::json!({}));

    let row = sqlx::query!(
        r#"INSERT INTO llm_providers (id, name, provider_type, enabled, api_key, base_url, built_in, proxy_settings)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id, name, provider_type, enabled, api_key, base_url, built_in, proxy_settings, created_at, updated_at"#,
        provider_id,
        &request.name,
        &request.provider_type,
        request.enabled.unwrap_or(false),
        request.api_key.as_deref(),
        request.base_url.as_deref(),
        false, // Custom providers are never built-in
        proxy_settings_json
    )
    .fetch_one(pool)
    .await?;

    Ok(LlmProvider {
        id: row.id,
        name: row.name,
        provider_type: row.provider_type,
        enabled: row.enabled,
        api_key: row.api_key,
        base_url: row.base_url,
        built_in: row.built_in,
        proxy_settings: row
            .proxy_settings
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default(),
        created_at: DateTime::from_timestamp(row.created_at.unix_timestamp(), 0).unwrap(),
        updated_at: DateTime::from_timestamp(row.updated_at.unix_timestamp(), 0).unwrap(),
    })
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
        sqlx::query!(
            "UPDATE llm_providers SET api_key = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            Some(api_key),
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
    // First check if provider exists and if it's built-in
    let built_in_result = sqlx::query_scalar!(
        "SELECT built_in FROM llm_providers WHERE id = $1",
        provider_id
    )
    .fetch_optional(pool)
    .await?;

    match built_in_result {
        Some(built_in) => {
            if built_in {
                Ok(Err("Cannot delete built-in provider".to_string()))
            } else {
                let result = sqlx::query!("DELETE FROM llm_providers WHERE id = $1", provider_id)
                    .execute(pool)
                    .await?;
                Ok(Ok(result.rows_affected() > 0))
            }
        }
        None => Ok(Ok(false)), // Provider not found
    }
}

// =====================================================
// User Group Assignment Functions
// =====================================================

/// Get all groups that have access to a provider
pub async fn get_llm_provider_groups(
    pool: &PgPool,
    provider_id: Uuid,
) -> Result<Vec<Group>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT g.id, g.name, g.description, g.permissions, g.is_system, g.is_active, g.is_default, g.created_at, g.updated_at
         FROM groups g
         INNER JOIN user_group_llm_providers ugp ON g.id = ugp.group_id
         WHERE ugp.provider_id = $1
         ORDER BY g.name ASC"#,
        provider_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| Group {
            id: r.id,
            name: r.name,
            description: r.description,
            permissions: r.permissions,
            is_system: r.is_system,
            is_active: r.is_active,
            is_default: r.is_default,
            created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
            updated_at: DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0).unwrap(),
        })
        .collect())
}

/// Assign a provider to a user group
pub async fn assign_provider_to_group(
    pool: &PgPool,
    provider_id: Uuid,
    group_id: Uuid,
) -> Result<(), sqlx::Error> {
    // Check if the relationship already exists
    let existing = sqlx::query!(
        "SELECT id FROM user_group_llm_providers WHERE group_id = $1 AND provider_id = $2",
        group_id,
        provider_id
    )
    .fetch_optional(pool)
    .await?;

    if existing.is_some() {
        // Relationship already exists, return success
        return Ok(());
    }

    let relationship_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO user_group_llm_providers (id, group_id, provider_id) VALUES ($1, $2, $3)",
        relationship_id,
        group_id,
        provider_id
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Remove a provider from a user group
pub async fn remove_provider_from_group(
    pool: &PgPool,
    group_id: Uuid,
    provider_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        "DELETE FROM user_group_llm_providers WHERE group_id = $1 AND provider_id = $2",
        group_id,
        provider_id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Get all providers assigned to a user group
pub async fn get_providers_for_group(
    pool: &PgPool,
    group_id: Uuid,
) -> Result<Vec<LlmProvider>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT p.id, p.name, p.provider_type, p.enabled, p.api_key, p.base_url, p.built_in, p.proxy_settings, p.created_at, p.updated_at
         FROM llm_providers p
         INNER JOIN user_group_llm_providers ugp ON p.id = ugp.provider_id
         WHERE ugp.group_id = $1
         ORDER BY p.built_in DESC, p.name ASC"#,
        group_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| LlmProvider {
            id: r.id,
            name: r.name,
            provider_type: r.provider_type,
            enabled: r.enabled,
            api_key: r.api_key,
            base_url: r.base_url,
            built_in: r.built_in,
            proxy_settings: r
                .proxy_settings
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default(),
            created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
            updated_at: DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0).unwrap(),
        })
        .collect())
}

/// Get all providers available to a user based on their group memberships
/// Returns only enabled providers assigned to the user's active groups
pub async fn get_providers_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<LlmProvider>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT DISTINCT p.id, p.name, p.provider_type, p.enabled, p.api_key, p.base_url, p.built_in, p.proxy_settings, p.created_at, p.updated_at
         FROM llm_providers p
         INNER JOIN user_group_llm_providers ugp ON p.id = ugp.provider_id
         INNER JOIN user_groups ug ON ugp.group_id = ug.group_id
         INNER JOIN groups g ON ug.group_id = g.id
         WHERE ug.user_id = $1
           AND g.is_active = true
           AND p.enabled = true
         ORDER BY p.built_in DESC, p.name ASC"#,
        user_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| LlmProvider {
            id: r.id,
            name: r.name,
            provider_type: r.provider_type,
            enabled: r.enabled,
            api_key: r.api_key,
            base_url: r.base_url,
            built_in: r.built_in,
            proxy_settings: r
                .proxy_settings
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default(),
            created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
            updated_at: DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0).unwrap(),
        })
        .collect())
}
