//! Durable, owner-scoped notification inbox — ziee's THIN consumer of the
//! `ziee-notification` SDK crate.
//!
//! Greenfield inbox where background results land ("your literature sweep found
//! 12 new papers"). New rows push live via `SyncEntity::Notification`
//! (`Audience::owner`, origin=None). The scheduler is the first producer via the
//! crate's `create_and_emit` seam, but the module is general.
//!
//! Chunk `notification` (R2) moved the ENGINE into `ziee-notification` with the
//! `routes` feature: the DB-free `models` + `permissions` key, the crate's own
//! (domain-agnostic, `payload jsonb`) migration, the schema-bound `repository`
//! (`query_as!`), the `events` producer + pluggable `set_sync_emitter` sync
//! seam, and the resolver-generic aide/axum `notification_router::<R>()`. ziee
//! keeps ONLY this thin `AppModule`: it registers ONE module entry, mounts
//! `notification_router::<ZieeIdentityResolver>()` (NOT the crate's turnkey
//! `module` feature, which would double-register), installs the sync emitter
//! that maps the crate's `NotifSyncAction` → ziee's `SyncAction`
//! (`SyncEntity::Notification`, `Audience::owner`), and spawns the retention
//! prune (ziee owns the caller because the window comes from
//! `scheduler_admin_settings`). Mirrors ziee-auth / ziee-file.

use std::error::Error;
use std::sync::Arc;

use aide::axum::ApiRouter;
use linkme::distributed_slice;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};
use crate::modules::permissions::extractors::ZieeIdentityResolver;
use crate::modules::sync::{Audience, SyncAction, SyncEntity, publish as sync_publish};

use ziee_notification::NotifSyncAction;

// The retention-prune caller stays ziee-side: the window is read from
// `scheduler_admin_settings` (an app-specific settings row the SDK crate knows
// nothing about). It drives the crate's `repository::prune_older_than`.
pub mod prune;

// Re-export the moved DB-free cores as equivalence-preserving shims so existing
// `crate::modules::notification::{models, permissions}` paths keep resolving.
#[allow(unused_imports)]
pub use ziee_notification::{models, permissions};

/// Map the SDK crate's transport-neutral `NotifSyncAction` onto ziee's concrete
/// `SyncAction` (the crate never names ziee's sync vocabulary).
fn map_sync_action(action: NotifSyncAction) -> SyncAction {
    match action {
        NotifSyncAction::Create => SyncAction::Create,
        NotifSyncAction::Update => SyncAction::Update,
        NotifSyncAction::Delete => SyncAction::Delete,
    }
}

#[distributed_slice(MODULE_ENTRIES)]
static NOTIFICATION_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "notification",
    // After the tables it references exist (migrations run at build); no init
    // ordering dependency on other modules.
    order: 89,
    description: "Durable notification inbox",
    constructor: || Box::new(NotificationModule),
};

pub struct NotificationModule;

impl AppModule for NotificationModule {
    fn name(&self) -> &'static str {
        "notification"
    }

    fn description(&self) -> &'static str {
        "Durable notification inbox"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        // Install the app's sync emitter once at boot: every inbox change the
        // crate produces (create / mark-read / delete / mark-all-read) maps onto
        // ziee's owner-scoped `SyncEntity::Notification` frame so all of the
        // owner's devices refetch. `set_sync_emitter` is first-registration-wins.
        ziee_notification::set_sync_emitter(Arc::new(
            |user_id, action, id, origin| {
                sync_publish(
                    SyncEntity::Notification,
                    map_sync_action(action),
                    id,
                    Audience::owner(user_id),
                    origin,
                );
            },
        ));

        // Periodic retention prune (reads scheduler_admin_settings each tick;
        // 0 = keep forever). Fire-and-forget, like the mcp tool-call prune.
        let pool = (*ctx.db_pool).clone();
        tokio::spawn(async move { prune::run_prune_loop(pool).await });
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        // The resolver-generic inbox bundle, mounted with ziee's resolver.
        router.merge(ziee_notification::notification_router::<ZieeIdentityResolver>())
    }
}
