// Global application state
// Provides access to app-wide configuration like data_dir

use once_cell::sync::Lazy;
use std::path::PathBuf;
use std::sync::Mutex;

/// Global application data directory
/// This stores models, caches, temporary files, etc.
pub static APP_DATA_DIR: Lazy<Mutex<PathBuf>> = Lazy::new(|| {
    // Default to ~/.ziee if not set
    let default_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ziee");
    Mutex::new(default_path)
});

/// Set the application data directory
/// This should be called once during application initialization from the config
pub fn set_app_data_dir(path: PathBuf) {
    if let Ok(mut app_data_dir) = APP_DATA_DIR.lock() {
        *app_data_dir = path;
        tracing::info!(
            "Application data directory set to: {}",
            app_data_dir.display()
        );
    } else {
        tracing::error!("Failed to lock APP_DATA_DIR mutex");
    }
}

/// Get the current application data directory
/// Returns a cloned PathBuf to avoid holding the mutex lock
pub fn get_app_data_dir() -> PathBuf {
    APP_DATA_DIR
        .lock()
        .expect("Failed to lock APP_DATA_DIR")
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_data_dir() {
        let test_path = PathBuf::from("/tmp/test-ziee-chat");
        set_app_data_dir(test_path.clone());
        let retrieved_path = get_app_data_dir();
        assert_eq!(retrieved_path, test_path);
    }
}
