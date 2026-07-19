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
        // `conversation_deleted` requires a prior SUCCESSFUL fire — one that
        // actually created + bound a conversation (last_status is a success
        // status) whose binding is now NULL (the conversation was deleted). A
        // FAILED first fire also leaves bound_conversation_id NULL but sets
        // last_status='failed' BEFORE create_conversation ran, so it must NOT be
        // mistaken for a deleted conversation (blind-audit fix: that false
        // positive bricked a task on a first-fire blip). Such a task retries on
        // its schedule (counting toward the failure cap) instead.
        "prompt"
            if task.bound_conversation_id.is_none()
                && matches!(
                    task.last_status.as_deref(),
                    Some("completed") | Some("no_change")
                ) =>
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
                result_preview: None,
                change_summary: None,
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

        let kind = task.schedule_kind();

        // ITEM-21/DEC-42: a self-paced task's next fire comes from the model's
        // proposal, not a fixed schedule. Advance-before-dispatch = DISARM the row
        // (`next_run_at` NULL, stays enabled) so the next tick can't re-claim it
        // while its turn runs; the post-dispatch write-back (`fire_task`) re-arms
        // `next_run_at` from the clamped proposal, or self-completes on stop/expiry.
        if matches!(kind, ScheduleKind::SelfPaced) {
            if let Err(e) = repository::disarm_self_paced(pool, task.id, now).await {
                tracing::warn!("scheduler.tick: disarm_self_paced {} failed: {e:?}", task.id);
                continue;
            }
            let pool = pool.clone();
            let config = config.clone();
            tokio::spawn(async move {
                fire_task(&pool, &config, &task, "schedule", now).await;
            });
            continue;
        }

        // Advance `next_run_at` SYNCHRONOUSLY before dispatch (coalesced catch-up:
        // skip missed intervals). This both prevents the next tick from re-claiming
        // the row and — crucially — means a slow/hung dispatch can't starve the
        // loop, because the actual firing is spawned off the tick.
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
        result_preview: outcome.result_preview.clone(),
        change_summary: outcome.change_summary.clone(),
        fired_at: now,
    };
    if let Err(e) = repository::insert_run(pool, run).await {
        tracing::warn!("scheduler.tick: insert_run {} failed: {e:?}", task.id);
    }

    // ITEM-21/DEC-42/44/45: self-paced write-back — a SCHEDULED self-paced firing
    // re-arms `next_run_at` (or self-completes on stop/expiry). run-now is
    // off-schedule (must not touch schedule bookkeeping), so it's excluded.
    if trigger != "run_now" && matches!(task.schedule_kind(), ScheduleKind::SelfPaced) {
        let (min_interval, max_horizon) = settings::get(pool)
            .await
            .map(|s| (i64::from(s.min_interval_seconds), i64::from(s.max_horizon_days)))
            .unwrap_or((300, 7));

        if let Some(condition) = task.completion_condition.as_deref() {
            // ITEM-24 / DEC-61/62/63: GOAL-SEEKING write-back. A single isolated,
            // cheap, INDEPENDENT model call judges this turn's result artifact
            // against the completion condition. `done` self-stops ('completed');
            // `not_done` re-arms another turn (reusing the self-paced clamp) until
            // the `goal_seek_max_turns` cap OR the `max_horizon_days` backstop →
            // stop 'incomplete'. Any evaluator error/timeout is `not_done` — the
            // loop keeps working and NEVER falsely reports success.
            let (eval_model_id, max_turns) =
                match crate::core::Repos.agent.get_admin_settings().await {
                    Ok(s) => (
                        s.goal_eval_model_id.or(task.model_id),
                        i64::from(s.goal_seek_max_turns),
                    ),
                    Err(e) => {
                        tracing::warn!(
                            "scheduler.tick: goal-seek settings read failed for {}: {e:?}",
                            task.id
                        );
                        (task.model_id, 10)
                    }
                };
            // The evaluator sees ONLY the result artifact + the condition.
            let artifact = outcome.result_text.as_deref().unwrap_or("");
            let verdict = match eval_model_id {
                Some(mid) => {
                    super::goal_eval::evaluate(mid, task.user_id, condition, artifact).await
                }
                // No resolvable model → can't confirm completion → keep working.
                None => super::goal_eval::GoalVerdict::NotDone,
            };
            // Turn counter: scheduled runs (incl. the row just inserted above).
            let turns = repository::count_scheduled_runs_for_task(pool, task.id)
                .await
                .unwrap_or(0);
            let (sp_outcome, reason) = match super::goal_eval::decide(
                verdict,
                turns,
                max_turns,
                min_interval,
                max_horizon,
                task.created_at,
                now,
            ) {
                super::goal_eval::GoalOutcome::Done => {
                    (schedule::SelfPacedOutcome::Disable, "completed")
                }
                super::goal_eval::GoalOutcome::Continue(t) => {
                    (schedule::SelfPacedOutcome::Fire(t), "completed")
                }
                super::goal_eval::GoalOutcome::Incomplete => {
                    (schedule::SelfPacedOutcome::Disable, "incomplete")
                }
            };
            if let Err(e) = repository::arm_self_paced(pool, task.id, sp_outcome, now, reason).await
            {
                tracing::warn!("scheduler.tick: goal arm_self_paced {} failed: {e:?}", task.id);
            }
        } else {
            // Plain self-paced: the model-facing `schedule_next` proposal tool is a
            // later tranche; until it lands there is no proposal, so a fired
            // self-paced turn self-completes ('completed').
            let proposal: Option<schedule::SelfPacedProposal> = None;
            let sp_outcome = dispatch::self_paced_writeback(
                proposal.as_ref(),
                min_interval,
                max_horizon,
                task.created_at,
                now,
            );
            if let Err(e) =
                repository::arm_self_paced(pool, task.id, sp_outcome, now, "completed").await
            {
                tracing::warn!("scheduler.tick: arm_self_paced {} failed: {e:?}", task.id);
            }
        }
    }

    // Notify the owner's devices that the task (its runs/state) changed.
    super::events::emit_task(crate::modules::sync::SyncAction::Update, task.id, task.user_id, None);
}

