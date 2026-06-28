//! Database repository for LLM provider file mappings

use super::models::*;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// Get provider file mapping, scoped to a specific user.
///
/// Returns the mapping between a system file and a provider's file ID.
/// SECURITY: the inner SELECT JOINs to `files` and filters by
/// `files.user_id = $3` so that even if an attacker somehow obtained
/// another user's file_id (UUIDs are unguessable but data might leak
/// via another bug), the cross-tenant access fails. Closes
/// 06-llm-provider F-04 (High).
pub async fn get_provider_file_mapping(
    pool: &PgPool,
    file_id: Uuid,
    provider_id: Uuid,
    user_id: Uuid,
) -> Result<Option<LlmProviderFile>, sqlx::Error> {
    sqlx::query_as!(
        LlmProviderFile,
        r#"
        SELECT
            lpf.id, lpf.file_id, lpf.provider_id, lpf.provider_file_id,
            lpf.provider_metadata,
            lpf.upload_status as "upload_status: UploadStatus",
            lpf.created_at as "created_at: DateTime<Utc>",
            lpf.updated_at as "updated_at: DateTime<Utc>"
        FROM llm_provider_files lpf
        INNER JOIN files f ON f.id = lpf.file_id
        WHERE lpf.file_id = $1 AND lpf.provider_id = $2 AND f.user_id = $3
        "#,
        file_id,
        provider_id,
        user_id
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

/// Check if an already-loaded mapping has expired (for Gemini 48h TTL).
///
/// Inspects the `expires_at` field in `provider_metadata`. Pure (no DB round
/// trip) so callers that already hold the mapping don't re-query it.
pub fn is_mapping_expired(mapping: &LlmProviderFile) -> bool {
    if let Some(expires_at_str) = mapping
        .provider_metadata
        .get("expires_at")
        .and_then(|v| v.as_str())
        && let Ok(expires_at) = DateTime::parse_from_rfc3339(expires_at_str)
    {
        return Utc::now() > expires_at;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn mapping_with_metadata(metadata: serde_json::Value) -> LlmProviderFile {
        LlmProviderFile {
            id: Uuid::new_v4(),
            file_id: Uuid::new_v4(),
            provider_id: Uuid::new_v4(),
            provider_file_id: Some("file_cached_123".to_string()),
            provider_metadata: metadata,
            upload_status: UploadStatus::Completed,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn cache_mapping_without_expiry_never_expires() {
        // No `expires_at` (e.g. Anthropic, which has no per-file TTL) → the
        // cached provider_file_id is reusable indefinitely.
        let m = mapping_with_metadata(json!({}));
        assert!(!is_mapping_expired(&m));
        let m2 = mapping_with_metadata(json!({ "api_key_fingerprint": "abc" }));
        assert!(!is_mapping_expired(&m2));
    }

    #[test]
    fn cache_mapping_with_future_expiry_is_valid() {
        // A Gemini-style mapping whose 48h TTL hasn't elapsed → still cached.
        let future = (Utc::now() + chrono::Duration::hours(10)).to_rfc3339();
        let m = mapping_with_metadata(json!({ "expires_at": future }));
        assert!(
            !is_mapping_expired(&m),
            "a not-yet-expired mapping must remain a cache hit"
        );
    }

    #[test]
    fn cache_mapping_past_expiry_is_invalidated() {
        // A mapping past its TTL → the cached id must NOT be reused (forces
        // re-upload).
        let past = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        let m = mapping_with_metadata(json!({ "expires_at": past }));
        assert!(is_mapping_expired(&m), "an expired mapping must invalidate the cache");
    }

    #[test]
    fn cache_mapping_malformed_expiry_does_not_panic_and_keeps_cache() {
        // An unparseable expires_at falls through to "not expired" rather than
        // panicking or eagerly discarding a usable cache entry.
        let m = mapping_with_metadata(json!({ "expires_at": "not-a-date" }));
        assert!(!is_mapping_expired(&m));
    }
}
