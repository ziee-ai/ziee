//! The single write seam for the inbox: insert a notification row then emit the
//! realtime sync frame so every one of the owner's devices refetches. Producers
//! (the scheduler, and future background completions) call `create_and_emit`.

use sqlx::PgPool;

use crate::common::AppError;
use crate::modules::sync::{Audience, SyncAction, SyncEntity, publish as sync_publish};

use super::models::{NewNotification, Notification};

/// Insert a notification and notify the owner's devices.
///
/// Owner-scoped, `origin = None` (a background producer has no originating
/// request connection, so even the triggering device refetches). The durable
/// row is always written; the `interrupt` flag on the row (set via
/// `NewNotification::silent`) is what the client consults to decide whether to
/// raise a live toast — the sync frame itself is payload-free.
pub async fn create_and_emit(
    pool: &PgPool,
    new: NewNotification,
) -> Result<Notification, AppError> {
    let user_id = new.user_id;
    let row = super::repository::insert(pool, new).await?;

    sync_publish(
        SyncEntity::Notification,
        SyncAction::Create,
        row.id,
        Audience::owner(user_id),
        None,
    );

    Ok(row)
}

/// Emit a bulk "the inbox changed, reload" signal (nil id) — used after
/// mark-all-read / prune where no single row addresses the change. Owner-scoped.
pub fn emit_bulk_changed(user_id: uuid::Uuid, action: SyncAction, origin: Option<uuid::Uuid>) {
    sync_publish(
        SyncEntity::Notification,
        action,
        uuid::Uuid::nil(),
        Audience::owner(user_id),
        origin,
    );
}