#[cfg(test)]
mod tests {
    use super::preemptive_pause_reason;
    use crate::modules::scheduler::models::ScheduledTask;
    use chrono::Utc;
    use uuid::Uuid;

    /// A minimal `ScheduledTask` with all "referent present, first run" defaults;
    /// individual tests flip the fields the pause decision reads.
    fn base_task() -> ScheduledTask {
        let now = Utc::now();
        ScheduledTask {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            name: "t".to_string(),
            enabled: true,
            paused_reason: None,
            target_kind: "prompt".to_string(),
            workflow_id: None,
            inputs_json: serde_json::json!({}),
            assistant_id: None,
            prompt: Some("hi".to_string()),
            model_id: Some(Uuid::new_v4()),
            schedule_kind: "recurring".to_string(),
            run_at: None,
            cron_expr: Some("0 9 * * 1".to_string()),
            timezone: "UTC".to_string(),
            next_run_at: Some(now),
            last_run_at: None,
            last_status: None,
            consecutive_failures: 0,
            notify_mode: "always".to_string(),
            notify_on: "always".to_string(),
            last_result_fingerprint: None,
            last_result_signature_json: None,
            bound_conversation_id: None,
            completion_condition: None,
            allowed_unattended_tools: serde_json::json!([]),
            created_at: now,
            updated_at: now,
        }
    }

    // TEST-12 (ITEM-7): a prompt task whose bound conversation was deleted AFTER
    // a prior run (bound_conversation_id NULL AND last_status set) pauses
    // `conversation_deleted`; a genuine first-run prompt task (both NULL) is NOT
    // paused (it legitimately creates its bound conversation on first fire).
    #[test]
    fn prompt_conversation_deleted_pauses_but_first_run_does_not() {
        // Deleted-after-a-run: bound NULL + last_status Some → pause.
        let mut deleted = base_task();
        deleted.target_kind = "prompt".to_string();
        deleted.bound_conversation_id = None;
        deleted.last_status = Some("completed".to_string());
        assert_eq!(
            preemptive_pause_reason(&deleted),
            Some("conversation_deleted"),
            "a prompt task whose bound conversation was deleted must pre-emptively pause"
        );

        // Genuine first run: bound NULL + last_status NULL → NOT paused.
        let mut first_run = base_task();
        first_run.target_kind = "prompt".to_string();
        first_run.bound_conversation_id = None;
        first_run.last_status = None;
        assert_eq!(
            preemptive_pause_reason(&first_run),
            None,
            "a first-run prompt task (never fired) must NOT be paused"
        );

        // A prompt task that still has its bound conversation → NOT paused
        // (regardless of last_status).
        let mut healthy = base_task();
        healthy.target_kind = "prompt".to_string();
        healthy.bound_conversation_id = Some(Uuid::new_v4());
        healthy.last_status = Some("completed".to_string());
        assert_eq!(preemptive_pause_reason(&healthy), None);
    }

    // TEST-12: a NULL assistant_id is NEVER a pause reason (NULL = use the user's
    // default assistant), even after a prior run.
    #[test]
    fn null_assistant_is_not_a_pause_reason() {
        let mut t = base_task();
        t.target_kind = "prompt".to_string();
        t.assistant_id = None;
        t.bound_conversation_id = Some(Uuid::new_v4());
        t.last_status = Some("completed".to_string());
        assert_eq!(preemptive_pause_reason(&t), None);
    }

    // TEST-13 (ITEM-7) — decision logic: a workflow task whose workflow_id is
    // NULL pre-emptively pauses `target_missing`; a workflow with its id present
    // is not paused. (Note: the DB CHECK `scheduled_tasks_target_coherent` makes
    // the NULL-workflow_id *row state* unreachable via normal FK cascade, so the
    // end-to-end path can't be integration-driven — this asserts the claim-time
    // decision function that would fire were the state reachable.)
    #[test]
    fn workflow_missing_id_pauses_target_missing() {
        let mut missing = base_task();
        missing.target_kind = "workflow".to_string();
        missing.workflow_id = None;
        missing.prompt = None;
        assert_eq!(
            preemptive_pause_reason(&missing),
            Some("target_missing"),
            "a workflow task with a NULLed workflow_id must pre-emptively pause"
        );

        let mut present = base_task();
        present.target_kind = "workflow".to_string();
        present.workflow_id = Some(Uuid::new_v4());
        present.prompt = None;
        assert_eq!(preemptive_pause_reason(&present), None);
    }
}
