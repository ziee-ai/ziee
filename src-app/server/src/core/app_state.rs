// Global application state
// Provides access to app-wide configuration like data_dir

use once_cell::sync::Lazy;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::core::config::CachesConfig;

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

/// Global cache-paths config (Phase-2 path consolidation). Holds the
/// resolved values that `Config::resolve_paths` filled in from
/// `app.data_dir`. Used by handlers that need a cache dir but don't
/// have the full Config in scope (e.g. `BinaryManager::new` callers).
///
/// Default is an empty CachesConfig (all None) — set ONLY after
/// `Config::resolve_paths` has run, so by the time any handler reads
/// it the paths are guaranteed populated. Accessor methods on
/// CachesConfig panic if read on an unresolved instance.
pub static CACHES_CONFIG: Lazy<Mutex<CachesConfig>> = Lazy::new(|| Mutex::new(CachesConfig::default()));

pub fn set_caches_config(c: CachesConfig) {
    let mut guard = CACHES_CONFIG.lock().unwrap_or_else(|poisoned| {
        tracing::error!("CACHES_CONFIG mutex poisoned in set_caches_config; recovering");
        poisoned.into_inner()
    });
    *guard = c;
}

pub fn get_caches_config() -> CachesConfig {
    CACHES_CONFIG
        .lock()
        .unwrap_or_else(|poisoned| {
            tracing::error!("CACHES_CONFIG mutex poisoned in get_caches_config; recovering");
            poisoned.into_inner()
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_data_dir() {
        let test_path = PathBuf::from("/tmp/test-ziee");
        set_app_data_dir(test_path.clone());
        let retrieved_path = get_app_data_dir();
        assert_eq!(retrieved_path, test_path);
    }
}
