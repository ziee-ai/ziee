//! REST handlers for the notification inbox. All owner-scoped via
//! `RequirePermissions<(NotificationsRead,)>` + `auth.user.id`; a cross-user
//! single-row access returns 404 (never 403 — don't disclose existence).

use aide::transform::TransformOperation;
use axum::{Json, debug_handler, extract::Path, extract::Query, http::StatusCode};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::{RequirePermissions, with_permission};
use crate::modules::sync::{SyncAction, SyncOrigin};

use super::events::emit_bulk_changed;
use super::models::{Notification, NotificationPage, UnreadCount};
use super::permissions::NotificationsRead;
use super::repository;

fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    30
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListNotificationsQuery {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
    /// Return only unread notifications.
    #[serde(default)]
    pub unread_only: bool,
}

/// GET /api/notifications — the caller's inbox, newest-first.
#[debug_handler]
pub async fn list_notifications(
    auth: RequirePermissions<(NotificationsRead,)>,
    Query(q): Query<ListNotificationsQuery>,
) -> ApiResult<Json<NotificationPage>> {
    let (items, total, unread) =
        repository::list_for_user(Repos.pool(), auth.user.id, q.unread_only, q.page.max(1), q.per_page)
            .await?;
    let page = NotificationPage {
        items,
        total,
        unread,
        page: q.page.max(1),
        per_page: q.per_page.clamp(1, 200),
    };
    Ok((StatusCode::OK, Json(page)))
}

pub fn list_notifications_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(NotificationsRead,)>(op)
        .id("Notification.list")
        .summary("List notifications")
        .description("The caller's own notification inbox, newest-first.")
        .response::<200, Json<NotificationPage>>()
}

/// GET /api/notifications/unread-count — the badge count.
#[debug_handler]
pub async fn unread_count(
    auth: RequirePermissions<(NotificationsRead,)>,
) -> ApiResult<Json<UnreadCount>> {
    let unread = repository::unread_count(Repos.pool(), auth.user.id).await?;
    Ok((StatusCode::OK, Json(UnreadCount { unread })))
}

pub fn unread_count_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(NotificationsRead,)>(op)
        .id("Notification.unreadCount")
        .summary("Unread notification count")
        .response::<200, Json<UnreadCount>>()
}

/// GET /api/notifications/{id} — a single notification, owner-scoped.
#[debug_handler]
pub async fn get_notification(
    auth: RequirePermissions<(NotificationsRead,)>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Notification>> {
    let row = repository::get_for_user(Repos.pool(), auth.user.id, id)
        .await?
        .ok_or_else(|| AppError::not_found("Notification"))?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn get_notification_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(NotificationsRead,)>(op)
        .id("Notification.get")
        .summary("Get a notification")
        .response::<200, Json<Notification>>()
}

/// POST /api/notifications/{id}/read — mark one read.
#[debug_handler]
pub async fn mark_read(
    auth: RequirePermissions<(NotificationsRead,)>,
    origin: SyncOrigin,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<UnreadCount>> {
    let affected = repository::mark_read(Repos.pool(), auth.user.id, id).await?;
    if affected > 0 {
        emit_bulk_changed(auth.user.id, SyncAction::Update, origin.0);
    }
    let unread = repository::unread_count(Repos.pool(), auth.user.id).await?;
    Ok((StatusCode::OK, Json(UnreadCount { unread })))
}

pub fn mark_read_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(NotificationsRead,)>(op)
        .id("Notification.markRead")
        .summary("Mark a notification read")
        .response::<200, Json<UnreadCount>>()
}

/// POST /api/notifications/read-all — mark every notification read.
#[debug_handler]
pub async fn mark_all_read(
    auth: RequirePermissions<(NotificationsRead,)>,
    origin: SyncOrigin,
) -> ApiResult<Json<UnreadCount>> {
    let affected = repository::mark_all_read(Repos.pool(), auth.user.id).await?;
    if affected > 0 {
        emit_bulk_changed(auth.user.id, SyncAction::Update, origin.0);
    }
    Ok((StatusCode::OK, Json(UnreadCount { unread: 0 })))
}

pub fn mark_all_read_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(NotificationsRead,)>(op)
        .id("Notification.markAllRead")
        .summary("Mark all notifications read")
        .response::<200, Json<UnreadCount>>()
}

/// DELETE /api/notifications/{id} — remove one notification.
#[debug_handler]
pub async fn delete_notification(
    auth: RequirePermissions<(NotificationsRead,)>,
    origin: SyncOrigin,
    Path(id): Path<Uuid>,
) -> ApiResult<()> {
    let affected = repository::delete(Repos.pool(), auth.user.id, id).await?;
    if affected == 0 {
        return Err(AppError::not_found("Notification").into());
    }
    emit_bulk_changed(auth.user.id, SyncAction::Delete, origin.0);
    Ok((StatusCode::NO_CONTENT, ()))
}

pub fn delete_notification_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(NotificationsRead,)>(op)
        .id("Notification.delete")
        .summary("Delete a notification")
        .response::<204, ()>()
}
