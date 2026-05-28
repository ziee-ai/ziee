//! Remote Access module — expose the local desktop server over an
//! ngrok tunnel.
//!
//! Lives in the **desktop tauri crate** (not the server crate) because
//! the whole feature is desktop-only:
//!   - The UI to configure the tunnel ships ONLY in the desktop bundle.
//!   - The `ngrok` dep is desktop-tauri-only — server-only deployments
//!     never link it in.
//!   - The schema (`remote_access_settings`, `magic_link_tokens`) lives
//!     in `desktop/tauri/migrations/`. The server crate's `build.rs`
//!     also walks that dir so the build DB has the schema and
//!     `sqlx::query_as!()` macros validate at compile time.
//!
//! Surfaces:
//!   - REST CRUD at `/api/remote-access/*` (this module). All routes
//!     are gated by `RequirePermissions<(RemoteAccessManage,)>` AND
//!     the localhost-Host middleware (defense in depth).
//!   - Magic-link issue/exchange endpoints live in the sibling
//!     `crate::modules::magic_link` module.
//!
//! Storage: singleton `remote_access_settings` (id=1) created by
//! desktop migration 10000000000003. Auth token encrypted at rest via
//! pgcrypto (`ziee::encrypt_secret` / `ziee::decrypt_secret`).
//!
//! Tunnel lifecycle: `TunnelDriver` trait — `NgrokDriver` (prod)
//! wraps the `ngrok` crate, `MockTunnelDriver` (test) returns canned
//! URLs without any network. The active driver lives in
//! `state::tunnel_driver()` (an `OnceLock`).

pub mod auto_start;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod state;
pub mod tunnel;

pub use auto_start::auto_start_if_configured;
pub use state::{init_tunnel_driver, set_local_server_port, tunnel_driver};

use anyhow::Result;
use tauri::App;
use ziee::ApiRouter;

use crate::module_api::DesktopModule;

pub struct RemoteAccessModule;

impl RemoteAccessModule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RemoteAccessModule {
    fn default() -> Self {
        Self::new()
    }
}

impl DesktopModule for RemoteAccessModule {
    fn name(&self) -> &'static str {
        "remote_access"
    }

    fn description(&self) -> &'static str {
        "Remote Access — expose the local server over an ngrok tunnel"
    }

    fn init(&mut self, _app: &mut App) -> Result<()> {
        // Explicitly install the prod tunnel driver. tunnel_driver()
        // would lazy-init to NgrokDriver anyway via OnceLock's
        // get_or_init, but doing it here gives a clear startup log
        // line and lets the choice be auditable (rather than
        // depending on the env-var-fallback path).
        state::init_tunnel_driver(std::sync::Arc::new(tunnel::NgrokDriver::new()));

        // Auto-start kicks off in a background task so it doesn't
        // block module init even if ngrok is slow. Uses
        // `tauri::async_runtime::spawn` (not `tokio::spawn`) because
        // the desktop module's `init()` runs in the Tauri setup
        // callback, which is synchronous — calling `tokio::spawn`
        // directly panics with "no reactor running". Tauri's
        // async_runtime is the same tokio runtime the embedded
        // server uses; it's safe to spawn long-running tasks here.
        //
        // The repository factory + storage_key + DB pool are
        // initialized by BackendModule's spawned server task —
        // hence the small delay so the HTTP listener (and Repos
        // global) is definitely up by the time we try to read the
        // settings + forward traffic.
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            auto_start::auto_start_if_configured().await;
        });
        Ok(())
    }

    fn register_api_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::remote_access_router())
    }
}
