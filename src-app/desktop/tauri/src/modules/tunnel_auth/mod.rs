//! Tunnel-aware auth endpoints — `GET /api/auth/config` and
//! `POST /api/auth/login-password-only`.
//!
//! Both depend on the desktop-only `remote_access_settings` row to
//! decide their behavior, so they live in the desktop crate.
//! Server-only deployments don't need them (they have their own
//! generic auth surface in `crate::auth` over in the server).
//!
//! `change_password` (POST /api/users/me/password) is generic and
//! stays in the server crate's auth module.

pub mod handlers;
pub mod models;
pub mod routes;

pub use routes::tunnel_auth_routes;

use anyhow::Result;
use tauri::App;
use ziee::ApiRouter;

use crate::module_api::DesktopModule;

pub struct TunnelAuthModule;

impl TunnelAuthModule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TunnelAuthModule {
    fn default() -> Self {
        Self::new()
    }
}

impl DesktopModule for TunnelAuthModule {
    fn name(&self) -> &'static str {
        "tunnel_auth"
    }

    fn description(&self) -> &'static str {
        "Tunnel-aware auth endpoints: /auth/config + /auth/login-password-only"
    }

    fn init(&mut self, _app: &mut App) -> Result<()> {
        Ok(())
    }

    fn register_api_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::tunnel_auth_routes())
    }
}
