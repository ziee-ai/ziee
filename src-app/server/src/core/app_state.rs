// Global application state
// Provides access to app-wide configuration like data_dir

use once_cell::sync::Lazy;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::core::config::CachesConfig;

// APP_DATA_DIR + SERVER_ADDR (and their accessors + `test_app_data_dir`) moved
// to `ziee-core` (`ziee_core::app_state`) in Chunk B1. The `~/.ziee` default is
// now derived from a configurable app-name (T-1); ziee registers "ziee" via the
// `ensure_app_name` guard threaded through the data-dir accessors below, so the
// default stays `~/.ziee` byte-for-byte. `get_server_addr`/`set_server_addr`
// are pure re-export shims (decision N2). CACHES_CONFIG + MAX_FILE_UPLOAD_BYTES
// remain here (CachesConfig moves in B2; the upload cap keeps its docker/nginx
// tests + private slack const app-side).
pub use ziee_core::app_state::{get_server_addr, set_server_addr};

/// Registers the "ziee" app-name in ziee-core exactly once, before the first
/// data-dir access, so `ziee_core`'s configurable default resolves to `~/.ziee`
/// (identical to the pre-extraction hardcode). Idempotent.
static APP_NAME_INIT: std::sync::Once = std::sync::Once::new();
fn ensure_app_name() {
    APP_NAME_INIT.call_once(|| ziee_core::app_state::set_app_name("ziee"));
}

/// Set the application data directory. Shim over `ziee_core::app_state`
/// (registers the "ziee" app-name first so the default matches `~/.ziee`).
pub fn set_app_data_dir(path: PathBuf) {
    ensure_app_name();
    ziee_core::app_state::set_app_data_dir(path);
}

/// Get the current application data directory. Shim over `ziee_core::app_state`.
pub fn get_app_data_dir() -> PathBuf {
    ensure_app_name();
    ziee_core::app_state::get_app_data_dir()
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

    // NOTE: `test_app_data_dir` moved with APP_DATA_DIR into `ziee-core`
    // (`app_state::tests`) in Chunk B1.

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

    /// The docker web image must plumb `ZIEE_MAX_FILE_UPLOAD_MB` all the way
    /// through: template key + envsubst allowlist. Missing the allowlist entry
    /// is the classic bug where the rendered config keeps a literal
    /// `${ZIEE_MAX_FILE_UPLOAD_MB}` and the server fails to parse it.
    #[test]
    fn docker_web_plumbs_max_file_upload_var() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        let template =
            std::fs::read_to_string(format!("{root}/docker/web/config.template.yaml"))
                .expect("read config.template.yaml");
        let entrypoint = std::fs::read_to_string(format!("{root}/docker/web/entrypoint.sh"))
            .expect("read entrypoint.sh");
        assert!(
            template.contains("max_file_upload_mb")
                && template.contains("${ZIEE_MAX_FILE_UPLOAD_MB}"),
            "config.template.yaml must template max_file_upload_mb from ZIEE_MAX_FILE_UPLOAD_MB"
        );
        assert!(
            entrypoint.contains("${ZIEE_MAX_FILE_UPLOAD_MB}"),
            "entrypoint.sh envsubst allowlist must include ZIEE_MAX_FILE_UPLOAD_MB"
        );
    }

    /// Invariant: `nginx client_max_body_size ≥ route body limit ≥ handler cap`.
    /// Checks the outermost layer (nginx) against the derived body limit at the
    /// default cap, so a future default bump that outgrows nginx is caught.
    #[test]
    fn nginx_body_size_covers_default_body_limit() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docker/web/nginx.conf");
        let conf = std::fs::read_to_string(path).expect("read nginx.conf");
        let bytes = parse_client_max_body_size(&conf)
            .expect("nginx.conf must set client_max_body_size");
        // Default cap (128 MiB) + slack (16 MiB), computed from constants so this
        // is independent of the mutable global (which another unit test sets).
        let default_body_limit = 128 * 1024 * 1024 + UPLOAD_BODY_LIMIT_SLACK_BYTES;
        assert!(
            bytes >= default_body_limit,
            "nginx client_max_body_size ({bytes} B) must be >= default route body limit ({default_body_limit} B)"
        );
    }

    /// Parse the first `client_max_body_size <N>[kKmMgG];` directive to bytes.
    fn parse_client_max_body_size(conf: &str) -> Option<usize> {
        for line in conf.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("client_max_body_size") {
                let tok = rest.trim().trim_end_matches(';').trim();
                let (num, unit) = tok.split_at(
                    tok.find(|c: char| !c.is_ascii_digit()).unwrap_or(tok.len()),
                );
                let n: usize = num.parse().ok()?;
                let mult = match unit.trim().to_ascii_lowercase().as_str() {
                    "" => 1,
                    "k" => 1024,
                    "m" => 1024 * 1024,
                    "g" => 1024 * 1024 * 1024,
                    _ => return None,
                };
                return Some(n * mult);
            }
        }
        None
    }
}
