// Filesystem storage implementation

use super::{FileStorage, StorageResult};
use crate::common::AppError;
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

/// Filesystem-based file storage
#[derive(Debug, Clone)]
pub struct FilesystemStorage {
    base_path: PathBuf,
}

impl FilesystemStorage {
    /// Create new filesystem storage
    pub fn new(base_path: impl AsRef<Path>) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
        }
    }

    /// Ensure directory exists
    async fn ensure_dir(&self, path: &Path) -> StorageResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::internal_error(format!("Failed to create directory: {}", e)))?;
        }
        Ok(())
    }

    /// Get base path for user
    fn get_user_path(&self, user_id: Uuid, subdir: &str) -> PathBuf {
        self.base_path.join(subdir).join(user_id.to_string())
    }
}

#[async_trait]
impl FileStorage for FilesystemStorage {
    async fn save_original(
        &self,
        user_id: Uuid,
        file_id: Uuid,
        extension: &str,
        data: &[u8],
    ) -> StorageResult<PathBuf> {
        let path = self.get_original_path(user_id, file_id, extension);
        self.ensure_dir(&path).await?;

        fs::write(&path, data)
            .await
            .map_err(|e| AppError::internal_error(format!("Failed to write file: {}", e)))?;

        Ok(path)
    }

    async fn save_text(
        &self,
        user_id: Uuid,
        file_id: Uuid,
        text: &str,
    ) -> StorageResult<PathBuf> {
        let path = self.get_text_path(user_id, file_id);
        self.ensure_dir(&path).await?;

        fs::write(&path, text)
            .await
            .map_err(|e| AppError::internal_error(format!("Failed to write text: {}", e)))?;

        Ok(path)
    }

    async fn save_image(
        &self,
        user_id: Uuid,
        file_id: Uuid,
        page_num: u32,
        is_thumbnail: bool,
        data: &[u8],
    ) -> StorageResult<PathBuf> {
        let path = self.get_image_path(user_id, file_id, page_num, is_thumbnail);
        self.ensure_dir(&path).await?;

        fs::write(&path, data)
            .await
            .map_err(|e| AppError::internal_error(format!("Failed to write image: {}", e)))?;

        Ok(path)
    }

    fn get_original_path(
        &self,
        user_id: Uuid,
        file_id: Uuid,
        extension: &str,
    ) -> PathBuf {
        self.get_user_path(user_id, "originals")
            .join(format!("{}.{}", file_id, extension))
    }

    fn get_text_path(&self, user_id: Uuid, file_id: Uuid) -> PathBuf {
        self.get_user_path(user_id, "text")
            .join(format!("{}.txt", file_id))
    }

    fn get_image_path(
        &self,
        user_id: Uuid,
        file_id: Uuid,
        page_num: u32,
        is_thumbnail: bool,
    ) -> PathBuf {
        if is_thumbnail {
            // Single thumbnail: thumbnails/{user_id}/{file_id}.jpg
            self.get_user_path(user_id, "thumbnails")
                .join(format!("{}.jpg", file_id))
        } else {
            // Multiple images: images/{user_id}/{file_id}/page_N.jpg
            self.get_user_path(user_id, "images")
                .join(file_id.to_string())
                .join(format!("page_{}.jpg", page_num))
        }
    }

    async fn load_original(
        &self,
        user_id: Uuid,
        file_id: Uuid,
        extension: &str,
    ) -> StorageResult<Vec<u8>> {
        let path = self.get_original_path(user_id, file_id, extension);

        fs::read(&path)
            .await
            .map_err(|e| AppError::not_found(&format!("File not found: {}", e)))
    }

    async fn load_text(&self, user_id: Uuid, file_id: Uuid) -> StorageResult<String> {
        let path = self.get_text_path(user_id, file_id);

        fs::read_to_string(&path)
            .await
            .map_err(|e| AppError::not_found(&format!("Text content not found: {}", e)))
    }

    async fn load_image(
        &self,
        user_id: Uuid,
        file_id: Uuid,
        page_num: u32,
        is_thumbnail: bool,
    ) -> StorageResult<Vec<u8>> {
        let path = self.get_image_path(user_id, file_id, page_num, is_thumbnail);

        fs::read(&path)
            .await
            .map_err(|e| AppError::not_found(&format!("Image not found: {}", e)))
    }

    async fn delete_all(&self, user_id: Uuid, file_id: Uuid) -> StorageResult<()> {
        // Delete from all possible locations
        let locations = vec![
            ("originals", None),
            ("text", None),
            ("images", Some(file_id.to_string())), // Directory with multiple pages
        ];

        for (subdir, file_subdir) in locations {
            let mut path = self.get_user_path(user_id, subdir);
            if let Some(ref subdir_name) = file_subdir {
                path = path.join(subdir_name);
                // Delete entire directory
                if path.exists() {
                    let _ = fs::remove_dir_all(&path).await;
                }
            } else {
                // Delete files matching pattern
                if let Ok(mut entries) = fs::read_dir(&path).await {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.starts_with(&file_id.to_string()) {
                                let _ = fs::remove_file(entry.path()).await;
                            }
                        }
                    }
                }
            }
        }

        // Delete single thumbnail file: thumbnails/{user_id}/{file_id}.jpg
        let thumbnail_path = self.get_user_path(user_id, "thumbnails")
            .join(format!("{}.jpg", file_id));
        if thumbnail_path.exists() {
            let _ = fs::remove_file(&thumbnail_path).await;
        }

        Ok(())
    }

    fn calculate_checksum(&self, data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        hex::encode(result)
    }
}
