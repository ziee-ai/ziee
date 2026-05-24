// User LLM provider API key repository

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::common::secret::{encrypt_secret, resolve_optional_secret};
use crate::core::secrets::storage_key;

use super::super::types::UserApiKeyEntry;

#[derive(Clone, Debug)]
pub struct UserKeyRepository {
    pool: PgPool,
}

impl UserKeyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get raw API key for a user+provider (for inference use only).
    ///
    /// Returns the decrypted plaintext when api_key_encrypted is set
    /// and storage_key is configured; falls back to the legacy
    /// plaintext api_key column for not-yet-backfilled rows. Closes
    /// 06-llm-provider F-02 (Critical) on the per-user path.
    pub async fn get(
        &self,
        user_id: Uuid,
        provider_id: Uuid,
    ) -> Result<Option<String>, AppError> {
        let row = sqlx::query!(
            r#"
            SELECT api_key, api_key_encrypted FROM user_llm_provider_api_keys
            WHERE user_id = $1 AND provider_id = $2
            "#,
            user_id,
            provider_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        let Some(r) = row else {
            return Ok(None);
        };
        // get() returning Some("") would mean "user explicitly set empty
        // key" — preserve that distinction. resolve_optional_secret
        // returns None only when both columns are NULL.
        Ok(resolve_optional_secret(&self.pool, r.api_key_encrypted, r.api_key).await)
    }

    /// Save or update a user API key. When secrets.storage_key is
    /// configured, writes the ciphertext to api_key_encrypted and
    /// NULLs the plaintext column; otherwise writes plaintext (compat
    /// mode). Closes 06-llm-provider F-02 (Critical) on the write path.
    pub async fn upsert(
        &self,
        user_id: Uuid,
        provider_id: Uuid,
        api_key: &str,
    ) -> Result<(), AppError> {
        let (plaintext_key, encrypted_key): (Option<&str>, Option<Vec<u8>>) =
            match encrypt_secret(&self.pool, api_key, storage_key()).await? {
                Some(blob) => (None, Some(blob)),
                None => (Some(api_key), None),
            };

        sqlx::query!(
            r#"
            INSERT INTO user_llm_provider_api_keys (user_id, provider_id, api_key, api_key_encrypted)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (user_id, provider_id)
            DO UPDATE SET api_key = EXCLUDED.api_key,
                          api_key_encrypted = EXCLUDED.api_key_encrypted,
                          updated_at = NOW()
            "#,
            user_id,
            provider_id,
            plaintext_key,
            encrypted_key.as_deref()
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
            SELECT provider_id, api_key, api_key_encrypted FROM user_llm_provider_api_keys
            WHERE user_id = $1
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        let mut entries = Vec::with_capacity(rows.len());
        for r in rows {
            // Resolve the actual key (decrypted or plaintext fallback)
            // so the masked first-4-chars preview stays correct after
            // A5 encryption. Decryption failures fall back to None.
            let resolved =
                resolve_optional_secret(&self.pool, r.api_key_encrypted, r.api_key).await;
            let masked_key = match resolved.as_deref() {
                Some(key) if key.len() > 4 => format!("{}***", &key[..4]),
                _ => "***".to_string(),
            };
            entries.push(UserApiKeyEntry {
                provider_id: r.provider_id,
                masked_key,
            });
        }

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
