//! Durable, owner-scoped notification inbox.
//!
//! Greenfield inbox where background results land ("your literature sweep found
//! 12 new papers"). Structurally mirrors `mcp/tool_calls/` (owner-scoped
//! history + retention prune). New rows push live via `SyncEntity::Notification`
//! (`Audience::owner`, origin=None). The scheduler is the first producer via the
//! `create_and_emit` seam, but the module is general.
//!
//! Module layout (built incrementally):
//!   permissions · models · repository · events (create_and_emit) · routes ·
//!   handlers · prune.

use std::error::Error;

use aide::axum::ApiRouter;
use linkme::distributed_slice;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

pub mod events;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod prune;
pub mod repository;
pub mod routes;

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
        // Periodic retention prune (reads scheduler_admin_settings each tick;
        // 0 = keep forever). Fire-and-forget, like the mcp tool-call prune.
        let pool = (*ctx.db_pool).clone();
        tokio::spawn(async move { prune::run_prune_loop(pool).await });
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::notification_router())
    }
}
