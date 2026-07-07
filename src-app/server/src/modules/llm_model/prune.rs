//! Boot-time retention/eviction loop for download bookkeeping and on-disk
//! caches. Mirrors the `mcp::tool_calls::prune` pattern: a fire-and-forget
//! background task spawned at module init that ticks on a fixed interval.
//!
//! Three jobs per tick (all best-effort — a failure logs and is retried next
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
//!
//!   3. **deprecation sweep** — re-discovers each remote provider's live model
//!      list and flips `is_deprecated` on saved rows that vanished (or that the
//!      curated catalog marks deprecated), clearing it when a model reappears.
//!      No-op on a failed/empty fetch (see [`sweep_provider_once`]).

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
    // One-shot boot reconcile: a download task runs in-process, so any row left
    // in a non-terminal state (`pending`/`downloading`) is an orphan from a
    // previous process that died mid-download — its task no longer exists. Left
    // alone, `find_existing_in_progress_download` dedupes new requests onto the
    // dead row and blocks re-download forever. Reset such rows to `failed` so
    // the UI shows a terminal state and re-download is unblocked. Safe to run at
    // init: this fires before any download request is served this process, so no
    // live in-progress row exists yet to be clobbered.
    reconcile_interrupted_downloads(&pool).await;
    loop {
        prune_download_instances(&pool).await;
        evict_caches(&pool).await;
        sweep_deprecated_models(&pool).await;
        tokio::time::sleep(PRUNE_INTERVAL).await;
    }
}

// =====================================================================
// Deprecation sweep (orphaned / removed remote models)
// =====================================================================
//
// A model a user added may be deprecated or removed by the provider. Once per
// tick we re-discover each REMOTE provider's live model list and flip
// `is_deprecated` on saved rows that vanished (or that the curated catalog marks
// deprecated), clearing the flag when a model reappears. Best-effort: any error
// logs and is retried next tick.

use crate::modules::llm_model::models::LlmModel;
use crate::modules::llm_model::permissions::LlmModelsRead;
use crate::modules::llm_model::repository::{list_llm_models_by_provider, set_model_deprecated};
use crate::modules::llm_provider::models::LlmProvider;
use crate::modules::llm_provider::permissions::UserLlmProvidersRead;
use crate::modules::llm_provider::repositories::admin::list_llm_providers;
use crate::modules::sync::{Audience, SyncAction, SyncEntity, publish as sync_publish};

/// Sweep every remote provider once. Local providers list from the DB, so they
/// are skipped.
async fn sweep_deprecated_models(pool: &PgPool) {
    let providers = match list_llm_providers(pool).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("llm_model::prune: deprecation sweep could not list providers: {e}");
            return;
        }
    };
    let mut flipped = 0usize;
    for provider in providers {
        // Skip local (DB-listed) providers and any provider an admin has not
        // enabled — probing a disabled/unconfigured provider would make a
        // pointless outbound call (and a 401) at every boot. The on-demand
        // refresh endpoint still works on any provider the admin explicitly asks
        // to reconcile.
        if provider.provider_type == "local" || !provider.enabled {
            continue;
        }
        match sweep_provider_once(pool, &provider).await {
            Ok(n) => flipped += n,
            Err(e) => tracing::warn!(
                "llm_model::prune: deprecation sweep for provider {} failed: {e}",
                provider.id
            ),
        }
    }
    if flipped > 0 {
        tracing::info!("llm_model::prune: deprecation sweep updated {flipped} model(s)");
    }
}

/// Reconcile one remote provider's saved models against its live `/v1/models`
/// set, returning the number of rows whose `is_deprecated` flag changed.
///
/// SAFETY (DEC-5): only mutate when the live fetch SUCCEEDS and returns a
/// NON-EMPTY set. A missing key / offline / 401 must be a no-op — otherwise a
/// transient failure would deprecate every model. A model is flagged deprecated
/// when it is absent from the live set OR the curated catalog marks it
/// deprecated; the flag is cleared when it is present and not catalog-deprecated.
///
/// Shared by the background loop and the on-demand refresh handler.
pub async fn sweep_provider_once(
    pool: &PgPool,
    provider: &LlmProvider,
) -> Result<usize, String> {
    // Local providers manage their own model list.
    if provider.provider_type == "local" {
        return Ok(0);
    }
    let Some(base_url) = provider.base_url.as_deref().filter(|b| !b.is_empty()) else {
        // No endpoint to query → cannot safely decide; no-op.
        return Ok(0);
    };
    let api_key = provider.api_key.clone().unwrap_or_default();

    let live = crate::modules::llm_provider::handlers::discover::fetch_live_models(
        &provider.provider_type,
        base_url,
        &api_key,
    )
    .await?;

    // DEC-5 guard: an empty set (or a failure, already `?`-propagated above) is
    // never treated as "every model is gone".
    if live.is_empty() {
        return Ok(0);
    }
    let live_ids: std::collections::HashSet<String> =
        live.iter().map(|m| m.id.clone()).collect();

    let models = list_llm_models_by_provider(pool, provider.id)
        .await
        .map_err(|e| format!("list models: {e}"))?;

    let decisions = decide_deprecations(&provider.provider_type, &live_ids, &models);

    let mut changed = 0usize;
    for (model_id, should_deprecate) in decisions {
        match set_model_deprecated(pool, model_id, should_deprecate).await {
            Ok(true) => {
                changed += 1;
                emit_model_sync(model_id);
                tracing::info!(
                    "llm_model::prune: model {model_id} is_deprecated -> {should_deprecate}"
                );
            }
            Ok(false) => {}
            Err(e) => tracing::warn!(
                "llm_model::prune: failed to set is_deprecated on {model_id}: {e}"
            ),
        }
    }
    Ok(changed)
}

