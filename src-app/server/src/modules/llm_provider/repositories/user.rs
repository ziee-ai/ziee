// User LLM provider API key repository

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

use super::super::types::UserApiKeyEntry;

#[derive(Clone, Debug)]
pub struct UserKeyRepository {
    pool: PgPool,
}

impl UserKeyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get raw API key for a user+provider (for inference use only)
    pub async fn get(
        &self,
        user_id: Uuid,
        provider_id: Uuid,
    ) -> Result<Option<String>, AppError> {
        let row = sqlx::query!(
            r#"
            SELECT api_key FROM user_llm_provider_api_keys
            WHERE user_id = $1 AND provider_id = $2
            "#,
            user_id,
            provider_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(row.map(|r| r.api_key))
    }

    /// Save or update a user API key
    pub async fn upsert(
        &self,
        user_id: Uuid,
        provider_id: Uuid,
        api_key: &str,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO user_llm_provider_api_keys (user_id, provider_id, api_key)
            VALUES ($1, $2, $3)
            ON CONFLICT (user_id, provider_id)
            DO UPDATE SET api_key = EXCLUDED.api_key, updated_at = NOW()
            "#,
            user_id,
            provider_id,
            api_key
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(())
    }

    /// Delete a user API key
    pub async fn delete(&self, user_id: Uuid, provider_id: Uuid) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            DELETE FROM user_llm_provider_api_keys
            WHERE user_id = $1 AND provider_id = $2
            "#,
            user_id,
            provider_id
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(())
    }

    /// List masked API keys for a user
    pub async fn list_for_user(&self, user_id: Uuid) -> Result<Vec<UserApiKeyEntry>, AppError> {
        let rows = sqlx::query!(
            r#"
            SELECT provider_id, api_key FROM user_llm_provider_api_keys
            WHERE user_id = $1
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        let entries = rows
            .into_iter()
            .map(|r| {
                let masked_key = if r.api_key.len() > 4 {
                    format!("{}***", &r.api_key[..4])
                } else {
                    "***".to_string()
                };
                UserApiKeyEntry {
                    provider_id: r.provider_id,
                    masked_key,
                }
            })
            .collect();

        Ok(entries)
    }

    /// Check if a user has a key for a provider
    pub async fn has_key(&self, user_id: Uuid, provider_id: Uuid) -> Result<bool, AppError> {
        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM user_llm_provider_api_keys
                WHERE user_id = $1 AND provider_id = $2
            ) as "exists!"
            "#,
            user_id,
            provider_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(exists)
    }
}
