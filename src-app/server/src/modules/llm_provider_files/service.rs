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

    // 2. Check for existing mapping
    if let Some(mapping) =
        repository::get_provider_file_mapping(pool, file_id, provider.id).await?
    {
        // 2a. Check if expired (Gemini 48h TTL)
        let is_expired = repository::is_file_expired(pool, file_id, provider.id).await?;

        if !is_expired && mapping.upload_status == UploadStatus::Completed {
            if let Some(provider_file_id) = mapping.provider_file_id {
                // Valid mapping exists - return it
                // Note: If provider returns "not found" error later, the caller
                // should handle re-upload (test-and-validate approach)
                return Ok(provider_file_id);
            }
        }
    }

    // 3. No valid mapping - need to upload

    // Load file from storage
    let file = file_repo
        .get_by_id(file_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;

    let extension = get_extension(&file.filename);
    let file_data = file_storage
        .load_original(file.user_id, file_id, &extension)
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

    // Get API key and base URL
    let api_key = provider
        .api_key
        .as_ref()
        .ok_or_else(|| {
            AppError::bad_request("PROVIDER_NO_API_KEY", "Provider has no API key configured")
        })?;

    let base_url = provider.base_url.as_deref().unwrap_or_else(|| {
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
    upload_response: FileUploadResponse,
) -> Result<String, AppError> {
    let mut metadata = upload_response.metadata.unwrap_or_default();
    metadata["uploaded_at"] = serde_json::json!(chrono::Utc::now().to_rfc3339());
    metadata["filename"] = serde_json::json!(filename);

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

/// Delete file from provider and remove mapping
///
/// This is called when a file is deleted from the system.
pub async fn delete_provider_file(
    pool: &PgPool,
    file_id: Uuid,
    provider: &LlmProvider,
    ai_provider: &dyn AIProvider,
) -> Result<(), AppError> {
    // Get mapping
    let mapping = repository::get_provider_file_mapping(pool, file_id, provider.id).await?;

    if let Some(mapping) = mapping {
        if let Some(provider_file_id) = mapping.provider_file_id {
            // Delete from provider
            if let Some(api_key) = &provider.api_key {
                let base_url = provider.base_url.as_deref().unwrap_or_else(|| {
                    match provider.provider_type.as_str() {
                        "anthropic" => "https://api.anthropic.com/v1",
                        "gemini" => "https://generativelanguage.googleapis.com/v1beta",
                        "openai" => "https://api.openai.com/v1",
                        _ => "http://localhost:8000/v1",
                    }
                });

                // Ignore errors from provider deletion (file might already be deleted)
                let _ = ai_provider
                    .delete_file(api_key, base_url, &provider_file_id)
                    .await;
            }
        }

        // Delete mapping from database
        repository::delete_provider_file_mapping(pool, file_id, provider.id).await?;
    }

    Ok(())
}

/// Background job: Cleanup expired Gemini files
///
/// Should be run periodically (e.g., daily) to remove expired file mappings.
pub async fn cleanup_expired_files(pool: &PgPool) -> Result<u64, AppError> {
    let deleted_count = repository::delete_expired_mappings(pool).await?;
    Ok(deleted_count)
}

/// Invalidate all provider file mappings when API key changes
///
/// This marks all files as expired so they will be re-uploaded on next use.
pub async fn invalidate_provider_files(pool: &PgPool, provider_id: Uuid) -> Result<u64, AppError> {
    let invalidated_count = repository::invalidate_provider_mappings(pool, provider_id).await?;
    Ok(invalidated_count)
}

/// Helper function to extract file extension
fn get_extension(filename: &str) -> String {
    std::path::Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_extension() {
        assert_eq!(get_extension("test.pdf"), "pdf");
        assert_eq!(get_extension("image.jpeg"), "jpeg");
        assert_eq!(get_extension("document.tar.gz"), "gz");
        assert_eq!(get_extension("noext"), "");
    }
}
