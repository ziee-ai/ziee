//! Realtime sync emitters for the scheduler. Task mutations are owner-scoped;
//! admin-settings changes go to holders of the admin read-perm.

use uuid::Uuid;

use crate::modules::sync::{Audience, SyncAction, SyncEntity, publish as sync_publish};

use super::permissions::SchedulerAdminRead;

/// A scheduled task changed (create/update/enable/pause/delete) or a firing
/// advanced its state. Owner-scoped; `origin` is `None` from the tick loop.
pub fn emit_task(action: SyncAction, id: Uuid, owner: Uuid, origin: Option<Uuid>) {
    sync_publish(
        SyncEntity::ScheduledTask,
        action,
        id,
        Audience::owner(owner),
        origin,
    );
}

/// The scheduler admin-settings singleton changed. Delivered to holders of
/// `scheduler::admin::read`. `id` is nil (a singleton addressed by perm).
pub fn emit_admin_settings(origin: Option<Uuid>) {
    sync_publish(
        SyncEntity::SchedulerAdminSettings,
        SyncAction::Update,
        Uuid::nil(),
        Audience::perm::<SchedulerAdminRead>(),
        origin,
    );
}
