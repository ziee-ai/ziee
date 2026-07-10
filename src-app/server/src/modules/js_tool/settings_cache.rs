//! Process-wide snapshot of the `js_tool_settings` singleton.
//!
//! `run_js` reads these caps on every invocation (via `JsCaps::from_settings`).
//! A Postgres round-trip per run would add latency, so we cache the row in a
//! `RwLock<Arc<...>>` and invalidate on the admin PUT. Mirrors
//! `code_sandbox::resource_limits_cache`.

use std::sync::{Arc, OnceLock, RwLock};

use chrono::Utc;

use crate::common::AppError;
use crate::core::repository::Repos;
use crate::modules::js_tool::settings::JsToolSettings;

static CACHE: OnceLock<RwLock<Arc<JsToolSettings>>> = OnceLock::new();

/// Get the current settings — loading from DB on first call, returning the
/// cached `Arc` snapshot thereafter.
pub async fn get() -> Result<Arc<JsToolSettings>, AppError> {
    if let Some(rw) = CACHE.get() {
        return Ok(rw.read().expect("js_tool settings_cache RwLock").clone());
    }
    let row = Repos.js_tool.get_settings().await?;
    let arc = Arc::new(row);
    let _ = CACHE.set(RwLock::new(arc.clone()));
    Ok(CACHE
        .get()
        .expect("just initialized")
        .read()
        .expect("js_tool settings_cache RwLock")
        .clone())
}

/// Replace the cached snapshot after a successful admin PUT so the next
/// `run_js` picks up the new caps immediately. No-op if the cache hasn't been
/// primed (the first [`get`] loads the new row anyway).
pub fn invalidate(new_row: &JsToolSettings) {
    if let Some(rw) = CACHE.get() {
        let mut w = rw.write().expect("js_tool settings_cache RwLock");
        *w = Arc::new(new_row.clone());
        tracing::info!(
            memory_bytes = new_row.memory_bytes,
            wall_secs = new_row.wall_secs,
            max_concurrent_runs = new_row.max_concurrent_runs,
            "js_tool: settings cache invalidated"
        );
    }
}

/// Hard-coded fallback matching the migration-135 DEFAULTs. Used only if a
/// caller needs a snapshot before the DB has ever been read.
pub fn defaults() -> JsToolSettings {
    let now = Utc::now();
    JsToolSettings {
        memory_bytes: 128 * 1024 * 1024,
        max_stack_bytes: 512 * 1024,
        wall_secs: 300,
        approval_timeout_secs: 300,
        max_concurrent_runs: 8,
        max_concurrent_dispatch: 6,
        max_trace_entries: 256,
        created_at: now,
        updated_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TEST-40: defaults() matches the migration DEFAULTs exactly.
    #[test]
    fn defaults_match_migration() {
        let d = defaults();
        assert_eq!(d.memory_bytes, 134217728); // 128 MiB
        assert_eq!(d.max_stack_bytes, 524288); // 512 KiB
        assert_eq!(d.wall_secs, 300);
        assert_eq!(d.approval_timeout_secs, 300);
        assert_eq!(d.max_concurrent_runs, 8);
        assert_eq!(d.max_concurrent_dispatch, 6);
        assert_eq!(d.max_trace_entries, 256);
    }
}
