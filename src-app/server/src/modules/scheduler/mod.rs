//! Scheduled / recurring background tasks.
//!
//! A user saves a *task* that fires on a schedule (once at time T, or recurring
//! via cron) and runs an existing execution seam with no browser attached — a
//! saved workflow (`runner::spawn_run`, `invocation_source='scheduled'`) or an
//! assistant + prompt turn (chat pipeline, appended to a per-task bound
//! conversation). A boot-spawned tick loop claims due tasks
//! (`FOR UPDATE SKIP LOCKED`, mirroring `memory/reaper.rs`), dispatches, and
//! advances `next_run_at`; downtime is handled by coalesced catch-up. Results
//! land in the `notification` inbox; realtime via `SyncEntity::ScheduledTask`.
//!
//! Module layout (built incrementally):
//!   permissions · schedule (pure cron/tz engine) · models · repository ·
//!   settings (admin singleton) · tick · dispatch · failure · change · dryrun ·
//!   routes · handlers · events.

use std::error::Error;

use aide::axum::ApiRouter;
use linkme::distributed_slice;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

pub mod models;
pub mod permissions;
pub mod repository;
pub mod schedule;
pub mod settings;

#[distributed_slice(MODULE_ENTRIES)]
static SCHEDULER_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "scheduler",
    // After workflow (82) + chat so both execution seams exist; the tick loop
    // depends on the notification module's create seam (order-independent —
    // it's called at runtime, not init).
    order: 90,
    description: "Scheduled / recurring background tasks",
    constructor: || Box::new(SchedulerModule),
};

pub struct SchedulerModule;

impl AppModule for SchedulerModule {
    fn name(&self) -> &'static str {
        "scheduler"
    }

    fn description(&self) -> &'static str {
        "Scheduled / recurring background tasks"
    }

    fn init(&mut self, _ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        // The tick loop is spawned here once tick.rs lands.
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        // REST routes are merged here once routes.rs lands.
        router
    }
}
