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

/// Set the application data directory.
///
/// Closes 14-core F-25 + F-17 (Info / Low): poison recovery rather than
/// silent log-and-continue. If a previous holder panicked while
/// mutating APP_DATA_DIR, recover the inner value and overwrite it.
/// The data dir is set-once at boot so the recovery branch should
/// never fire in practice.
pub fn set_app_data_dir(path: PathBuf) {
    let mut guard = APP_DATA_DIR.lock().unwrap_or_else(|poisoned| {
        tracing::error!("APP_DATA_DIR mutex poisoned in set_app_data_dir; recovering");
        poisoned.into_inner()
    });
    *guard = path;
    tracing::info!("Application data directory set to: {}", guard.display());
}

/// Get the current application data directory.
/// Returns a cloned PathBuf to avoid holding the mutex lock.
/// Poison recovery same as set_app_data_dir (14-core F-17 + F-25).
pub fn get_app_data_dir() -> PathBuf {
    APP_DATA_DIR
        .lock()
        .unwrap_or_else(|poisoned| {
            tracing::error!("APP_DATA_DIR mutex poisoned in get_app_data_dir; recovering");
            poisoned.into_inner()
        })
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
