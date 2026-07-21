//! Boot-time orphan-run sweep (plan §4.3 server-restart caveat).
//!
//! On server boot, flips every `workflow_runs` row in
//! `pending`/`running` to `failed` ("server restart during execution")
//! and removes staged `workspace/<conv>/workflow/<run>/` dirs for
//! runs that are no longer non-terminal.


use sqlx::PgPool;

use crate::common::AppError;
use crate::modules::workflow::repository;

/// True when `path`'s mtime is more than 30 days in the past (or its metadata
/// can't be read). Used to bound the leak of a genuinely-orphaned staging dir
/// whose run row is gone, WITHOUT reclaiming another live server's active run
/// when the workspace root is shared (integration harness). Mirrors the
/// code_sandbox per-conversation workspace reaper's 30-day policy.
fn dir_older_than_30d(path: &std::path::Path) -> bool {
    const THIRTY_DAYS: std::time::Duration = std::time::Duration::from_secs(30 * 24 * 60 * 60);
    match std::fs::metadata(path).and_then(|m| m.modified()) {
        Ok(mtime) => mtime.elapsed().map(|age| age > THIRTY_DAYS).unwrap_or(false),
        // Can't stat / mtime in the future (clock skew) → treat as NOT ancient
        // so we never delete a dir we're unsure about.
        Err(_) => false,
    }
}

pub async fn sweep_at_boot(
    pool: &PgPool,
    cutoff: time::OffsetDateTime,
) -> Result<(), AppError> {
    let rows = repository::fail_orphaned_runs(pool, cutoff).await?;
    if rows > 0 {
        tracing::warn!(
            count = rows,
            "workflow: startup sweep marked {rows} orphan in-flight run(s) as failed"
        );
    }

    // Walk <workspace_root>/*/workflow/*/ and rm any subdir whose
    // run_id is no longer in a non-terminal status. We delete only
    // dirs that match the run-id naming convention (UUID v4).
    let workspace_root = crate::modules::workflow::runner::workflow_workspace_root();
    if !workspace_root.exists() {
        return Ok(());
    }
    let entries = match std::fs::read_dir(&workspace_root) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    let mut removed = 0usize;
    for conv_entry in entries.flatten() {
        let wf_dir = conv_entry.path().join("workflow");
        if !wf_dir.is_dir() {
            continue;
        }
        let runs = match std::fs::read_dir(&wf_dir) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for run_entry in runs.flatten() {
            let name = run_entry.file_name();
            let name_s = name.to_string_lossy();
            let run_id = match uuid::Uuid::parse_str(&name_s) {
                Ok(u) => u,
                Err(_) => continue,
            };
            // Check status. A `pending`/`running` orphan was just flipped to
            // `failed` by `fail_orphaned_runs`, so its dir is GC'd here. A
            // `waiting` run is a DURABLE elicit gate: `fail_orphaned_runs`
            // spared it, and its `outputs/` is the resume checkpoint — KEEP it
            // so `resume_run` can rehydrate after the user submits.
            let keep_dir = match repository::find_run(pool, run_id).await {
                // ITEM-17: `resumable` is a non-terminal crash-resume state — keep
                // its staged dir (its workspace + transcript are the resume source).
                Ok(Some(r)) => matches!(
                    r.status.as_str(),
                    "pending" | "running" | "waiting" | "resumable"
                ),
                // A dir whose run_id is NOT in THIS server's DB is NOT ours to
                // reclaim at boot. In production the workspace root has a single
                // owner, so this only ever means a genuinely-orphaned leak (the
                // run row was deleted) — but when the root is SHARED by more than
                // one live server (the integration harness spawns a server per
                // test, all under one /tmp/ziee-workflows), it is another server's
                // ACTIVE run, and `remove_dir_all` would clobber it mid-run. Keep
                // it unless it is genuinely ancient (30-day guard, mirroring the
                // code_sandbox workspace reaper) so a true leak still gets GC'd.
                Ok(None) => !dir_older_than_30d(&run_entry.path()),
                // DB hiccup: never delete on uncertainty.
                Err(_) => true,
            };
            if !keep_dir {
                let _ = std::fs::remove_dir_all(run_entry.path());
                removed += 1;
            }
        }
    }
    if removed > 0 {
        tracing::info!(
            removed,
            "workflow: startup sweep removed {removed} stale staged dir(s)"
        );
    }

    // ITEM-17: re-drive every `resumable` crash-resume run. `fail_orphaned_runs`
    // marked crashed `kind: agent` runs `resumable` (spared, dir kept); each is
    // re-entered via `resume_run`, whose AgentDispatcher replays the persisted
    // transcript so completed tool calls are not re-executed. Best-effort — a
    // resume that can't currently resolve its model just logs and stays parked.
    match repository::list_resumable_run_ids(pool).await {
        Ok(ids) if !ids.is_empty() => {
            tracing::info!(
                count = ids.len(),
                "workflow: startup sweep re-driving {} resumable agent run(s)",
                ids.len()
            );
            for run_id in ids {
                let pool = pool.clone();
                tokio::spawn(async move {
                    if let Err(e) =
                        crate::modules::workflow::runner::resume_run(&pool, run_id).await
                    {
                        tracing::warn!(
                            run_id = %run_id,
                            error = %e,
                            "workflow: resume of crashed agent run failed"
                        );
                    }
                });
            }
        }
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "workflow: list resumable runs failed"),
    }

    Ok(())
}
