//! DB access for `scheduled_tasks` + `scheduled_task_runs` (free functions over
//! `&PgPool`, mirroring `mcp/tool_calls/repository.rs`). Every task query is
//! owner-scoped; the tick's due-claim is the one system-wide query.
//!
//! chrono is used in the row structs via the `: _` column override; bare
//! `query!` timestamptz params take `time::OffsetDateTime`.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

use super::models::{
    CreateScheduledTask, NewTaskRun, ScheduledTask, ScheduledTaskRun, UpdateScheduledTask,
};

/// chrono→time bridge: sqlx binds `timestamptz` params as `time::OffsetDateTime`
/// (the module is chrono-native because croner is), so convert at the bind edge.
fn to_offset(dt: DateTime<Utc>) -> time::OffsetDateTime {
    time::OffsetDateTime::from_unix_timestamp_nanos(dt.timestamp_nanos_opt().unwrap_or(0) as i128)
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
}
fn to_offset_opt(dt: Option<DateTime<Utc>>) -> Option<time::OffsetDateTime> {
    dt.map(to_offset)
}

// The full `scheduled_tasks` projection with chrono/JSONB overrides. Repeated
// inline in each `query_as!` (the macro needs a string literal).
//   id, user_id, name, enabled, paused_reason, target_kind, workflow_id,
//   inputs_json:_, assistant_id, prompt, model_id, schedule_kind, run_at:_,
//   cron_expr, timezone, next_run_at:_, last_run_at:_, last_status,
//   consecutive_failures, notify_mode, notify_on, last_result_fingerprint,
//   last_result_signature_json:_, bound_conversation_id, created_at:_, updated_at:_

