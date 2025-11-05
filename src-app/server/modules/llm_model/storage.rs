// Model storage utility for managing model files
// Adapted from react-test/src-tauri/src/utils/model_storage.rs

use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum ModelStorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Model already exists: {0}")]
    ModelAlreadyExists(String),
}

#[derive(Debug, Clone)]
pub struct TempFile {
    // Struct kept for API compatibility but fields removed as they were never read
}

pub struct ModelStorage {
    base_path: PathBuf,
}

impl ModelStorage {
    pub async fn new() -> Result<Self, ModelStorageError> {
        let app_data_path = crate::core::get_app_data_dir();
        let base_path = app_data_path.join("models");

        // Create models directory if it doesn't exist
        if !base_path.exists() {
            tracing::info!("Creating ModelStorage base directory: {}", base_path.display());
            tokio::fs::create_dir_all(&base_path).await.map_err(|e| {
                ModelStorageError::Io(std::io::Error::new(
                    e.kind(),
                    format!(
                        "Failed to create base directory {}: {}",
                        base_path.display(),
                        e
                    ),
                ))
            })?;
        }

        // Create temp directory at APP_DATA_DIR level
        let temp_base = app_data_path.join("temp");
        if !temp_base.exists() {
            tracing::info!("Creating temp directory: {}", temp_base.display());
            tokio::fs::create_dir_all(&temp_base).await.map_err(|e| {
                ModelStorageError::Io(std::io::Error::new(
                    e.kind(),
                    format!(
                        "Failed to create temp directory {}: {}",
                        temp_base.display(),
                        e
                    ),
                ))
            })?;
        }

        tracing::info!("ModelStorage initialized with base path: {}", base_path.display());
        tracing::debug!("Temp directory: {}", temp_base.display());
        Ok(Self { base_path })
    }

    /// Get the storage path for a specific provider and model
    pub fn get_model_path(&self, provider_id: &Uuid, model_id: &Uuid) -> PathBuf {
        self.base_path
            .join(provider_id.to_string())
            .join(model_id.to_string())
    }

    /// Create a new model directory
    pub async fn create_model_directory(
        &self,
        provider_id: &Uuid,
        model_id: &Uuid,
    ) -> Result<PathBuf, ModelStorageError> {
        let model_path = self.get_model_path(provider_id, model_id);

        if model_path.exists() {
            return Err(ModelStorageError::ModelAlreadyExists(format!(
                "Model directory already exists: {}",
                model_path.display()
            )));
        }

        tokio::fs::create_dir_all(&model_path).await?;
        Ok(model_path)
    }

    /// Save file to temporary storage
    pub async fn save_temp_file(
        &self,
        session_id: &Uuid,
        temp_file_id: &Uuid,
        filename: &str,
        data: &[u8],
    ) -> Result<TempFile, ModelStorageError> {
        // Save to APP_DATA_DIR/temp/session_id/safe_filename
        let temp_base = crate::core::get_app_data_dir().join("temp");
        let session_dir = temp_base.join(session_id.to_string());

        // Ensure session temp directory exists
        if !session_dir.exists() {
            tracing::debug!("Creating session temp directory: {}", session_dir.display());
            tokio::fs::create_dir_all(&session_dir).await.map_err(|e| {
                ModelStorageError::Io(std::io::Error::new(
                    e.kind(),
                    format!(
                        "Failed to create session temp directory {}: {}",
                        session_dir.display(),
                        e
                    ),
                ))
            })?;
        }

        // Sanitize filename to prevent path traversal
        let safe_filename = filename
            .replace('/', "_")
            .replace('\\', "_")
            .replace("..", "_");

        let file_path = session_dir.join(&safe_filename);
        tracing::debug!("Saving temp file to: {} ({} bytes)", file_path.display(), data.len());

        tokio::fs::write(&file_path, data).await.map_err(|e| {
            ModelStorageError::Io(std::io::Error::new(
                e.kind(),
                format!("Failed to write file {}: {}", file_path.display(), e),
            ))
        })?;

        // Create metadata file to map temp_file_id to original filename
        let metadata = serde_json::json!({
            "temp_file_id": temp_file_id,
            "filename": filename,
            "safe_filename": safe_filename,
            "size_bytes": data.len()
        });

        let metadata_path = session_dir.join(format!("{}.meta", temp_file_id));
        tokio::fs::write(&metadata_path, metadata.to_string())
            .await
            .map_err(|e| {
                ModelStorageError::Io(std::io::Error::new(
                    e.kind(),
                    format!(
                        "Failed to write metadata file {}: {}",
                        metadata_path.display(),
                        e
                    ),
                ))
            })?;

        tracing::info!(
            "Successfully saved temp file: {} ({} bytes)",
            file_path.display(),
            data.len()
        );

        Ok(TempFile {})
    }

    /// Clear all temporary files from the temp directory
    /// Called during app startup and shutdown to ensure clean state
    pub async fn clear_temp_directory() -> Result<(), ModelStorageError> {
        let temp_path = crate::core::get_app_data_dir().join("temp");

        if !temp_path.exists() {
            return Ok(()); // Nothing to clean up
        }

        tracing::info!("Clearing temp directory: {}", temp_path.display());

        // Remove all session directories and files in the temp directory
        let mut read_dir = tokio::fs::read_dir(&temp_path).await?;
        let mut removed_sessions = 0;
        let mut removed_files = 0;
        let mut error_count = 0;

        while let Some(entry) = read_dir.next_entry().await? {
            let entry_path = entry.path();
            let entry_type = entry.file_type().await?;

            if entry_type.is_dir() {
                // Remove session directory
                match tokio::fs::remove_dir_all(&entry_path).await {
                    Ok(()) => {
                        removed_sessions += 1;
                        tracing::debug!("Removed temp session directory: {}", entry_path.display());
                    }
                    Err(e) => {
                        error_count += 1;
                        tracing::error!(
                            "Failed to remove temp session directory {}: {}",
                            entry_path.display(),
                            e
                        );
                    }
                }
            } else {
                // Remove individual files (legacy flat structure)
                match tokio::fs::remove_file(&entry_path).await {
                    Ok(()) => {
                        removed_files += 1;
                        tracing::debug!("Removed temp file: {}", entry_path.display());
                    }
                    Err(e) => {
                        error_count += 1;
                        tracing::error!("Failed to remove temp file {}: {}", entry_path.display(), e);
                    }
                }
            }
        }

        if removed_sessions > 0 || removed_files > 0 {
            tracing::info!(
                "Temp directory cleanup complete: {} session directories and {} files removed",
                removed_sessions, removed_files
            );
        }
        if error_count > 0 {
            tracing::warn!("Temp directory cleanup had {} errors", error_count);
        }

        Ok(())
    }
}
