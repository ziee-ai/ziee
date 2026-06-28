//! Boot-time retention / cache-eviction loop for download lifecycle artifacts.
//!
//! Mirrors `mcp::tool_calls::prune` (a fire-and-forget `tokio::spawn` loop on a
//! fixed tick). Two jobs run each tick:
//!
//!   1. **download_instances retention** — terminal (`completed` / `failed`)
//!      rows older than [`DOWNLOAD_RETENTION_DAYS`] are deleted. Previously
//!      these rows were only ever removed by an explicit manual clear, so a
//!      busy deployment accumulated them forever.
//!
//!   2. **Transient clone-cache eviction** — entries under the git + LFS
//!      caches that have not been touched (mtime) in [`CACHE_MAX_AGE_DAYS`]
//!      days are removed. These caches are intermediate download scratch that
//!      git/LFS re-creates on demand, so mtime is a safe "unused" proxy here.
//!
//! NOTE — the `hf-models` and `llm-engines` caches are deliberately NOT
//! mtime-evicted: those directories hold the FINAL model/engine artifacts that
//! registered `llm_models` / runtime-version rows point at, and their files'
//! mtimes are set at download time and never updated on use (model loads are
//! read-only). An mtime sweep would therefore delete actively-used models that
//! merely haven't been re-downloaded in 30 days. Reclaiming those safely needs
//! last-used accounting (a load-timestamp signal) that does not exist yet; it
//! is intentionally out of scope for this mtime-based loop.

use std::path::Path;
use std::time::{Duration, SystemTime};

use sqlx::PgPool;

use crate::modules::llm_model::repository;

/// How often the retention loop runs.
const TICK: Duration = Duration::from_secs(6 * 60 * 60);
/// Terminal download_instances rows older than this are pruned.
const DOWNLOAD_RETENTION_DAYS: i64 = 7;
/// Transient clone-cache entries untouched longer than this are evicted.
const CACHE_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// Spawn-friendly entry point: runs forever on [`TICK`].
pub async fn run_retention_loop(pool: PgPool) {
    tracing::info!(
        "llm_model: retention loop started; download_retention={}d cache_max_age=30d tick=6h",
        DOWNLOAD_RETENTION_DAYS
    );
    loop {
        prune_downloads_once(&pool).await;
        evict_transient_caches_once();
        tokio::time::sleep(TICK).await;
    }
}

async fn prune_downloads_once(pool: &PgPool) {
    let cutoff = time::OffsetDateTime::now_utc() - time::Duration::days(DOWNLOAD_RETENTION_DAYS);
    match repository::prune_terminal_download_instances(pool, cutoff).await {
        Ok(0) => {}
        Ok(n) => tracing::info!(
            "llm_model: pruned {n} terminal download_instances older than {DOWNLOAD_RETENTION_DAYS}d"
        ),
        Err(e) => tracing::warn!("llm_model: download_instances prune failed: {e}"),
    }
}

fn evict_transient_caches_once() {
    let caches = crate::core::get_caches_config();
    for root in [caches.git_cache_dir(), caches.lfs_cache_dir()] {
        evict_stale_entries(Path::new(root));
    }
}

/// Remove immediate children of `root` whose mtime is older than
/// [`CACHE_MAX_AGE`]. Best-effort: unreadable entries / failed removals are
/// logged and skipped.
fn evict_stale_entries(root: &Path) {
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return, // cache dir not created yet — nothing to do
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let path = entry.path();
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let age = now.duration_since(mtime).unwrap_or(Duration::ZERO);
        if age <= CACHE_MAX_AGE {
            continue;
        }
        let res = if meta.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        match res {
            Ok(()) => tracing::info!(
                "llm_model: evicted stale cache entry {} (age={}d)",
                path.display(),
                age.as_secs() / 86_400
            ),
            Err(e) => {
                tracing::warn!("llm_model: failed to evict cache entry {}: {e}", path.display())
            }
        }
    }
}
