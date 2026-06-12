//! Server self-update notification module.
//!
//! NOTIFICATION ONLY: a daily background task polls GitHub's `releases/latest`
//! for `phibya/ziee-chat-new`, compares semver against the running version, and
//! caches the result. An admin endpoint (`GET /api/server-update/status`)
//! exposes it to the web UI (banner + System/About page). It never downloads or
//! installs — operators update manually via `install.sh`.
//!
//! Disabled when `update_check.enabled` is false (air-gapped) and, critically,
//! in the **embedded desktop server** (the desktop app has its own
//! `tauri-plugin-updater`) — the desktop forces the flag off in
//! `desktop/tauri/src/modules/backend/mod.rs`.

mod checker;
mod handlers;
pub mod permissions;
mod routes;
mod types;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use std::error::Error;
use std::time::Duration;

use crate::module_api::{AppModule, MODULE_ENTRIES, ModuleContext, ModuleEntry};

/// One day between background checks.
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

#[distributed_slice(MODULE_ENTRIES)]
static SERVER_UPDATE_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "server_update",
    order: 92,
    description: "Server self-update notification (daily GitHub version check)",
    constructor: || Box::new(ServerUpdateModule::new()),
};

#[derive(Default)]
pub struct ServerUpdateModule;

impl ServerUpdateModule {
    pub fn new() -> Self {
        Self
    }
}

impl AppModule for ServerUpdateModule {
    fn name(&self) -> &'static str {
        "server_update"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn description(&self) -> &'static str {
        "Server self-update notification"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        let enabled = ctx.config.update_check.enabled;
        checker::set_enabled(enabled);
        if !enabled {
            tracing::info!(
                "server_update: update checks disabled in config; no polling, no outbound calls"
            );
            return Ok(());
        }
        // Daily poll: one immediate check on boot, then every 24h. Spawns inside
        // the server's tokio runtime (same as code_sandbox's background tasks).
        tokio::spawn(async move {
            loop {
                checker::check_once().await;
                tokio::time::sleep(CHECK_INTERVAL).await;
            }
        });
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        // Owns its own `/server-update` prefix (two modules nesting the same
        // prefix would panic at merge time).
        router.merge(ApiRouter::new().nest("/server-update", routes::routes()))
    }
}
