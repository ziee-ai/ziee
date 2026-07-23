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
pub mod continue_chat;
pub mod dispatch;
pub mod dryrun;
pub mod events;
pub mod failure;
pub mod goal_eval;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod proposal;
pub mod prune;
pub mod repository;
pub mod routes;
pub mod schedule;
pub mod settings;
pub mod tick;

// ── Group F inbox: declare the scheduler's notification kinds ────────────────
//
// The scheduler writes a durable typed notification on every task firing
// (`dispatch.rs::finalize_success` → kind `"scheduled_task_result"`;
// `finalize_failure` → kind `"scheduled_task_failed"`). Declaring those kinds in
// the SDK's per-module kind registry (`ziee_notification::registry`) makes
// `GET /api/notifications/kinds` advertise them, so the unified agent-inbox FE
// can build its scheduled-task filter + renderer from the registry (the same
// closure `background_mcp` did for `"background_run_result"`). Additive
// `#[distributed_slice]` — no OpenAPI change, no migration, data-only
// introspection. This registration ONLY declares the kinds; the producers still
// create rows directly via `create_and_emit` in `dispatch.rs` (untouched here).
#[distributed_slice(ziee_notification::registry::NOTIFICATION_KINDS)]
static SCHEDULED_TASK_RESULT_KIND: ziee_notification::registry::NotificationKindDescriptor =
    ziee_notification::registry::NotificationKindDescriptor {
        kind: "scheduled_task_result",
        description: "A scheduled / recurring task fired; its result is ready in the inbox.",
    };

#[distributed_slice(ziee_notification::registry::NOTIFICATION_KINDS)]
static SCHEDULED_TASK_FAILED_KIND: ziee_notification::registry::NotificationKindDescriptor =
    ziee_notification::registry::NotificationKindDescriptor {
        kind: "scheduled_task_failed",
        description: "A scheduled / recurring task failed to run; it needs attention.",
    };

#[cfg(test)]
mod kind_registration_tests {
    // The scheduler's two firing-outcome notification kinds must be advertised by
    // `GET /api/notifications/kinds` (via the `#[distributed_slice]` registry)
    // so the agent-inbox filter is complete (Group F). Asserts the slice collects
    // both scheduler kinds AND carries a description (the wire contract).
    use ziee_notification::registry::{is_registered_kind, registered_kinds};

    #[test]
    fn scheduler_notification_kinds_are_registered() {
        assert!(
            is_registered_kind("scheduled_task_result"),
            "scheduled_task_result must be advertised by /api/notifications/kinds"
        );
        assert!(
            is_registered_kind("scheduled_task_failed"),
            "scheduled_task_failed must be advertised by /api/notifications/kinds"
        );
        let kinds = registered_kinds();
        for kind in ["scheduled_task_result", "scheduled_task_failed"] {
            let d = kinds
                .iter()
                .find(|d| d.kind == kind)
                .unwrap_or_else(|| panic!("{kind} descriptor present"));
            assert!(!d.description.is_empty(), "{kind} carries a description");
        }
    }
}

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
        let _ = SCHEDULER_CONFIG.set(crate::module_api::app_config(ctx));
        let pool = (*ctx.db_pool).clone();
        let config = crate::module_api::app_config(ctx);
        tokio::spawn(async move { tick::run_tick_loop(pool, config).await });
        // Boot-spawned run-history retention prune (ITEM-8/DEC-7): reuses the
        // admin `notification_retention_days` window; 0 = keep forever.
        let prune_pool = (*ctx.db_pool).clone();
        tokio::spawn(async move { prune::run_prune_loop(prune_pool).await });
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::scheduler_router())
    }
}