/// Insert a new task with its computed first `next_run_at`.
#[allow(clippy::too_many_arguments)]
pub async fn insert(
    pool: &PgPool,
    user_id: Uuid,
    req: &CreateScheduledTask,
    next_run_at: Option<DateTime<Utc>>,
) -> Result<ScheduledTask, AppError> {
    let row = sqlx::query_as!(
        ScheduledTask,
        r#"
        INSERT INTO scheduled_tasks (
            user_id, name, target_kind, workflow_id, inputs_json,
            assistant_id, prompt, model_id, schedule_kind, run_at,
            cron_expr, timezone, next_run_at, notify_mode, notify_on,
            allowed_unattended_tools
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)
        RETURNING
            id, user_id, name, enabled, paused_reason, target_kind, workflow_id,
            inputs_json as "inputs_json: _", assistant_id, prompt, model_id,
            schedule_kind, run_at as "run_at: _", cron_expr, timezone,
            next_run_at as "next_run_at: _", last_run_at as "last_run_at: _",
            last_status, consecutive_failures, notify_mode, notify_on,
            last_result_fingerprint,
            last_result_signature_json as "last_result_signature_json: _",
            bound_conversation_id,
            allowed_unattended_tools as "allowed_unattended_tools: _",
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        "#,
        user_id,
        req.name,
        req.target_kind,
        req.workflow_id,
        req.inputs_json,
        req.assistant_id,
        req.prompt,
        req.model_id,
        req.schedule_kind,
        to_offset_opt(req.run_at),
        req.cron_expr,
        req.timezone,
        to_offset_opt(next_run_at),
        req.notify_mode,
        req.notify_on,
        serde_json::to_value(&req.allowed_unattended_tools)
            .unwrap_or_else(|_| serde_json::json!([])),
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

/// One task, owner-scoped.
pub async fn get_for_user(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<ScheduledTask>, AppError> {
    let row = sqlx::query_as!(
        ScheduledTask,
        r#"
        SELECT
            id, user_id, name, enabled, paused_reason, target_kind, workflow_id,
            inputs_json as "inputs_json: _", assistant_id, prompt, model_id,
            schedule_kind, run_at as "run_at: _", cron_expr, timezone,
            next_run_at as "next_run_at: _", last_run_at as "last_run_at: _",
            last_status, consecutive_failures, notify_mode, notify_on,
            last_result_fingerprint,
            last_result_signature_json as "last_result_signature_json: _",
            bound_conversation_id,
            allowed_unattended_tools as "allowed_unattended_tools: _",
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        FROM scheduled_tasks
        WHERE id = $1 AND user_id = $2
        "#,
        id,
        user_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

/// A user's tasks, newest-first.
pub async fn list_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<ScheduledTask>, AppError> {
    let rows = sqlx::query_as!(
        ScheduledTask,
        r#"
        SELECT
            id, user_id, name, enabled, paused_reason, target_kind, workflow_id,
            inputs_json as "inputs_json: _", assistant_id, prompt, model_id,
            schedule_kind, run_at as "run_at: _", cron_expr, timezone,
            next_run_at as "next_run_at: _", last_run_at as "last_run_at: _",
            last_status, consecutive_failures, notify_mode, notify_on,
            last_result_fingerprint,
            last_result_signature_json as "last_result_signature_json: _",
            bound_conversation_id,
            allowed_unattended_tools as "allowed_unattended_tools: _",
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        FROM scheduled_tasks
        WHERE user_id = $1
        ORDER BY created_at DESC
        "#,
        user_id,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(rows)
}

/// Count a user's ENABLED (active) tasks — for the create-time quota.
pub async fn count_active_for_user(pool: &PgPool, user_id: Uuid) -> Result<i64, AppError> {
    let row = sqlx::query!(
        r#"SELECT count(*) AS "n!" FROM scheduled_tasks WHERE user_id = $1 AND enabled"#,
        user_id,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row.n)
}

/// Apply a partial update (COALESCE — only present fields change). Re-enabling
/// (`enabled = true`) clears any auto-pause reason. `next_run_at` is recomputed
/// by the caller when the schedule changed. Owner-scoped.
pub async fn update(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
    upd: &UpdateScheduledTask,
    next_run_at: Option<Option<DateTime<Utc>>>,
) -> Result<Option<ScheduledTask>, AppError> {
    // next_run_at is tri-state: None = don't touch; Some(v) = set to v.
    let (set_next, next_val) = match next_run_at {
        Some(v) => (true, v),
        None => (false, None),
    };
    let row = sqlx::query_as!(
        ScheduledTask,
        r#"
        UPDATE scheduled_tasks SET
            name          = COALESCE($3, name),
            enabled       = COALESCE($4, enabled),
            paused_reason = CASE WHEN $4 IS TRUE THEN NULL ELSE paused_reason END,
            inputs_json   = COALESCE($5, inputs_json),
            assistant_id  = COALESCE($6, assistant_id),
            prompt        = COALESCE($7, prompt),
            model_id      = COALESCE($8, model_id),
            schedule_kind = COALESCE($9, schedule_kind),
            run_at        = COALESCE($10, run_at),
            cron_expr     = COALESCE($11, cron_expr),
            timezone      = COALESCE($12, timezone),
            notify_mode   = COALESCE($13, notify_mode),
            notify_on     = COALESCE($14, notify_on),
            next_run_at   = CASE WHEN $15 THEN $16 ELSE next_run_at END,
            allowed_unattended_tools = COALESCE($17, allowed_unattended_tools),
            updated_at    = NOW()
        WHERE id = $1 AND user_id = $2
        RETURNING
            id, user_id, name, enabled, paused_reason, target_kind, workflow_id,
            inputs_json as "inputs_json: _", assistant_id, prompt, model_id,
            schedule_kind, run_at as "run_at: _", cron_expr, timezone,
            next_run_at as "next_run_at: _", last_run_at as "last_run_at: _",
            last_status, consecutive_failures, notify_mode, notify_on,
            last_result_fingerprint,
            last_result_signature_json as "last_result_signature_json: _",
            bound_conversation_id,
            allowed_unattended_tools as "allowed_unattended_tools: _",
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        "#,
        id,
        user_id,
        upd.name,
        upd.enabled,
        upd.inputs_json,
        upd.assistant_id,
        upd.prompt,
        upd.model_id,
        upd.schedule_kind,
        to_offset_opt(upd.run_at),
        upd.cron_expr,
        upd.timezone,
        upd.notify_mode,
        upd.notify_on,
        set_next,
        to_offset_opt(next_val),
        upd.allowed_unattended_tools
            .as_ref()
            .map(|v| serde_json::to_value(v).unwrap_or_else(|_| serde_json::json!([]))),
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

/// Delete a task (owner-scoped). Returns rows affected.
pub async fn delete(pool: &PgPool, user_id: Uuid, id: Uuid) -> Result<u64, AppError> {
    let res = sqlx::query!(
        r#"DELETE FROM scheduled_tasks WHERE id = $1 AND user_id = $2"#,
        id,
        user_id,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(res.rows_affected())
}

/// Claim up to `limit` due tasks for firing: enabled, not paused, with
/// `next_run_at <= now`. `FOR UPDATE SKIP LOCKED` is retained as defense-in-depth
/// for a future multi-instance deployment, but in the single-process model
/// (DEC-10) the row lock is released when this SELECT auto-commits; the caller
/// (`tick::run_once`) then advances `next_run_at` via `mark_fired` immediately —
/// BEFORE spawning the dispatch — so the next sequential tick can't re-claim the
/// row and a slow dispatch can't starve the loop. (A crash in the claim→advance
/// window leaves the row un-advanced → at-least-once, not exactly-once; a true
/// single-tx claim+advance would be required for exactly-once across replicas.)
pub async fn claim_due_tasks(
    pool: &PgPool,
    now: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<ScheduledTask>, AppError> {
    let rows = sqlx::query_as!(
        ScheduledTask,
        r#"
        SELECT
            id, user_id, name, enabled, paused_reason, target_kind, workflow_id,
            inputs_json as "inputs_json: _", assistant_id, prompt, model_id,
            schedule_kind, run_at as "run_at: _", cron_expr, timezone,
            next_run_at as "next_run_at: _", last_run_at as "last_run_at: _",
            last_status, consecutive_failures, notify_mode, notify_on,
            last_result_fingerprint,
            last_result_signature_json as "last_result_signature_json: _",
            bound_conversation_id,
            allowed_unattended_tools as "allowed_unattended_tools: _",
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        FROM scheduled_tasks
        WHERE enabled
          AND paused_reason IS NULL
          AND next_run_at IS NOT NULL
          AND next_run_at <= $1
        ORDER BY next_run_at ASC
        LIMIT $2
        FOR UPDATE SKIP LOCKED
        "#,
        to_offset(now),
        limit,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(rows)
}

/// Advance a task after a firing was claimed: set the next fire instant (NULL
/// disables a spent `once`/no-occurrence task) and stamp `last_run_at`. Called
/// immediately after claim to guarantee no double-fire even if dispatch later
/// crashes (DEC-16). `paused_reason` is set when auto-pausing (target missing).
pub async fn mark_fired(
    pool: &PgPool,
    id: Uuid,
    next_run_at: Option<DateTime<Utc>>,
    fired_at: DateTime<Utc>,
    paused_reason: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE scheduled_tasks SET
            next_run_at   = $2,
            last_run_at   = $3,
            -- Disable the task when it is paused (reason set) OR spent — a `once`
            -- task (or a recurring task with no further occurrence) has no future
            -- run, so it flips to disabled per the documented "once disables after
            -- firing" contract (tick.rs). A recurring task keeps its next_run_at
            -- (non-NULL) and stays enabled.
            enabled       = CASE
                                WHEN $4::text IS NOT NULL THEN FALSE
                                WHEN $2::timestamptz IS NULL THEN FALSE
                                ELSE enabled
                            END,
            -- ITEM-10: a spent task (no further occurrence → next_run_at NULL)
            -- with no explicit pause reason is marked 'completed' so the UI can
            -- show "Completed" rather than an ambiguous disabled/paused state.
            paused_reason = CASE
                                WHEN $4::text IS NOT NULL THEN $4
                                WHEN $2::timestamptz IS NULL THEN 'completed'
                                ELSE paused_reason
                            END,
            updated_at    = NOW()
        WHERE id = $1
        "#,
        id,
        to_offset_opt(next_run_at),
        to_offset(fired_at),
        paused_reason,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

/// Record the outcome of a firing on the task row: `last_status`, the failure
/// counter (reset on success, incremented on failure), the change-detection
/// signature (on success), and an auto-pause when the failure cap is crossed.
pub async fn record_outcome(
    pool: &PgPool,
    id: Uuid,
    status: &str,
    success: bool,
    fingerprint: Option<&str>,
    signature_json: Option<&serde_json::Value>,
    pause_reason: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE scheduled_tasks SET
            last_status = $2,
            consecutive_failures = CASE WHEN $3 THEN 0 ELSE consecutive_failures + 1 END,
            last_result_fingerprint = CASE WHEN $3 THEN COALESCE($4, last_result_fingerprint) ELSE last_result_fingerprint END,
            last_result_signature_json = CASE WHEN $3 THEN COALESCE($5, last_result_signature_json) ELSE last_result_signature_json END,
            enabled = CASE WHEN $6::text IS NOT NULL THEN FALSE ELSE enabled END,
            paused_reason = COALESCE($6, paused_reason),
            updated_at = NOW()
        WHERE id = $1
        "#,
        id,
        status,
        success,
        fingerprint,
        signature_json,
        pause_reason,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

/// Insert a per-firing audit row (`scheduled_task_runs`). Returns its id.
pub async fn insert_run(pool: &PgPool, run: NewTaskRun) -> Result<Uuid, AppError> {
    let row = sqlx::query!(
        r#"
        INSERT INTO scheduled_task_runs (
            scheduled_task_id, user_id, trigger, status, error_class,
            error_message, notification_id, workflow_run_id, conversation_id,
            skipped_tools, result_preview, change_summary_json, fired_at, finished_at
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13, NOW())
        RETURNING id
        "#,
        run.scheduled_task_id,
        run.user_id,
        run.trigger,
        run.status,
        run.error_class,
        run.error_message,
        run.notification_id,
        run.workflow_run_id,
        run.conversation_id,
        serde_json::to_value(&run.skipped_tools).unwrap_or_else(|_| serde_json::json!([])),
        run.result_preview,
        run.change_summary,
        to_offset(run.fired_at),
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row.id)
}

/// ITEM-8/DEC-7: delete `scheduled_task_runs` older than `cutoff` (retention
/// prune). Returns rows deleted. Reuses the admin `notification_retention_days`
/// window (migration 144's documented-but-unimplemented "pruned alongside
/// notifications" intent).
pub async fn prune_runs_older_than(
    pool: &PgPool,
    cutoff: DateTime<Utc>,
) -> Result<u64, AppError> {
    let res = sqlx::query!(
        r#"DELETE FROM scheduled_task_runs WHERE fired_at < $1"#,
        to_offset(cutoff),
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(res.rows_affected())
}

/// A task's firing history, newest-first (owner-scoped via the task).
/// ITEM-41: a page of run history, newest-first, owner-scoped, with the total
/// count for the `ListPagination` "Showing N of M". `page` is 1-based; `per_page`
/// is clamped to a sane band.
pub async fn list_runs_for_task(
    pool: &PgPool,
    user_id: Uuid,
    task_id: Uuid,
    page: i64,
    per_page: i64,
) -> Result<(Vec<ScheduledTaskRun>, i64), AppError> {
    let per_page = per_page.clamp(1, 200);
    let offset = (page - 1).max(0) * per_page;
    let rows = sqlx::query_as!(
        ScheduledTaskRun,
        r#"
        SELECT
            id, scheduled_task_id, user_id, trigger, status, error_class,
            error_message, notification_id, workflow_run_id, conversation_id,
            skipped_tools as "skipped_tools: _",
            result_preview, change_summary_json,
            fired_at as "fired_at: _", finished_at as "finished_at: _"
        FROM scheduled_task_runs
        WHERE scheduled_task_id = $1 AND user_id = $2
        ORDER BY fired_at DESC
        LIMIT $3 OFFSET $4
        "#,
        task_id,
        user_id,
        per_page,
        offset,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    let total = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM scheduled_task_runs WHERE scheduled_task_id = $1 AND user_id = $2"#,
        task_id,
        user_id,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok((rows, total))
}

/// Fetch a single run row owner-scoped (its `user_id` denormalizes the task
/// owner). Used by the continue-in-chat endpoint (cross-user → None → 404).
pub async fn get_run_for_user(
    pool: &PgPool,
    user_id: Uuid,
    run_id: Uuid,
) -> Result<Option<ScheduledTaskRun>, AppError> {
    let row = sqlx::query_as!(
        ScheduledTaskRun,
        r#"
        SELECT
            id, scheduled_task_id, user_id, trigger, status, error_class,
            error_message, notification_id, workflow_run_id, conversation_id,
            skipped_tools as "skipped_tools: _",
            result_preview, change_summary_json,
            fired_at as "fired_at: _", finished_at as "finished_at: _"
        FROM scheduled_task_runs
        WHERE id = $1 AND user_id = $2
        "#,
        run_id,
        user_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

/// Set the bound conversation for a prompt-kind task (first firing).
pub async fn set_bound_conversation(
    pool: &PgPool,
    id: Uuid,
    conversation_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"UPDATE scheduled_tasks SET bound_conversation_id = $2, updated_at = NOW() WHERE id = $1"#,
        id,
        conversation_id,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // These exercise the real SQL of the repository against a live migrated DB.
    // DB-gated (mirrors `web_search/repository.rs`): soft-skips when `DATABASE_URL`
    // is unset / unreachable so `cargo test --lib` without Postgres stays green;
    // runs for real wherever `DATABASE_URL` points at a migrated DB. Prune has NO
    // REST/re-export seam, so an in-source DB test is the only real-path home for
    // TEST-14/15 (the private `prune_runs_older_than` DELETE predicate).
    async fn db() -> Option<PgPool> {
        let url = std::env::var("DATABASE_URL").ok()?;
        match sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
        {
            Ok(p) => Some(p),
            Err(e) => {
                eprintln!("skip: DB unreachable ({e})");
                None
            }
        }
    }

    /// Insert a minimal, real `users` row (only username/email are NOT NULL
    /// without a default). FK target for a task row.
    async fn seed_user(pool: &PgPool) -> Uuid {
        let uniq = Uuid::new_v4().to_string();
        let short = &uniq[..8];
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id",
        )
        .bind(format!("sched_repo_{short}"))
        .bind(format!("sched_repo_{short}@example.test"))
        .fetch_one(pool)
        .await
        .expect("seed user");
        id
    }

    /// Insert a minimal prompt-kind task (model_id NULL — nullable FK) satisfying
    /// both CHECK constraints. `kind` ∈ {"once","recurring"}.
    async fn seed_prompt_task(pool: &PgPool, user_id: Uuid, kind: &str, enabled: bool) -> Uuid {
        let now = Utc::now();
        let (run_at, cron, next): (Option<time::OffsetDateTime>, Option<String>, Option<time::OffsetDateTime>) =
            if kind == "once" {
                (Some(to_offset(now)), None, None)
            } else {
                (None, Some("0 9 * * 1".to_string()), Some(to_offset(now)))
            };
        let id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO scheduled_tasks
                (user_id, name, target_kind, prompt, schedule_kind,
                 run_at, cron_expr, timezone, enabled, next_run_at)
            VALUES ($1, $2, 'prompt', 'hi', $3, $4, $5, 'UTC', $6, $7)
            RETURNING id
            "#,
        )
        .bind(user_id)
        .bind("repo-test")
        .bind(kind)
        .bind(run_at)
        .bind(cron)
        .bind(enabled)
        .bind(next)
        .fetch_one(pool)
        .await
        .expect("seed task");
        id
    }

    fn run_for(task: Uuid, user: Uuid, fired_at: DateTime<Utc>) -> NewTaskRun {
        NewTaskRun {
            scheduled_task_id: task,
            user_id: user,
            trigger: "schedule".to_string(),
            status: "completed".to_string(),
            error_class: None,
            error_message: None,
            notification_id: None,
            workflow_run_id: None,
            conversation_id: None,
            skipped_tools: Vec::new(),
            result_preview: Some("preview".to_string()),
            change_summary: Some(serde_json::json!({"changed": true, "new_count": 0, "new_items": []})),
            fired_at,
        }
    }

    // TEST-11 (ITEM-6): the active-task count excludes DISABLED rows, so the
    // re-enable quota check has no off-by-one (a disabled task being re-enabled
    // isn't already in the count).
    #[tokio::test]
    async fn count_active_excludes_disabled_rows() {
        let Some(pool) = db().await else { return };
        let user = seed_user(&pool).await;
        seed_prompt_task(&pool, user, "recurring", true).await; // active
        seed_prompt_task(&pool, user, "recurring", false).await; // disabled → excluded

        let n = count_active_for_user(&pool, user).await.expect("count");
        assert_eq!(n, 1, "count_active_for_user must exclude the disabled row");
    }

    // TEST-19 (ITEM-10): `mark_fired` marks a SPENT once-kind task (no next
    // occurrence → next_run_at NULL) `paused_reason='completed'` + disabled;
    // a recurring task that still has a next occurrence stays enabled with a
    // NULL paused_reason.
    #[tokio::test]
    async fn mark_fired_completes_spent_once_and_leaves_recurring_unpaused() {
        let Some(pool) = db().await else { return };
        let user = seed_user(&pool).await;

        let once = seed_prompt_task(&pool, user, "once", true).await;
        mark_fired(&pool, once, None, Utc::now(), None)
            .await
            .expect("mark once");
        let t = get_for_user(&pool, user, once)
            .await
            .expect("get once")
            .expect("once row");
        assert!(!t.enabled, "a spent once task is disabled");
        assert_eq!(
            t.paused_reason.as_deref(),
            Some("completed"),
            "a spent once task is marked 'completed'"
        );

        let rec = seed_prompt_task(&pool, user, "recurring", true).await;
        let future = Utc::now() + chrono::Duration::days(7);
        mark_fired(&pool, rec, Some(future), Utc::now(), None)
            .await
            .expect("mark recurring");
        let t = get_for_user(&pool, user, rec)
            .await
            .expect("get rec")
            .expect("rec row");
        assert!(t.enabled, "a recurring task with a next run stays enabled");
        assert!(
            t.paused_reason.is_none(),
            "a recurring task with a next run keeps a NULL paused_reason"
        );
    }

    // TEST-14 / TEST-15 (ITEM-8): `prune_runs_older_than` deletes ONLY runs whose
    // `fired_at < cutoff`; newer runs are retained.
    #[tokio::test]
    async fn prune_deletes_only_runs_older_than_cutoff() {
        let Some(pool) = db().await else { return };
        let user = seed_user(&pool).await;
        let task = seed_prompt_task(&pool, user, "recurring", true).await;

        let old_at = Utc::now() - chrono::Duration::days(40);
        let recent_at = Utc::now() - chrono::Duration::days(1);
        let old_run = insert_run(&pool, run_for(task, user, old_at))
            .await
            .expect("old run");
        let new_run = insert_run(&pool, run_for(task, user, recent_at))
            .await
            .expect("new run");

        // Cutoff = 30 days ago: the 40-day-old run is deleted, the 1-day-old kept.
        let cutoff = Utc::now() - chrono::Duration::days(30);
        let deleted = prune_runs_older_than(&pool, cutoff).await.expect("prune");
        assert!(deleted >= 1, "at least the 40-day-old run is pruned");

        let (remaining, _total) = list_runs_for_task(&pool, user, task, 1, 100)
            .await
            .expect("list runs");
        let ids: Vec<Uuid> = remaining.iter().map(|r| r.id).collect();
        assert!(!ids.contains(&old_run), "the old run (fired_at < cutoff) is pruned");
        assert!(ids.contains(&new_run), "the recent run (fired_at >= cutoff) is retained");
    }

    // TEST-43 (ITEM-41): `list_runs_for_task` pages newest-first with a correct total.
    #[tokio::test]
    async fn list_runs_paginates_newest_first_with_total() {
        let Some(pool) = db().await else { return };
        let user = seed_user(&pool).await;
        let task = seed_prompt_task(&pool, user, "paged", true).await;

        // Insert 3 runs at increasing fired_at (r2 newest).
        let base = Utc::now() - chrono::Duration::days(3);
        let mut ids = Vec::new();
        for i in 0..3 {
            let id = insert_run(&pool, run_for(task, user, base + chrono::Duration::days(i)))
                .await
                .expect("run");
            ids.push(id);
        }

        // Page 1, per_page 2 → the two newest (r2, r1); total = 3.
        let (page1, total) = list_runs_for_task(&pool, user, task, 1, 2).await.expect("page1");
        assert_eq!(total, 3, "total counts all runs");
        assert_eq!(page1.len(), 2, "per_page bounds the page");
        assert_eq!(page1[0].id, ids[2], "newest first");
        assert_eq!(page1[1].id, ids[1]);

        // Page 2 → the remaining oldest run, no overlap.
        let (page2, _t) = list_runs_for_task(&pool, user, task, 2, 2).await.expect("page2");
        assert_eq!(page2.len(), 1);
        assert_eq!(page2[0].id, ids[0], "oldest on the last page");
    }
}
