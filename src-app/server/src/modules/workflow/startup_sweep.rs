//! Boot-time orphan-run sweep (plan §4.3 server-restart caveat).
//!
//! On server boot, flips every `workflow_runs` row in
//! `pending`/`running` to `failed` ("server restart during execution")
//! and removes staged `workspace/<conv>/workflow/<run>/` dirs for
//! runs that are no longer non-terminal.

#![allow(dead_code)]

use sqlx::PgPool;

use crate::common::AppError;
use crate::modules::workflow::repository;

pub async fn sweep_at_boot(pool: &PgPool) -> Result<(), AppError> {
    let rows = repository::fail_orphaned_runs(pool).await?;
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
            // Check status — non-terminal still gets cleaned (the row
            // was just flipped to `failed`).
            let still_running = match repository::find_run(pool, run_id).await {
                Ok(Some(r)) => matches!(r.status.as_str(), "pending" | "running"),
                Ok(None) => false,
                Err(_) => false,
            };
            if !still_running {
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
    Ok(())
}
