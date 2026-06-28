//! Boot-time retention/eviction loop for download bookkeeping and on-disk
//! caches. Mirrors the `mcp::tool_calls::prune` pattern: a fire-and-forget
//! background task spawned at module init that ticks on a fixed interval.
//!
//! Two jobs per tick (both best-effort — a failure logs and is retried next
//! tick, never crashes the server):
//!
//!   1. **download_instances retention** — terminal rows
//!      (`completed`/`failed`/`cancelled`) older than 7 days are deleted.
//!      Active (`pending`/`downloading`) rows are never touched.
//!
//!   2. **cache eviction** — top-level entries under the git, LFS, and engine
//!      caches that have not been touched in >30 days are removed. The engine
//!      cache additionally skips any binary still referenced by a
//!      `llm_runtime_versions.binary_path` row, so an engine that was
//!      downloaded long ago but is still registered/in-use is NEVER evicted.
//!      (The HF-models cache is intentionally NOT swept here: its files back
//!      registered models with no on-disk→DB path mapping to make mtime
//!      eviction safe.)

use std::path::Path;
use std::time::{Duration, SystemTime};

use sqlx::PgPool;
use time::OffsetDateTime;

/// How often the prune/eviction loop runs.
const PRUNE_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// Terminal download_instances rows are kept this long, then pruned.
const DOWNLOAD_RETENTION_DAYS: i64 = 7;

/// Cache entries untouched for longer than this are evicted.
const CACHE_UNUSED_DAYS: u64 = 30;

pub async fn run_prune_loop(pool: PgPool) {
    tracing::info!(
        "llm_model::prune: started (tick {}h; downloads kept {}d, caches kept {}d)",
        PRUNE_INTERVAL.as_secs() / 3600,
        DOWNLOAD_RETENTION_DAYS,
        CACHE_UNUSED_DAYS,
    );
    loop {
        prune_download_instances(&pool).await;
        evict_caches(&pool).await;
        tokio::time::sleep(PRUNE_INTERVAL).await;
    }
}

/// Delete terminal download rows older than the retention window.
async fn prune_download_instances(pool: &PgPool) {
    let cutoff = OffsetDateTime::now_utc() - time::Duration::days(DOWNLOAD_RETENTION_DAYS);
    match sqlx::query!(
        r#"
        DELETE FROM download_instances
        WHERE status IN ('completed', 'failed', 'cancelled')
          AND updated_at < $1
        "#,
        cutoff
    )
    .execute(pool)
    .await
    {
        Ok(res) if res.rows_affected() > 0 => {
            tracing::info!(
                "llm_model::prune: removed {} terminal download_instances rows older than {}d",
                res.rows_affected(),
                DOWNLOAD_RETENTION_DAYS
            );
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("llm_model::prune: download_instances prune failed: {e}"),
    }
}

/// Evict stale entries from the git / LFS / engine caches.
async fn evict_caches(pool: &PgPool) {
    let caches = crate::core::get_caches_config();

    // git + LFS clones are content-addressed and fully re-derivable, so a
    // pure mtime sweep is safe (they re-clone on next use).
    evict_dir_by_mtime(Path::new(caches.git_cache_dir()), &Default::default());
    evict_dir_by_mtime(Path::new(caches.lfs_cache_dir()), &Default::default());

    // Engine binaries back DB-registered runtime versions; never evict one
    // that is still referenced, regardless of its age.
    let referenced = referenced_engine_paths(pool).await;
    evict_dir_by_mtime(Path::new(caches.llm_engines_dir()), &referenced);
}

/// Set of canonicalized paths that must never be evicted from the engine
/// cache because a runtime-version row still points at them.
async fn referenced_engine_paths(pool: &PgPool) -> std::collections::HashSet<std::path::PathBuf> {
    let mut set = std::collections::HashSet::new();
    match sqlx::query_scalar!(r#"SELECT binary_path FROM llm_runtime_versions"#)
        .fetch_all(pool)
        .await
    {
        Ok(paths) => {
            for p in paths {
                let pb = std::path::PathBuf::from(&p);
                // Insert both the raw and canonicalized form; entries under the
                // cache are matched by prefix below.
                if let Ok(c) = pb.canonicalize() {
                    set.insert(c);
                }
                set.insert(pb);
            }
        }
        Err(e) => {
            // Fail SAFE: if we can't read the references, skip engine eviction
            // entirely this tick rather than risk deleting an in-use binary.
            tracing::warn!("llm_model::prune: could not read runtime-version paths ({e}); skipping engine-cache eviction this tick");
            set.insert(std::path::PathBuf::from("\0__skip_all__"));
        }
    }
    set
}

/// Remove top-level entries directly under `root` whose most-recent
/// modification is older than [`CACHE_UNUSED_DAYS`]. An entry is preserved if
/// any path in `protected` equals it or lives underneath it.
fn evict_dir_by_mtime(
    root: &Path,
    protected: &std::collections::HashSet<std::path::PathBuf>,
) {
    // The sentinel inserted on a DB-read failure means "skip this sweep".
    if protected.contains(&std::path::PathBuf::from("\0__skip_all__")) {
        return;
    }
    let read = match std::fs::read_dir(root) {
        Ok(r) => r,
        Err(_) => return, // cache dir absent → nothing to evict
    };
    let max_age = Duration::from_secs(CACHE_UNUSED_DAYS * 24 * 60 * 60);
    let now = SystemTime::now();

    for entry in read.flatten() {
        let path = entry.path();
        if entry_is_protected(&path, protected) {
            continue;
        }
        if !older_than(&path, now, max_age) {
            continue;
        }
        let res = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        match res {
            Ok(()) => tracing::info!(
                "llm_model::prune: evicted stale cache entry {} (unused >{}d)",
                path.display(),
                CACHE_UNUSED_DAYS
            ),
            Err(e) => tracing::warn!(
                "llm_model::prune: failed to evict {}: {e}",
                path.display()
            ),
        }
    }
}

/// True if `path` (or anything under it) is referenced by a protected path.
fn entry_is_protected(
    path: &Path,
    protected: &std::collections::HashSet<std::path::PathBuf>,
) -> bool {
    let canon = path.canonicalize().ok();
    protected.iter().any(|p| {
        p == path
            || p.starts_with(path)
            || canon.as_ref().is_some_and(|c| p == c || p.starts_with(c))
    })
}

/// True if every modification timestamp we can read for `path` is older than
/// `max_age`. Walks one level into directories so a recently-touched file
/// inside an otherwise-old dir keeps the whole entry alive.
fn older_than(path: &Path, now: SystemTime, max_age: Duration) -> bool {
    fn newest_mtime(path: &Path, best: &mut Option<SystemTime>) {
        if let Ok(meta) = std::fs::metadata(path) {
            if let Ok(m) = meta.modified() {
                if best.map_or(true, |b| m > b) {
                    *best = Some(m);
                }
            }
        }
        if path.is_dir() {
            if let Ok(rd) = std::fs::read_dir(path) {
                for e in rd.flatten() {
                    newest_mtime(&e.path(), best);
                }
            }
        }
    }
    let mut newest = None;
    newest_mtime(path, &mut newest);
    match newest {
        Some(m) => now.duration_since(m).map(|age| age > max_age).unwrap_or(false),
        None => false, // can't read mtime → keep, fail safe
    }
}
