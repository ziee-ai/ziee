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

/// ITEM-7/DEC-6: a due task whose referenced entity was deleted should pause
/// pre-emptively rather than fire. A workflow task with no `workflow_id` (FK went
/// NULL) → `target_missing`. A prompt task whose bound conversation was deleted
/// AFTER a prior run (`bound_conversation_id` NULL but `last_status` set) →
/// `conversation_deleted`. A genuine first-run prompt task (both NULL) is NOT
/// paused — it legitimately creates its bound conversation. A NULL `assistant_id`
/// is never a pause reason (NULL = use the user's default assistant).
fn preemptive_pause_reason(task: &super::models::ScheduledTask) -> Option<&'static str> {
    match task.target_kind.as_str() {
        "workflow" if task.workflow_id.is_none() => Some("target_missing"),
        "prompt"
            if task.bound_conversation_id.is_none() && task.last_status.is_some() =>
        {
            Some("conversation_deleted")
        }
        _ => None,
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
        // ITEM-7: pre-emptively pause a task whose referenced entity was deleted,
        // instead of firing → creating a throwaway conversation / emitting a
        // spurious failure notification. The run is recorded (history) + sync'd,
        // but no target is dispatched and no notification is sent.
        if let Some(reason) = preemptive_pause_reason(&task) {
            if let Err(e) = repository::mark_fired(pool, task.id, None, now, Some(reason)).await {
                tracing::warn!("scheduler.tick: preemptive mark_fired {} failed: {e:?}", task.id);
            }
            let run = super::models::NewTaskRun {
                scheduled_task_id: task.id,
                user_id: task.user_id,
                trigger: "schedule".to_string(),
                status: "failed".to_string(),
                error_class: Some(reason.to_string()),
                error_message: Some(format!("task paused: {reason} (referenced entity deleted)")),
                notification_id: None,
                workflow_run_id: None,
                conversation_id: None,
                skipped_tools: Vec::new(),
                fired_at: now,
            };
            if let Err(e) = repository::insert_run(pool, run).await {
                tracing::warn!("scheduler.tick: preemptive insert_run {} failed: {e:?}", task.id);
            }
            super::events::emit_task(
                crate::modules::sync::SyncAction::Update,
                task.id,
                task.user_id,
                None,
            );
            continue;
        }

        // Advance `next_run_at` SYNCHRONOUSLY before dispatch (coalesced catch-up:
        // skip missed intervals). This both prevents the next tick from re-claiming
        // the row and — crucially — means a slow/hung dispatch can't starve the
        // loop, because the actual firing is spawned off the tick.
        let kind = task.schedule_kind();
        let next = schedule::next_occurrence(
            kind,
            task.run_at,
            task.cron_expr.as_deref(),
            &task.timezone,
            now,
        )
        .unwrap_or(None);
        // `once` (and a recurring task with no future occurrence) disables after firing.
        let next_to_set = if matches!(kind, ScheduleKind::Once) || next.is_none() {
            None
        } else {
            next
        };
        if let Err(e) = repository::mark_fired(pool, task.id, next_to_set, now, None).await {
            tracing::warn!("scheduler.tick: mark_fired {} failed: {e:?}", task.id);
            continue;
        }

        // Dispatch + record OFF the tick loop so one slow task never blocks the
        // rest of the due batch (or the next sweep).
        let pool = pool.clone();
        let config = config.clone();
        tokio::spawn(async move {
            fire_task(&pool, &config, &task, "schedule", now).await;
        });
    }
    Ok(())
}

/// Dispatch a task's target, then (for scheduled firings) record the outcome +
/// auto-pause on failure, and always append a run-history row + emit sync. The
/// `next_run_at` advance is done by the caller (`run_once`) for scheduled
/// firings; `run-now` deliberately does NOT mutate the task's schedule / failure
/// / change-detection bookkeeping — it is an off-schedule manual trigger.
/// Never panics; a bad task can't kill the sweep.
pub async fn fire_task(
    pool: &PgPool,
    config: &Arc<Config>,
    task: &super::models::ScheduledTask,
    trigger: &str,
    now: DateTime<Utc>,
) {
    // Dispatch (never returns Err — captures failures into the outcome).
    let outcome: DispatchOutcome = dispatch::dispatch(pool, config, task, trigger).await;

    // run-now must NOT touch the task's failure counter / change-detection
    // signature / auto-pause state (handler contract). Only scheduled firings do.
    if trigger != "run_now" {
        let admin_max = settings::get(pool)
            .await
            .map(|s| s.max_consecutive_failures)
            .unwrap_or(5);
        // Auto-pause: a TERMINAL failure (target missing / auth / permission /
        // validation) pauses immediately with its class as the reason; a
        // transient failure pauses only once it crosses the consecutive-failure
        // cap. Success clears (record_outcome resets the counter).
        let pause_reason: Option<String> = if outcome.success {
            None
        } else {
            let class = outcome.error_class.as_deref().unwrap_or("internal");
            if class == "transient" {
                let will_be = task.consecutive_failures + 1;
                super::failure::should_autopause(will_be, admin_max)
                    .then(|| "max_failures".to_string())
            } else {
                Some(class.to_string())
            }
        };
        if let Err(e) = repository::record_outcome(
            pool,
            task.id,
            outcome.status,
            outcome.success,
            outcome.fingerprint.as_deref(),
            outcome.signature.as_ref(),
            pause_reason.as_deref(),
        )
        .await
        {
            tracing::warn!("scheduler.tick: record_outcome {} failed: {e:?}", task.id);
        }
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
        skipped_tools: outcome.skipped_tools.clone(),
        fired_at: now,
    };
    if let Err(e) = repository::insert_run(pool, run).await {
        tracing::warn!("scheduler.tick: insert_run {} failed: {e:?}", task.id);
    }

    // Notify the owner's devices that the task (its runs/state) changed.
    super::events::emit_task(crate::modules::sync::SyncAction::Update, task.id, task.user_id, None);
}
