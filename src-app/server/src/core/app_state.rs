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

/// Server-side host + port + api_prefix, captured from `Config::server`
/// at boot. Used by the llm_local_runtime URL injection to derive the
/// proxy base_url at read time. The api_prefix matters because module
/// routes are nested under it (`app_builder.rs`), so the externally
/// reachable proxy URL is `http://host:port{api_prefix}/local-llm/v1`.
/// Default `("127.0.0.1", 3000, "/api")` so pre-boot reads work in tests.
pub static SERVER_ADDR: Lazy<Mutex<(String, u16, String)>> =
    Lazy::new(|| Mutex::new(("127.0.0.1".to_string(), 3000, "/api".to_string())));

#[allow(dead_code)]
pub fn set_server_addr(host: String, port: u16, api_prefix: String) {
    let mut guard = SERVER_ADDR.lock().unwrap_or_else(|p| p.into_inner());
    *guard = (host, port, api_prefix);
}

pub fn get_server_addr() -> (String, u16, String) {
    SERVER_ADDR
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
}

/// Per-file upload size cap in bytes, captured from
/// `config.server.max_file_upload_mb` at boot. Read by BOTH the per-route
/// body-limit layer (built at router construction, where the full Config is
/// not in scope) AND the per-request handler check in `upload_file_inner` — so
/// it lives here as a process-global rather than being threaded through both
/// call paths (same pattern as `CACHES_CONFIG` / `SERVER_ADDR`).
///
/// Default 128 MiB so pre-boot reads (unit tests, or a router built before boot)
/// are sane and never panic; the boot path overwrites it from config.
pub static MAX_FILE_UPLOAD_BYTES: Lazy<Mutex<usize>> = Lazy::new(|| Mutex::new(128 * 1024 * 1024));

/// Slack added on top of the per-file cap to derive the route body limit. Covers
/// multipart framing + any extra form fields, and keeps `body limit > handler
/// cap` so an over-cap upload is rejected by the handler with a clear
/// `FILE_TOO_LARGE` (400) rather than by the raw body limit (opaque 413).
const UPLOAD_BODY_LIMIT_SLACK_BYTES: usize = 16 * 1024 * 1024;

/// Set the per-file upload cap (bytes). Called once at boot from
/// `config.server.max_file_upload_mb`. Poison recovery mirrors the other
/// app_state setters.
pub fn set_max_file_upload_bytes(bytes: usize) {
    let mut guard = MAX_FILE_UPLOAD_BYTES.lock().unwrap_or_else(|poisoned| {
        tracing::error!("MAX_FILE_UPLOAD_BYTES mutex poisoned in set_max_file_upload_bytes; recovering");
        poisoned.into_inner()
    });
    *guard = bytes;
    tracing::info!("Max file upload size set to {} bytes", *guard);
}

/// Get the per-file upload cap (bytes). Read per-request by `upload_file_inner`.
pub fn get_max_file_upload_bytes() -> usize {
    *MAX_FILE_UPLOAD_BYTES.lock().unwrap_or_else(|poisoned| {
        tracing::error!("MAX_FILE_UPLOAD_BYTES mutex poisoned in get_max_file_upload_bytes; recovering");
        poisoned.into_inner()
    })
}

/// Route body limit derived from the per-file cap (`cap + framing slack`). Read
/// by the `DefaultBodyLimit` layer on the upload routes at router-build time.
pub fn file_upload_body_limit_bytes() -> usize {
    get_max_file_upload_bytes().saturating_add(UPLOAD_BODY_LIMIT_SLACK_BYTES)
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

    #[test]
    fn max_file_upload_bytes_round_trip_and_derived_body_limit() {
        // Single mutator of the global among the unit tests, so the round-trip
        // is race-free.
        let cap = 5 * 1024 * 1024;
        set_max_file_upload_bytes(cap);
        assert_eq!(get_max_file_upload_bytes(), cap);
        assert_eq!(
            file_upload_body_limit_bytes(),
            cap + UPLOAD_BODY_LIMIT_SLACK_BYTES
        );
    }
}
