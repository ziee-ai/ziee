// Global file storage manager

use super::{filesystem::FilesystemStorage, FileStorage};
use once_cell::sync::OnceCell;
use std::sync::Arc;

/// Global file storage instance
static FILE_STORAGE: OnceCell<Arc<dyn FileStorage>> = OnceCell::new();

/// Initialize file storage
pub fn init_file_storage(base_path: impl AsRef<std::path::Path>) -> Arc<dyn FileStorage> {
    let storage = Arc::new(FilesystemStorage::new(base_path)) as Arc<dyn FileStorage>;
    FILE_STORAGE.set(storage.clone()).ok();
    storage
}

/// Get file storage instance
pub fn get_file_storage() -> Arc<dyn FileStorage> {
    FILE_STORAGE
        .get()
        .expect("File storage not initialized")
        .clone()
}
