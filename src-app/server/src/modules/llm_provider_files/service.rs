//! Service layer for LLM provider file operations

use super::{models::*, repository};
use crate::{
    common::AppError,
    modules::{
        file::{storage::FileStorage, FileRepository},
        llm_provider::models::LlmProvider,
    },
};
use ai_providers::{AIProvider, FileUpload, FileUploadResponse};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

/// Get or upload file to provider Files API
///
/// This function implements the test-and-validate approach for API key rotation:
/// 1. Check for existing mapping
/// 2. If found and not expired, return provider file ID
/// 3. If not found or expired, upload to provider
/// 4. Save/update mapping
///
/// # Arguments
/// * `pool` - Database connection pool
/// * `file_repo` - File repository for database operations
/// * `file_storage` - File storage instance
/// * `file_id` - System file ID
/// * `provider` - LLM provider configuration
/// * `ai_provider` - AI provider implementation (from ai-providers crate)
///
/// # Returns
/// Provider file ID or error
pub async fn get_or_upload_provider_file(
    pool: &PgPool,
    file_repo: &FileRepository,
    file_storage: &Arc<dyn FileStorage>,
    file_id: Uuid,
    user_id: Uuid,
    provider: &LlmProvider,
    ai_provider: &dyn AIProvider,
) -> Result<String, AppError> {
    // 1. Check if provider supports file API
    if !ai_provider.supports_file_api() {
        return Err(AppError::bad_request(
            "PROVIDER_NO_FILE_API",
            format!(
                "Provider '{}' does not support file uploads",
                provider.name
            ),
        ));
    }

    // Extract API key early — needed for the key-rotation fingerprint
    // comparison on cache hit AND for the upload path on cache miss.
    let api_key = provider
        .api_key
        .as_ref()
        .ok_or_else(|| {
            AppError::bad_request("PROVIDER_NO_API_KEY", "Provider has no API key configured")
        })?;

    let current_key_fingerprint = api_key_fingerprint(api_key);

    // 2. Check for existing mapping. Scoped by user_id — closes
    // 06-llm-provider F-04 (defense-in-depth even though file_id is
    // globally unique, the JOIN to files makes cross-tenant access
    // structurally impossible if file_id ever leaks via another bug).
    if let Some(mapping) =
        repository::get_provider_file_mapping(pool, file_id, provider.id, user_id).await?
    {
        // 2a. Check if expired (Gemini 48h TTL) — computed from the mapping we
        //     just loaded, no redundant re-query.
        let is_expired = repository::is_mapping_expired(&mapping);

        // 2b. Detect API key rotation: if the stored fingerprint doesn't
        //     match the current key, the cached provider_file_id belongs to
        //     a different account and must be discarded.
        let key_rotated = mapping
            .provider_metadata
            .get("api_key_fingerprint")
            .and_then(|v| v.as_str())
            != Some(&current_key_fingerprint);

        if !is_expired && !key_rotated && mapping.upload_status == UploadStatus::Completed
            && let Some(provider_file_id) = mapping.provider_file_id {
                // Valid mapping exists - return it
                // Note: If provider returns "not found" error later, the caller
                // should handle re-upload (test-and-validate approach)
                return Ok(provider_file_id);
            }
    }

    // 3. No valid mapping - need to upload

    // Load file from storage
    let file = file_repo
        .get_by_id(file_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;

    let extension = get_extension(&file.filename);
    // Load the HEAD version's blob (`blob_version_id`), NOT `file_id` (= v1's
    // blob) — else a provider upload sends the stale original of an edited file.
    let file_data = file_storage
        .load_original(file.user_id, file.blob_version_id, &extension)
        .await?;

    // Create upload request
    let upload = FileUpload {
        filename: file.filename.clone(),
        file_data,
        mime_type: file.mime_type.clone().unwrap_or_else(|| {
            mime_guess::from_path(&file.filename)
                .first_or_octet_stream()
                .to_string()
        }),
    };

    // Get base URL
    let base_url = provider.base_url.as_deref().unwrap_or({
        // Default base URLs for known providers
        match provider.provider_type.as_str() {
            "anthropic" => "https://api.anthropic.com/v1",
            "gemini" => "https://generativelanguage.googleapis.com/v1beta",
            "openai" => "https://api.openai.com/v1",
            _ => "http://localhost:8000/v1",
        }
    });

    // Upload to provider
    let upload_response = ai_provider
        .upload_file(api_key, base_url, upload)
        .await
        .map_err(|e| AppError::internal_error(format!("Provider upload failed: {}", e)))?
        .ok_or_else(|| AppError::internal_error("Provider returned no file ID"))?;

    // Save mapping
    let provider_file_id = save_upload_response(
        pool,
        file_id,
        provider.id,
        &file.filename,
        api_key,
        upload_response,
    )
    .await?;

    Ok(provider_file_id)
}

/// Save upload response to database
async fn save_upload_response(
    pool: &PgPool,
    file_id: Uuid,
    provider_id: Uuid,
    filename: &str,
    api_key: &str,
    upload_response: FileUploadResponse,
) -> Result<String, AppError> {
    let mut metadata = upload_response.metadata.unwrap_or_default();
    metadata["uploaded_at"] = serde_json::json!(chrono::Utc::now().to_rfc3339());
    metadata["filename"] = serde_json::json!(filename);
    metadata["api_key_fingerprint"] = serde_json::json!(api_key_fingerprint(api_key));

    if let Some(expires_at) = upload_response.expires_at {
        metadata["expires_at"] = serde_json::json!(expires_at.to_rfc3339());
    }

    repository::upsert_provider_file_mapping(
        pool,
        file_id,
        provider_id,
        upload_response.provider_file_id.clone(),
        metadata,
    )
    .await?;

    Ok(upload_response.provider_file_id)
}

/// Compute a SHA-256 fingerprint of the API key for rotation detection.
/// Stored in `provider_metadata` so cache lookups can detect that the
/// admin has changed the provider's API key (which typically points to
/// a different account whose file IDs are invalid with the new key).
fn api_key_fingerprint(api_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(api_key.as_bytes());
    hex::encode(hasher.finalize())
}

/// Helper function to extract file extension. Delegates to the canonical
/// `extension_of` (rsplit + lowercase) so the load key matches how `upload.rs`
/// named the blob — `Path::extension` disagrees for dotfiles / no-extension
/// names and would 404 the load.
fn get_extension(filename: &str) -> String {
    crate::modules::file::utils::extension_of(filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_extension() {
        assert_eq!(get_extension("test.pdf"), "pdf");
        assert_eq!(get_extension("image.jpeg"), "jpeg");
        assert_eq!(get_extension("document.tar.gz"), "gz");
        // Per `file::utils::extension_of`'s documented contract, a dot-less
        // name yields the WHOLE (lowercased) name — so the on-disk blob key
        // matches how `upload` wrote it — NOT "".
        assert_eq!(get_extension("noext"), "noext");
    }

    // ── API-key rotation detection (cached provider_file_id invalidation) ──────

    #[test]
    fn api_key_fingerprint_is_stable_for_same_key() {
        // The cache-reuse path compares the stored fingerprint against the
        // current key's fingerprint; the SAME key must yield the SAME
        // fingerprint so a valid cached provider_file_id is reused.
        let k = "sk-abc123";
        assert_eq!(api_key_fingerprint(k), api_key_fingerprint(k));
        // SHA-256 hex is 64 chars.
        assert_eq!(api_key_fingerprint(k).len(), 64);
        assert!(api_key_fingerprint(k).bytes().all(|b| b.is_ascii_hexdigit()));
    }

    #[test]
    fn api_key_fingerprint_differs_after_rotation() {
        // A rotated key must produce a DIFFERENT fingerprint so `key_rotated`
        // detects the rotation and discards the stale cached provider_file_id
        // (which belongs to the previous account).
        let old = api_key_fingerprint("sk-old-account-key");
        let new = api_key_fingerprint("sk-new-account-key");
        assert_ne!(old, new, "rotation must change the fingerprint");
        // The comparison the service uses: stored != current ⇒ invalidate.
        assert!(old != new, "stored fingerprint != current ⇒ cache invalidated");
    }
}
