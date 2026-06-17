//! Host-folder mount module — mount a folder from the user's machine into the
//! code sandbox (feature #3, Part B).
//!
//! Lives in the **desktop tauri crate** (not the server crate) because the
//! whole feature is desktop-only: host mounting is only possible where the
//! sandbox runs on the user's machine. The server core stays generic — it
//! exposes a `SandboxMountProvider` seam (`ziee::code_sandbox`), and this
//! module registers a provider against it at boot. A standalone/remote-web
//! server links none of this and the sandbox behaves exactly as before.
//!
//! Surfaces:
//!   - REST CRUD at `/api/host-mounts/*` (policy + per-conversation +
//!     per-project mount lists), gated by `host_mount::{read,manage}`.
//!   - A `SandboxMountProvider` (`provider::DesktopHostMountProvider`) that
//!     resolves the effective mounts (read-through conversation→project),
//!     enforces the policy, and maps each folder to `/mnt/<full host path>`.
//!
//! Storage: `host_mount_policy` (singleton) + `host_mounts` (per-scope),
//! created by desktop migration 10000000000005. The server crate's `build.rs`
//! walks `desktop/tauri/migrations/` so the build DB has the schema and the
//! `sqlx::query!()` macros validate at compile time.

pub mod handlers;
pub mod models;
pub mod paths;
pub mod permissions;
pub mod provider;
pub mod repository;
pub mod routes;

use std::sync::Arc;

use anyhow::Result;
use tauri::App;
use ziee::ApiRouter;

use crate::module_api::DesktopModule;
use provider::DesktopHostMountProvider;

pub struct HostMountModule;

impl HostMountModule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HostMountModule {
    fn default() -> Self {
        Self::new()
    }
}

impl DesktopModule for HostMountModule {
    fn name(&self) -> &'static str {
        "host_mount"
    }

    fn description(&self) -> &'static str {
        "Mount host folders into the code sandbox (desktop-only)."
    }

    fn init(&mut self, _app: &mut App) -> Result<()> {
        // No app-level state. The sandbox provider is registered separately
        // (see `register_provider`) once the server pool exists.
        Ok(())
    }

    fn register_api_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::host_mount_router())
    }
}

/// Register the desktop host-mount provider against the generic server seam.
///
/// MUST be called AFTER the embedded server has initialized `ziee::Repos`
/// (i.e. from inside / after `start_server_with_routes`), not from
/// `DesktopModule::init` which runs before the pool exists. Idempotent enough
/// for boot — call exactly once.
pub fn register_provider() {
    let pool = ziee::Repos.pool().clone();
    ziee::code_sandbox::register_sandbox_mount_provider(Arc::new(DesktopHostMountProvider::new(
        pool,
    )));
    tracing::info!("host_mount: registered sandbox mount provider");
}
