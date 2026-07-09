//! REST handlers for scheduled-task CRUD + the admin-settings singleton.
//! Task endpoints are owner-scoped via `RequirePermissions<(SchedulerUse,)>`;
//! admin endpoints require `scheduler::admin::{read,manage}`.

use aide::transform::TransformOperation;
use axum::{Json, debug_handler, extract::Path, http::StatusCode};
use chrono::Utc;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::{RequirePermissions, with_permission};
use crate::modules::sync::{SyncAction, SyncOrigin};

use super::events::{emit_admin_settings, emit_task};
use super::models::{
    CreateScheduledTask, ScheduledTask, ScheduledTaskRun, UpdateScheduledTask, MAX_NAME_LEN,
    MAX_PROMPT_LEN,
};
use super::dryrun::{self, TestFireRequest, TestFireResult};
use super::tick;
use super::permissions::{SchedulerAdminManage, SchedulerAdminRead, SchedulerUse};
use super::schedule::{self, ScheduleError, ScheduleKind};
use super::repository;
use super::settings::{self, SchedulerAdminSettings, UpdateSchedulerAdminSettings};

fn parse_kind(s: &str) -> Result<ScheduleKind, (StatusCode, AppError)> {
    match s {
        "once" => Ok(ScheduleKind::Once),
        "recurring" => Ok(ScheduleKind::Recurring),
        _ => Err(AppError::bad_request(
            "SCHEDULER_BAD_SCHEDULE_KIND",
            "schedule_kind must be 'once' or 'recurring'",
        )
        .into()),
    }
}

fn map_schedule_err(e: ScheduleError) -> (StatusCode, AppError) {
    AppError::bad_request("SCHEDULER_INVALID_SCHEDULE", e.to_string()).into()
}

/// POST /api/scheduled-tasks
#[debug_handler]
pub async fn create_task(
    auth: RequirePermissions<(SchedulerUse,)>,
    origin: SyncOrigin,
    Json(body): Json<CreateScheduledTask>,
) -> ApiResult<Json<ScheduledTask>> {
    // Field validation.
    let name = body.name.trim();
    if name.is_empty() || name.len() > MAX_NAME_LEN {
        return Err(AppError::bad_request("SCHEDULER_BAD_NAME", "name is empty or too long").into());
    }
    match body.target_kind.as_str() {
        "workflow" => {
            if body.workflow_id.is_none() {
                return Err(AppError::bad_request(
                    "SCHEDULER_BAD_TARGET",
                    "workflow target requires workflow_id",
                )
                .into());
            }
        }
        "prompt" => match body.prompt.as_deref() {
            Some(p) if !p.trim().is_empty() && p.len() <= MAX_PROMPT_LEN => {}
            _ => {
                return Err(AppError::bad_request(
                    "SCHEDULER_BAD_TARGET",
                    "prompt target requires a non-empty prompt within the size limit",
                )
                .into());
            }
        },
        _ => {
            return Err(AppError::bad_request(
                "SCHEDULER_BAD_TARGET_KIND",
                "target_kind must be 'workflow' or 'prompt'",
            )
            .into());
        }
    }

    let kind = parse_kind(&body.schedule_kind)?;
    let admin = settings::get(Repos.pool()).await?;

    // Quota (422).
    let active = repository::count_active_for_user(Repos.pool(), auth.user.id).await?;
    if active >= i64::from(admin.max_active_tasks_per_user) {
        return Err(AppError::unprocessable_entity(
            "SCHEDULER_TASK_QUOTA",
            format!(
                "active scheduled-task limit ({}) reached",
                admin.max_active_tasks_per_user
            ),
        )
        .into());
    }

    let now = Utc::now();
    schedule::validate_schedule(
        kind,
        body.run_at,
        body.cron_expr.as_deref(),
        &body.timezone,
        i64::from(admin.min_interval_seconds),
        now,
    )
    .map_err(map_schedule_err)?;

    let next_run_at = schedule::next_occurrence(
        kind,
        body.run_at,
        body.cron_expr.as_deref(),
        &body.timezone,
        now,
    )
    .map_err(map_schedule_err)?;

    let task = repository::insert(Repos.pool(), auth.user.id, &body, next_run_at).await?;
    emit_task(SyncAction::Create, task.id, auth.user.id, origin.0);
    Ok((StatusCode::CREATED, Json(task)))
}

