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
use std::sync::Arc;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use once_cell::sync::OnceCell;

use crate::core::config::Config;
use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

/// The deployment `Config`, stashed at init so the run-now/test-fire handlers
/// (plain axum handlers with no config in scope) can build the chat extension
/// registry for a prompt-target firing. Set once in `init`.
static SCHEDULER_CONFIG: OnceCell<Arc<Config>> = OnceCell::new();

/// The stashed config, if the module has initialized.
pub fn scheduler_config() -> Option<Arc<Config>> {
    SCHEDULER_CONFIG.get().cloned()
}

pub mod change;
pub mod dispatch;
pub mod dryrun;
pub mod events;
pub mod failure;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod schedule;
pub mod settings;
pub mod tick;

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

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        // Boot-spawned tick loop: claims due tasks, dispatches, advances
        // next_run_at (coalesced catch-up on the first tick). Fire-and-forget,
        // like the memory reaper. Runs only while the process is up (on desktop:
        // while the app is open — DEC-8).
        let _ = SCHEDULER_CONFIG.set(ctx.config.clone());
        let pool = (*ctx.db_pool).clone();
        let config = ctx.config.clone();
        tokio::spawn(async move { tick::run_tick_loop(pool, config).await });
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::scheduler_router())
    }
}