/// Pure decision core: given the live model-id set and the saved models, return
/// the `(model_id, new_is_deprecated)` pairs that must change. Network- and
/// DB-free so it is directly unit-testable.
///
/// A model is deprecated when it is absent from the live set OR the curated
/// catalog marks it deprecated; the flag is cleared when it is present and not
/// catalog-deprecated. An EMPTY live set yields no changes (DEC-5 guard, also
/// enforced by the caller before the fetch is trusted).
pub(crate) fn decide_deprecations(
    provider_type: &str,
    live_ids: &std::collections::HashSet<String>,
    models: &[LlmModel],
) -> Vec<(uuid::Uuid, bool)> {
    if live_ids.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for m in models {
        let catalog_deprecated = ai_providers::registry_lookup(provider_type, &m.name)
            .map(|c| c.deprecated)
            .unwrap_or(false);
        let should = !live_ids.contains(&m.name) || catalog_deprecated;
        if should != m.is_deprecated {
            out.push((m.id, should));
        }
    }
    out
}

/// Emit the dual permission-scoped sync pair for a model change from a detached
/// task (origin = None), mirroring what `create_model` emits.
fn emit_model_sync(model_id: uuid::Uuid) {
    sync_publish(
        SyncEntity::LlmModel,
        SyncAction::Update,
        model_id,
        Audience::perm::<LlmModelsRead>(),
        None,
    );
    sync_publish(
        SyncEntity::UserLlmProvider,
        SyncAction::Update,
        model_id,
        Audience::perm::<UserLlmProvidersRead>(),
        None,
    );
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

/// Reset download rows orphaned by a server restart (non-terminal status with
/// no live in-process task) to `failed`. Run once at boot. See the call site in
/// [`run_prune_loop`] for why this is safe to do unconditionally at init.
async fn reconcile_interrupted_downloads(pool: &PgPool) {
    match sqlx::query!(
        r#"
        UPDATE download_instances
        SET status = 'failed',
            error_message = 'download interrupted by server restart',
            completed_at = NOW(),
            updated_at = NOW()
        WHERE status IN ('pending', 'downloading')
        "#,
    )
    .execute(pool)
    .await
    {
        Ok(res) if res.rows_affected() > 0 => {
            tracing::warn!(
                "llm_model::prune: reconciled {} interrupted download_instances row(s) to 'failed' (orphaned by a prior restart)",
                res.rows_affected()
            );
        }
        Ok(_) => {}
        Err(e) => tracing::warn!(
            "llm_model::prune: interrupted-download reconcile failed: {e}"
        ),
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

#[cfg(test)]
mod deprecation_tests {
    use super::decide_deprecations;
    use crate::modules::llm_model::models::LlmModel;
    use std::collections::HashSet;
    use uuid::Uuid;

    fn model(name: &str, is_deprecated: bool) -> LlmModel {
        serde_json::from_value(serde_json::json!({
            "id": Uuid::new_v4(),
            "provider_id": Uuid::new_v4(),
            "name": name,
            "display_name": name,
            "enabled": true,
            "is_deprecated": is_deprecated,
            "is_active": false,
            "capabilities": {},
            "parameters": {},
            "created_at": "2020-01-01T00:00:00Z",
            "updated_at": "2020-01-01T00:00:00Z",
            "engine_type": "none",
            "file_format": "safetensors"
        }))
        .expect("valid LlmModel")
    }

    fn ids(list: &[&str]) -> HashSet<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn flags_model_absent_from_live_set() {
        let live = ids(&["gpt-4o"]);
        let models = vec![model("gpt-4o", false), model("removed-model", false)];
        let d = decide_deprecations("openai", &live, &models);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].0, models[1].id);
        assert!(d[0].1, "removed model must be flagged deprecated");
    }

    #[test]
    fn clears_flag_when_model_reappears() {
        let live = ids(&["gpt-4o"]);
        let models = vec![model("gpt-4o", true)]; // currently deprecated, but present
        let d = decide_deprecations("openai", &live, &models);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].1, false, "reappeared model must be un-deprecated");
    }

    #[test]
    fn empty_live_set_never_flags() {
        // DEC-5: an empty (failed/degraded) fetch is a no-op, never mass-flag.
        let live: HashSet<String> = HashSet::new();
        let models = vec![model("gpt-4o", false), model("anything", false)];
        assert!(decide_deprecations("openai", &live, &models).is_empty());
    }

    #[test]
    fn catalog_deprecated_flagged_even_when_present() {
        // gpt-3.5-turbo is present live but marked deprecated in known_models.json.
        let live = ids(&["gpt-4o", "gpt-3.5-turbo"]);
        let models = vec![model("gpt-4o", false), model("gpt-3.5-turbo", false)];
        let d = decide_deprecations("openai", &live, &models);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].0, models[1].id);
        assert!(d[0].1);
    }

    #[test]
    fn no_change_returns_empty() {
        let live = ids(&["gpt-4o"]);
        let models = vec![model("gpt-4o", false)];
        assert!(decide_deprecations("openai", &live, &models).is_empty());
    }
}
