//! The scheduler tick loop — the boot-spawned heartbeat that fires due tasks.
//!
//! Mirrors `memory/reaper.rs` (thin loop → testable `run_once`) with the
//! `llm_local_runtime` debug interval seam (`SCHEDULER_TICK_MS`). Single-process
//! (DEC-10): one loop, sequential `run_once`, so there is no concurrent
//! double-fire; each claimed task's `next_run_at` is advanced BEFORE dispatch
//! (DEC-16 intent) so a crash mid-dispatch never re-fires it. Downtime is
//! handled by coalesced catch-up — an overdue task fires once, then advances
//! past `now`.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::core::config::Config;

use super::dispatch::{self, DispatchOutcome};
use super::repository;
use super::schedule::{self, ScheduleKind};
use super::settings;

/// How many due tasks a single tick processes (bounds work per tick).
const BATCH: i64 = 50;
const DEFAULT_TICK: Duration = Duration::from_secs(60);

/// Tick cadence. Debug builds honor `SCHEDULER_TICK_MS` so tests observe
/// behavior in ms (compiled out of release).
fn tick_interval() -> Duration {
    #[cfg(debug_assertions)]
    if let Ok(ms) = std::env::var("SCHEDULER_TICK_MS") {
        if let Ok(ms) = ms.parse::<u64>() {
            return Duration::from_millis(ms.max(1));
        }
    }
    DEFAULT_TICK
}

/// Spawned at module init. Never returns.
pub async fn run_tick_loop(pool: PgPool, config: Arc<Config>) {
    tracing::info!(
        "scheduler.tick: started; interval={:?} (boot catch-up on first tick)",
        tick_interval()
    );
    loop {
        if let Err(e) = run_once(&pool, &config).await {
            tracing::warn!("scheduler.tick: tick failed: {e}");
        }
        tokio::time::sleep(tick_interval()).await;
    }
}

/// One sweep: claim due tasks, advance each, dispatch, record the outcome.
pub async fn run_once(pool: &PgPool, config: &Arc<Config>) -> Result<(), sqlx::Error> {
    let now = Utc::now();
    let due = match repository::claim_due_tasks(pool, now, BATCH).await {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("scheduler.tick: claim_due_tasks failed: {e:?}");
            return Ok(());
        }
    };

    for task in due {
        fire_task(pool, config, &task, "schedule", now).await;
    }
    Ok(())
}

/// Advance the task's `next_run_at` (coalesced), then dispatch + record. Used by
/// both the tick and the run-now path (`trigger`). Errors are captured into the
/// run record, never propagated (one bad task can't kill the sweep).
pub async fn fire_task(
    pool: &PgPool,
    config: &Arc<Config>,
    task: &super::models::ScheduledTask,
    trigger: &str,
    now: DateTime<Utc>,
) {
    // Coalesced next occurrence strictly after `now` (skips missed intervals).
    let kind = task.schedule_kind();
    let next = match schedule::next_occurrence(
        kind,
        task.run_at,
        task.cron_expr.as_deref(),
        &task.timezone,
        now,
    ) {
        Ok(n) => n,
        Err(_) => None, // unschedulable now → disable
    };
    // A recurring task with no future occurrence, or a spent `once`, disables.
    let disable_reason = if trigger == "run_now" {
        // run-now never mutates the schedule bookkeeping.
        None
    } else if matches!(kind, ScheduleKind::Once) || next.is_none() {
        // once always disables after firing; recurring w/ no next also disables.
        Some(())
    } else {
        None
    };

    if trigger != "run_now" {
        let next_to_set = if disable_reason.is_some() { None } else { next };
        if let Err(e) = repository::mark_fired(pool, task.id, next_to_set, now, None).await {
            tracing::warn!("scheduler.tick: mark_fired {} failed: {e:?}", task.id);
        }
    }

    // Dispatch (never returns Err — captures failures into the outcome).
    let outcome: DispatchOutcome = dispatch::dispatch(pool, config, task, trigger).await;

    // Persist the outcome on the task (failure counter / fingerprint / auto-pause).
    let admin_max = settings::get(pool)
        .await
        .map(|s| s.max_consecutive_failures)
        .unwrap_or(5);
    let pause_reason = if !outcome.success {
        let will_be = task.consecutive_failures + 1;
        if super::failure::should_autopause(will_be, admin_max) {
            Some("max_failures")
        } else {
            None
        }
    } else {
        None
    };
    if let Err(e) = repository::record_outcome(
        pool,
        task.id,
        outcome.status,
        outcome.success,
        outcome.fingerprint.as_deref(),
        outcome.signature.as_ref(),
        pause_reason,
    )
    .await
    {
        tracing::warn!("scheduler.tick: record_outcome {} failed: {e:?}", task.id);
    }

    // Record the firing in the audit history.
    let run = super::models::NewTaskRun {
        scheduled_task_id: task.id,
        user_id: task.user_id,
        trigger: trigger.to_string(),
        status: outcome.status.to_string(),
        error_class: outcome.error_class.clone(),
        error_message: outcome.error_message.clone(),
        notification_id: outcome.notification_id,
        workflow_run_id: outcome.workflow_run_id,
        conversation_id: outcome.conversation_id,
        fired_at: now,
    };
    if let Err(e) = repository::insert_run(pool, run).await {
        tracing::warn!("scheduler.tick: insert_run {} failed: {e:?}", task.id);
    }

    // Notify the owner's devices that the task (its runs/state) changed.
    super::events::emit_task(crate::modules::sync::SyncAction::Update, task.id, task.user_id, None);
}