pub fn create_task_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SchedulerUse,)>(op)
        .id("ScheduledTask.create")
        .summary("Create a scheduled task")
        .response::<201, Json<ScheduledTask>>()
}

/// GET /api/scheduled-tasks
#[debug_handler]
pub async fn list_tasks(
    auth: RequirePermissions<(SchedulerUse,)>,
) -> ApiResult<Json<Vec<ScheduledTask>>> {
    let tasks = repository::list_for_user(Repos.pool(), auth.user.id).await?;
    Ok((StatusCode::OK, Json(tasks)))
}

pub fn list_tasks_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SchedulerUse,)>(op)
        .id("ScheduledTask.list")
        .summary("List your scheduled tasks")
        .response::<200, Json<Vec<ScheduledTask>>>()
}

/// GET /api/scheduled-tasks/{id}
#[debug_handler]
pub async fn get_task(
    auth: RequirePermissions<(SchedulerUse,)>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ScheduledTask>> {
    let task = repository::get_for_user(Repos.pool(), auth.user.id, id)
        .await?
        .ok_or_else(|| AppError::not_found("Scheduled task"))?;
    Ok((StatusCode::OK, Json(task)))
}

pub fn get_task_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SchedulerUse,)>(op)
        .id("ScheduledTask.get")
        .summary("Get a scheduled task")
        .response::<200, Json<ScheduledTask>>()
}

/// PUT /api/scheduled-tasks/{id}
#[debug_handler]
pub async fn update_task(
    auth: RequirePermissions<(SchedulerUse,)>,
    origin: SyncOrigin,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateScheduledTask>,
) -> ApiResult<Json<ScheduledTask>> {
    let existing = repository::get_for_user(Repos.pool(), auth.user.id, id)
        .await?
        .ok_or_else(|| AppError::not_found("Scheduled task"))?;

    // Recompute next_run_at when a schedule field changed or the task is being
    // (re)enabled — a resumed task should fire going forward, not on a stale
    // past instant.
    let schedule_touched = body.schedule_kind.is_some()
        || body.run_at.is_some()
        || body.cron_expr.is_some()
        || body.timezone.is_some()
        || body.enabled == Some(true);

    let next_arg = if schedule_touched {
        let kind = parse_kind(body.schedule_kind.as_deref().unwrap_or(&existing.schedule_kind))?;
        let run_at = body.run_at.or(existing.run_at);
        let cron = body
            .cron_expr
            .as_deref()
            .or(existing.cron_expr.as_deref());
        let tz = body.timezone.as_deref().unwrap_or(&existing.timezone);
        let admin = settings::get(Repos.pool()).await?;
        let now = Utc::now();
        schedule::validate_schedule(kind, run_at, cron, tz, i64::from(admin.min_interval_seconds), now)
            .map_err(map_schedule_err)?;
        let next = schedule::next_occurrence(kind, run_at, cron, tz, now).map_err(map_schedule_err)?;
        Some(next)
    } else {
        None
    };

    let task = repository::update(Repos.pool(), auth.user.id, id, &body, next_arg)
        .await?
        .ok_or_else(|| AppError::not_found("Scheduled task"))?;
    emit_task(SyncAction::Update, task.id, auth.user.id, origin.0);
    Ok((StatusCode::OK, Json(task)))
}

pub fn update_task_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SchedulerUse,)>(op)
        .id("ScheduledTask.update")
        .summary("Update a scheduled task")
        .response::<200, Json<ScheduledTask>>()
}

/// DELETE /api/scheduled-tasks/{id}
#[debug_handler]
pub async fn delete_task(
    auth: RequirePermissions<(SchedulerUse,)>,
    origin: SyncOrigin,
    Path(id): Path<Uuid>,
) -> ApiResult<()> {
    let affected = repository::delete(Repos.pool(), auth.user.id, id).await?;
    if affected == 0 {
        return Err(AppError::not_found("Scheduled task").into());
    }
    emit_task(SyncAction::Delete, id, auth.user.id, origin.0);
    Ok((StatusCode::NO_CONTENT, ()))
}

pub fn delete_task_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SchedulerUse,)>(op)
        .id("ScheduledTask.delete")
        .summary("Delete a scheduled task")
        .response::<204, ()>()
}

