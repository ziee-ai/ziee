//! Scheduler admin settings — the `scheduler_admin_settings` singleton
//! (deployment-wide quota / cadence floor / failure cap / notification
//! retention). Read fresh each create/tick so admin edits take effect without a
//! restart. Mirrors `memory` admin settings.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::common::AppError;

/// The singleton settings row.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct SchedulerAdminSettings {
    pub max_active_tasks_per_user: i32,
    pub min_interval_seconds: i32,
    pub max_consecutive_failures: i32,
    pub notification_retention_days: i32,
    /// ITEM-21 / DEC-45: the absolute self-paced backstop (days). A self-paced
    /// task's model-proposed delay is clamped to at most this, and the task
    /// self-stops `max_horizon_days` after creation. Default 7, range 1..=365.
    pub max_horizon_days: i32,
    pub updated_at: DateTime<Utc>,
}

/// Admin update body (all fields required — the form always sends the full set).
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UpdateSchedulerAdminSettings {
    pub max_active_tasks_per_user: i32,
    pub min_interval_seconds: i32,
    pub max_consecutive_failures: i32,
    pub notification_retention_days: i32,
    pub max_horizon_days: i32,
}

/// Read the singleton (id = TRUE).
pub async fn get(pool: &PgPool) -> Result<SchedulerAdminSettings, AppError> {
    let row = sqlx::query_as!(
        SchedulerAdminSettings,
        r#"
        SELECT max_active_tasks_per_user, min_interval_seconds,
               max_consecutive_failures, notification_retention_days,
               max_horizon_days,
               updated_at as "updated_at: _"
        FROM scheduler_admin_settings WHERE id = TRUE
        "#,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

/// Update the singleton. The DB CHECK constraints are the last line; the handler
/// validates ranges first for clearer errors.
pub async fn update(
    pool: &PgPool,
    upd: &UpdateSchedulerAdminSettings,
) -> Result<SchedulerAdminSettings, AppError> {
    let row = sqlx::query_as!(
        SchedulerAdminSettings,
        r#"
        UPDATE scheduler_admin_settings SET
            max_active_tasks_per_user = $1,
            min_interval_seconds = $2,
            max_consecutive_failures = $3,
            notification_retention_days = $4,
            max_horizon_days = $5,
            updated_at = NOW()
        WHERE id = TRUE
        RETURNING max_active_tasks_per_user, min_interval_seconds,
                  max_consecutive_failures, notification_retention_days,
                  max_horizon_days,
                  updated_at as "updated_at: _"
        "#,
        upd.max_active_tasks_per_user,
        upd.min_interval_seconds,
        upd.max_consecutive_failures,
        upd.notification_retention_days,
        upd.max_horizon_days,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}
