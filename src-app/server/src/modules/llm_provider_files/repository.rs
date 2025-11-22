//! Database repository for LLM provider file mappings

use super::models::*;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// Get provider file mapping
///
/// Returns the mapping between a system file and a provider's file ID.
pub async fn get_provider_file_mapping(
    pool: &PgPool,
    file_id: Uuid,
    provider_id: Uuid,
) -> Result<Option<LlmProviderFile>, sqlx::Error> {
    sqlx::query_as!(
        LlmProviderFile,
        r#"
        SELECT
            id, file_id, provider_id, provider_file_id,
            provider_metadata,
            upload_status as "upload_status: UploadStatus",
            created_at as "created_at: DateTime<Utc>",
            updated_at as "updated_at: DateTime<Utc>"
        FROM llm_provider_files
        WHERE file_id = $1 AND provider_id = $2
        "#,
        file_id,
        provider_id
    )
    .fetch_optional(pool)
    .await
}

/// Create or update provider file mapping (UPSERT)
///
/// Uses PostgreSQL's ON CONFLICT to either insert a new mapping or update an existing one.
/// This is idempotent and safe for concurrent calls.
pub async fn upsert_provider_file_mapping(
    pool: &PgPool,
    file_id: Uuid,
    provider_id: Uuid,
    provider_file_id: String,
    provider_metadata: serde_json::Value,
) -> Result<LlmProviderFile, sqlx::Error> {
    sqlx::query_as!(
        LlmProviderFile,
        r#"
        INSERT INTO llm_provider_files (
            file_id, provider_id, provider_file_id,
            provider_metadata, upload_status
        )
        VALUES ($1, $2, $3, $4, 'completed')
        ON CONFLICT (file_id, provider_id) DO UPDATE SET
            provider_file_id = EXCLUDED.provider_file_id,
            provider_metadata = EXCLUDED.provider_metadata,
            upload_status = 'completed',
            updated_at = NOW()
        RETURNING
            id, file_id, provider_id, provider_file_id,
            provider_metadata,
            upload_status as "upload_status: UploadStatus",
            created_at as "created_at: DateTime<Utc>",
            updated_at as "updated_at: DateTime<Utc>"
        "#,
        file_id,
        provider_id,
        provider_file_id,
        provider_metadata
    )
    .fetch_one(pool)
    .await
}

/// Check if file expired (for Gemini 48h TTL)
///
/// Checks the `expires_at` field in provider_metadata to determine if a file has expired.
pub async fn is_file_expired(
    pool: &PgPool,
    file_id: Uuid,
    provider_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let mapping = get_provider_file_mapping(pool, file_id, provider_id).await?;

    if let Some(mapping) = mapping {
        if let Some(expires_at_str) = mapping
            .provider_metadata
            .get("expires_at")
            .and_then(|v| v.as_str())
        {
            if let Ok(expires_at) = DateTime::parse_from_rfc3339(expires_at_str) {
                return Ok(Utc::now() > expires_at);
            }
        }
    }

    Ok(false)
}