/// POST /api/scheduled-tasks/{id}/run-now — fire immediately, off-schedule.
/// Spawns the firing (a prompt turn can take minutes) and returns 202; the
/// result lands as a notification. Does NOT touch the schedule bookkeeping.
#[debug_handler]
pub async fn run_now(
    auth: RequirePermissions<(SchedulerUse,)>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ScheduledTask>> {
    let task = repository::get_for_user(Repos.pool(), auth.user.id, id)
        .await?
        .ok_or_else(|| AppError::not_found("Scheduled task"))?;
    let config = super::scheduler_config()
        .ok_or_else(|| AppError::internal_error("scheduler not initialized"))?;
    let pool = Repos.pool().clone();
    let fired = task.clone();
    tokio::spawn(async move {
        tick::fire_task(&pool, &config, &fired, "run_now", Utc::now()).await;
    });
    Ok((StatusCode::ACCEPTED, Json(task)))
}

pub fn run_now_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SchedulerUse,)>(op)
        .id("ScheduledTask.runNow")
        .summary("Run a scheduled task now")
        .description("Fires the task immediately, off-schedule; the result lands as a notification.")
        .response::<202, Json<ScheduledTask>>()
}

/// GET /api/scheduled-tasks/{id}/runs — the task's firing history (Runs tab).
#[debug_handler]
pub async fn list_task_runs(
    auth: RequirePermissions<(SchedulerUse,)>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<ScheduledTaskRun>>> {
    let runs = repository::list_runs_for_task(Repos.pool(), auth.user.id, id, 100).await?;
    Ok((StatusCode::OK, Json(runs)))
}

pub fn list_task_runs_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SchedulerUse,)>(op)
        .id("ScheduledTask.listRuns")
        .summary("List a scheduled task's run history")
        .response::<200, Json<Vec<ScheduledTaskRun>>>()
}

/// POST /api/scheduled-tasks/test-fire — run the target ONCE, side-effect-free,
/// and return the result inline (the drawer's "Test" button). Blocks until the
/// turn/run completes; no notification, no history, no schedule change.
#[debug_handler]
pub async fn test_fire(
    auth: RequirePermissions<(SchedulerUse,)>,
    Json(body): Json<TestFireRequest>,
) -> ApiResult<Json<TestFireResult>> {
    let config = super::scheduler_config()
        .ok_or_else(|| AppError::internal_error("scheduler not initialized"))?;
    let result = dryrun::test_fire(Repos.pool(), &config, auth.user.id, &body).await;
    Ok((StatusCode::OK, Json(result)))
}

pub fn test_fire_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SchedulerUse,)>(op)
        .id("ScheduledTask.testFire")
        .summary("Test-fire a task target (dry run)")
        .description("Runs the target once with no side effects; returns the result inline.")
        .response::<200, Json<TestFireResult>>()
}

// ── Admin settings ──────────────────────────────────────────────────────

/// GET /api/scheduler/admin-settings
#[debug_handler]
pub async fn get_admin_settings(
    _auth: RequirePermissions<(SchedulerAdminRead,)>,
) -> ApiResult<Json<SchedulerAdminSettings>> {
    let s = settings::get(Repos.pool()).await?;
    Ok((StatusCode::OK, Json(s)))
}

pub fn get_admin_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SchedulerAdminRead,)>(op)
        .id("SchedulerAdminSettings.get")
        .summary("Get scheduler admin settings")
        .response::<200, Json<SchedulerAdminSettings>>()
}

/// PUT /api/scheduler/admin-settings
#[debug_handler]
pub async fn update_admin_settings(
    _auth: RequirePermissions<(SchedulerAdminManage,)>,
    origin: SyncOrigin,
    Json(body): Json<UpdateSchedulerAdminSettings>,
) -> ApiResult<Json<SchedulerAdminSettings>> {
    // Range validation (clearer than the DB CHECK).
    if !(1..=1000).contains(&body.max_active_tasks_per_user)
        || !(60..=86400).contains(&body.min_interval_seconds)
        || !(1..=100).contains(&body.max_consecutive_failures)
        || !(0..=3650).contains(&body.notification_retention_days)
    {
        return Err(AppError::bad_request(
            "SCHEDULER_SETTINGS_RANGE",
            "one or more settings are out of range",
        )
        .into());
    }
    let s = settings::update(Repos.pool(), &body).await?;
    emit_admin_settings(origin.0);
    Ok((StatusCode::OK, Json(s)))
}

pub fn update_admin_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SchedulerAdminManage,)>(op)
        .id("SchedulerAdminSettings.update")
        .summary("Update scheduler admin settings")
        .response::<200, Json<SchedulerAdminSettings>>()
}
