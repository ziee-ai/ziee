// File storage abstraction

pub mod filesystem;
pub mod manager;

use crate::common::AppError;
use async_trait::async_trait;
use std::path::PathBuf;
use uuid::Uuid;

/// Storage result type
pub type StorageResult<T> = Result<T, AppError>;

/// File storage operations
#[async_trait]
pub trait FileStorage: Send + Sync {
    /// Save original file
    async fn save_original(
        &self,
        user_id: Uuid,
        file_id: Uuid,
        extension: &str,
        data: &[u8],
    ) -> StorageResult<PathBuf>;

    /// Save extracted text
    async fn save_text(
        &self,
        user_id: Uuid,
        file_id: Uuid,
        text: &str,
    ) -> StorageResult<PathBuf>;

    /// Save image (page or thumbnail)
    async fn save_image(
        &self,
        user_id: Uuid,
        file_id: Uuid,
        page_num: u32,
        is_thumbnail: bool,
        data: &[u8],
    ) -> StorageResult<PathBuf>;

    /// Get original file path
    fn get_original_path(
        &self,
        user_id: Uuid,
        file_id: Uuid,
        extension: &str,
    ) -> PathBuf;

    /// Get text file path
    fn get_text_path(&self, user_id: Uuid, file_id: Uuid) -> PathBuf;

    /// Get image path
    fn get_image_path(
        &self,
        user_id: Uuid,
        file_id: Uuid,
        page_num: u32,
        is_thumbnail: bool,
    ) -> PathBuf;

    /// Load original file
    async fn load_original(
        &self,
        user_id: Uuid,
        file_id: Uuid,
        extension: &str,
    ) -> StorageResult<Vec<u8>>;

    /// Load text content
    async fn load_text(&self, user_id: Uuid, file_id: Uuid) -> StorageResult<String>;

    /// Load image
    async fn load_image(
        &self,
        user_id: Uuid,
        file_id: Uuid,
        page_num: u32,
        is_thumbnail: bool,
    ) -> StorageResult<Vec<u8>>;

    /// Delete all files for a file_id
    async fn delete_all(&self, user_id: Uuid, file_id: Uuid) -> StorageResult<()>;

    /// Calculate SHA-256 checksum
    fn calculate_checksum(&self, data: &[u8]) -> String;
}
