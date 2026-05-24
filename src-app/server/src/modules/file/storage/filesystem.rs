// Filesystem storage implementation

use super::{FileStorage, StorageResult};
use crate::common::AppError;
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

/// Reject reads that would follow a symlink. If the storage tree
/// somehow contains a symlink (planted by a co-located process, a
/// privilege escalation, or a future bug), refuse to read it rather
/// than silently following to arbitrary host paths. Closes
/// 05-file F-15 (Medium). NotFound is the same shape callers already
/// expect, so no surface-level change.
async fn reject_if_symlink(path: &Path) -> StorageResult<()> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(meta) if meta.file_type().is_symlink() => {
            tracing::error!(
                path = %path.display(),
                "Refusing to read storage path that is a symlink"
            );
            Err(AppError::not_found("File"))
        }
        // ENOENT → propagate as not_found later in the read call.
        // Other errors (permission denied, etc.) → propagate the same.
        _ => Ok(()),
    }
}

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
                .map_err(|e| {
                tracing::error!(error = %e, "ensure_dir failed");
                AppError::internal_error("Storage error")
            })?;
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
            .map_err(|e| {
                tracing::error!(error = %e, "save_original write failed");
                AppError::internal_error("Storage error")
            })?;

        Ok(path)
    }

    async fn save_text_page(
        &self,
        user_id: Uuid,
        file_id: Uuid,
        page_num: u32,
        text: &str,
    ) -> StorageResult<PathBuf> {
        let path = self.get_text_path(user_id, file_id, page_num);
        self.ensure_dir(&path).await?;

        fs::write(&path, text)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "save_text_page write failed");
                AppError::internal_error("Storage error")
            })?;

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
            .map_err(|e| {
                tracing::error!(error = %e, "save_image write failed");
                AppError::internal_error("Storage error")
            })?;

        Ok(path)
    }

    fn get_original_path(
        &self,
        user_id: Uuid,
        file_id: Uuid,
        extension: &str,
    ) -> PathBuf {
        // SECURITY: the extension flows from user input on upload. Without
        // sanitization, a value like 'x/../../<victim_uuid>/y.pdf' lets
        // create_dir_all + fs::write escape the per-user originals
        // directory and overwrite (or shadow) another user's file. Same
        // primitive on the read path. Closes 05-file F-03 (High).
        //
        // Allow only ASCII alphanumeric extensions (matches every legit
        // mime-type-derived extension). Anything else is replaced with
        // 'bin', which keeps the file storable but isolated.
        let safe_ext: String = if extension
            .chars()
            .all(|c| c.is_ascii_alphanumeric())
            && !extension.is_empty()
            && extension.len() <= 16
        {
            extension.to_ascii_lowercase()
        } else {
            "bin".to_string()
        };
        self.get_user_path(user_id, "originals")
            .join(format!("{}.{}", file_id, safe_ext))
    }

    fn get_text_path(&self, user_id: Uuid, file_id: Uuid, page_num: u32) -> PathBuf {
        self.get_user_path(user_id, "text")
            .join(file_id.to_string())
            .join(format!("page_{}.txt", page_num))
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
        reject_if_symlink(&path).await?;
        fs::read(&path)
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, "load_original failed");
                AppError::not_found("File")
            })
    }

    async fn load_text_page(&self, user_id: Uuid, file_id: Uuid, page_num: u32) -> StorageResult<String> {
        let path = self.get_text_path(user_id, file_id, page_num);
        reject_if_symlink(&path).await?;
        fs::read_to_string(&path)
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, page = page_num, "load_text_page failed");
                AppError::not_found("Text page")
            })
    }

    async fn load_preview(
        &self,
        user_id: Uuid,
        file_id: Uuid,
        page_num: u32,
    ) -> StorageResult<Vec<u8>> {
        let path = self.get_image_path(user_id, file_id, page_num, false);
        reject_if_symlink(&path).await?;
        fs::read(&path)
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, "load_preview failed");
                AppError::not_found("Preview")
            })
    }

    async fn load_thumbnail(&self, user_id: Uuid, file_id: Uuid) -> StorageResult<Vec<u8>> {
        let path = self.get_image_path(user_id, file_id, 1, true);
        reject_if_symlink(&path).await?;
        fs::read(&path)
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, "load_thumbnail failed");
                AppError::not_found("Thumbnail")
            })
    }

    async fn delete_all(&self, user_id: Uuid, file_id: Uuid) -> StorageResult<()> {
        // Delete from all possible locations
        let locations = vec![
            ("originals", None),
            ("text", Some(file_id.to_string())),   // Directory with text pages
            ("images", Some(file_id.to_string())), // Directory with image pages
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
