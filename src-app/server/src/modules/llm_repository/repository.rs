// LLM Repository database queries - copied from react-test and refactored for ziee-chat
// Source: react-test/src-tauri/src/database/queries/repositories.rs

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    models::{LlmRepository, RepositoryAuthConfig},
    types::{CreateLlmRepositoryRequest, UpdateLlmRepositoryRequest},
};

// =====================================================
// Repository Struct
// =====================================================

#[derive(Clone)]
pub struct LlmRepositoryRepository {
    pool: PgPool,
}

impl LlmRepositoryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_by_id(&self, repository_id: Uuid) -> Result<Option<LlmRepository>, sqlx::Error> {
        get_llm_repository_by_id(&self.pool, repository_id).await
    }

    pub async fn list(&self) -> Result<Vec<LlmRepository>, sqlx::Error> {
        list_llm_repositories(&self.pool).await
    }

    pub async fn create(&self, request: CreateLlmRepositoryRequest) -> Result<LlmRepository, sqlx::Error> {
        create_llm_repository(&self.pool, request).await
    }

    pub async fn update(&self, repository_id: Uuid, request: UpdateLlmRepositoryRequest) -> Result<Option<LlmRepository>, sqlx::Error> {
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
        r#"SELECT id, name, url, auth_type, auth_config, enabled, built_in, created_at, updated_at
         FROM llm_repositories
         WHERE id = $1"#,
        repository_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| LlmRepository {
        id: r.id,
        name: r.name,
        url: r.url,
        auth_type: r.auth_type,
        auth_config: r.auth_config
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
        r#"SELECT id, name, url, auth_type, auth_config, enabled, built_in, created_at, updated_at
         FROM llm_repositories
         ORDER BY built_in DESC, name ASC"#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| LlmRepository {
            id: r.id,
            name: r.name,
            url: r.url,
            auth_type: r.auth_type,
            auth_config: r.auth_config
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default(),
            enabled: r.enabled,
            built_in: r.built_in,
            created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
            updated_at: DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0).unwrap(),
        })
        .collect())
}

pub async fn create_llm_repository(
    pool: &PgPool,
    request: CreateLlmRepositoryRequest,
) -> Result<LlmRepository, sqlx::Error> {
    let repository_id = Uuid::new_v4();
    let auth_config_json = serde_json::to_value(&request.auth_config).unwrap_or(serde_json::json!({}));

    let row = sqlx::query!(
        r#"INSERT INTO llm_repositories (id, name, url, auth_type, auth_config, enabled, built_in)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id, name, url, auth_type, auth_config, enabled, built_in, created_at, updated_at"#,
        repository_id,
        &request.name,
        &request.url,
        &request.auth_type,
        auth_config_json,
        request.enabled.unwrap_or(true),
        false
    )
    .fetch_one(pool)
    .await?;

    Ok(LlmRepository {
        id: row.id,
        name: row.name,
        url: row.url,
        auth_type: row.auth_type,
        auth_config: row.auth_config
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
        sqlx::query!(
            "UPDATE llm_repositories SET auth_config = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            auth_config_json,
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
                let result = sqlx::query!("DELETE FROM llm_repositories WHERE id = $1", repository_id)
                    .execute(pool)
                    .await?;
                Ok(Ok(result.rows_affected() > 0))
            }
        }
        None => Ok(Ok(false)),
    }
}
