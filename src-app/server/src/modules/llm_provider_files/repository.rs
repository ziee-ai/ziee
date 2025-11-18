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

/// Delete expired mappings (background cleanup)
///
/// This should be run periodically (e.g., daily) to clean up expired Gemini files.
pub async fn delete_expired_mappings(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        DELETE FROM llm_provider_files
        WHERE (provider_metadata->>'expires_at')::TIMESTAMPTZ < NOW()
          AND upload_status = 'completed'
        "#
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// Invalidate all mappings for a provider (soft delete)
///
/// Called when a provider's configuration changes (e.g., API key rotation).
/// Marks all files as expired so they will be re-uploaded on next use.
pub async fn invalidate_provider_mappings(
    pool: &PgPool,
    provider_id: Uuid,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        UPDATE llm_provider_files
        SET
            upload_status = 'expired',
            provider_metadata = provider_metadata || jsonb_build_object(
                'invalidated_at', NOW(),
                'reason', 'provider_config_changed'
            ),
            updated_at = NOW()
        WHERE provider_id = $1
          AND upload_status = 'completed'
        "#,
        provider_id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// Get all active mappings (for background validation)
pub async fn get_all_active_mappings(pool: &PgPool) -> Result<Vec<LlmProviderFile>, sqlx::Error> {
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
        WHERE upload_status = 'completed'
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(pool)
    .await
}

/// Delete a specific mapping
pub async fn delete_provider_file_mapping(
    pool: &PgPool,
    file_id: Uuid,
    provider_id: Uuid,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        DELETE FROM llm_provider_files
        WHERE file_id = $1 AND provider_id = $2
        "#,
        file_id,
        provider_id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// Update upload status
pub async fn update_upload_status(
    pool: &PgPool,
    file_id: Uuid,
    provider_id: Uuid,
    status: UploadStatus,
    error_message: Option<String>,
) -> Result<(), sqlx::Error> {
    let mut metadata_update = serde_json::json!({
        "status_updated_at": Utc::now().to_rfc3339()
    });

    if let Some(error) = error_message {
        metadata_update["upload_error"] = serde_json::json!(error);
    }

    sqlx::query!(
        r#"
        UPDATE llm_provider_files
        SET
            upload_status = $3,
            provider_metadata = provider_metadata || $4,
            updated_at = NOW()
        WHERE file_id = $1 AND provider_id = $2
        "#,
        file_id,
        provider_id,
        status.to_string(),
        metadata_update
    )
    .execute(pool)
    .await?;

    Ok(())
}
