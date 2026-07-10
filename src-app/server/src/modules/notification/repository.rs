//! DB access for `notifications` (free functions over `&PgPool`, mirroring
//! `mcp/tool_calls/repository.rs`). Every query is owner-scoped.

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

use super::models::{NewNotification, Notification};

/// Insert one notification, returning the full row.
pub async fn insert(pool: &PgPool, n: NewNotification) -> Result<Notification, AppError> {
    let row = sqlx::query_as!(
        Notification,
        r#"
        INSERT INTO notifications (
            user_id, kind, title, body, interrupt,
            scheduled_task_id, workflow_run_id, conversation_id
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
        RETURNING
            id, user_id, kind, title, body, interrupt,
            scheduled_task_id, workflow_run_id, conversation_id,
            read_at as "read_at: _",
            created_at as "created_at: _"
        "#,
        n.user_id,
        n.kind,
        n.title,
        n.body,
        n.interrupt,
        n.scheduled_task_id,
        n.workflow_run_id,
        n.conversation_id,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

/// A single notification, owner-scoped (None if not found / not owned).
pub async fn get_for_user(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<Notification>, AppError> {
    let row = sqlx::query_as!(
        Notification,
        r#"
        SELECT
            id, user_id, kind, title, body, interrupt,
            scheduled_task_id, workflow_run_id, conversation_id,
            read_at as "read_at: _",
            created_at as "created_at: _"
        FROM notifications
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

/// List a user's inbox, newest-first. Returns `(rows, total, unread)`.
pub async fn list_for_user(
    pool: &PgPool,
    user_id: Uuid,
    unread_only: bool,
    page: i64,
    per_page: i64,
) -> Result<(Vec<Notification>, i64, i64), AppError> {
    let per_page = per_page.clamp(1, 200);
    let offset = (page - 1).max(0) * per_page;

    let rows = sqlx::query_as!(
        Notification,
        r#"
        SELECT
            id, user_id, kind, title, body, interrupt,
            scheduled_task_id, workflow_run_id, conversation_id,
            read_at as "read_at: _",
            created_at as "created_at: _"
        FROM notifications
        WHERE user_id = $1
          AND ($2::bool = FALSE OR read_at IS NULL)
        ORDER BY created_at DESC
        LIMIT $3 OFFSET $4
        "#,
        user_id,
        unread_only,
        per_page,
        offset,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    let counts = sqlx::query!(
        r#"
        SELECT
            count(*) AS "total!",
            count(*) FILTER (WHERE read_at IS NULL) AS "unread!"
        FROM notifications
        WHERE user_id = $1
        "#,
        user_id,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok((rows, counts.total, counts.unread))
}

/// The user's unread count.
pub async fn unread_count(pool: &PgPool, user_id: Uuid) -> Result<i64, AppError> {
    let row = sqlx::query!(
        r#"SELECT count(*) AS "n!" FROM notifications WHERE user_id = $1 AND read_at IS NULL"#,
        user_id,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row.n)
}

/// Mark one notification read (owner-scoped, idempotent). Returns rows affected.
pub async fn mark_read(pool: &PgPool, user_id: Uuid, id: Uuid) -> Result<u64, AppError> {
    let res = sqlx::query!(
        r#"UPDATE notifications SET read_at = NOW()
           WHERE id = $1 AND user_id = $2 AND read_at IS NULL"#,
        id,
        user_id,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(res.rows_affected())
}

/// Mark all of a user's notifications read. Returns rows affected.
pub async fn mark_all_read(pool: &PgPool, user_id: Uuid) -> Result<u64, AppError> {
    let res = sqlx::query!(
        r#"UPDATE notifications SET read_at = NOW()
           WHERE user_id = $1 AND read_at IS NULL"#,
        user_id,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(res.rows_affected())
}

/// Delete one notification (owner-scoped). Returns rows affected.
pub async fn delete(pool: &PgPool, user_id: Uuid, id: Uuid) -> Result<u64, AppError> {
    let res = sqlx::query!(
        r#"DELETE FROM notifications WHERE id = $1 AND user_id = $2"#,
        id,
        user_id,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(res.rows_affected())
}

/// Prune notifications older than `cutoff` (retention). Returns rows deleted.
/// `cutoff` is `time::OffsetDateTime` — sqlx maps `timestamptz` params to the
/// `time` crate in bare `query!` (chrono is used only in row structs via the
/// `: _` override), mirroring `mcp/tool_calls/prune.rs`.
pub async fn prune_older_than(
    pool: &PgPool,
    cutoff: time::OffsetDateTime,
) -> Result<u64, AppError> {
    let res = sqlx::query!(
        r#"DELETE FROM notifications WHERE created_at < $1"#,
        cutoff,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(res.rows_affected())
}
