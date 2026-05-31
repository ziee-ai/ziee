//! Route registration for the remote_access module.
//!
//! All routes are mounted with the localhost-Host middleware as
//! defense in depth (see middleware.rs). Even with the
//! `RequirePermissions<(RemoteAccessManage,)>` check, a phone with a
//! stolen admin token cannot disable the tunnel because the Host
//! header gives the tunneled request away.

use aide::axum::{
    ApiRouter,
    routing::{get_with, post_with, put_with},
};
use axum::middleware;

use super::handlers::*;
use super::middleware::require_localhost_host;

pub fn remote_access_router() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/api/remote-access/status",
            get_with(get_status, get_status_docs),
        )
        .api_route(
            "/api/remote-access/settings",
            get_with(get_settings, get_settings_docs)
                .put_with(update_settings, update_settings_docs),
        )
        .api_route(
            "/api/remote-access/tunnel/start",
            post_with(start_tunnel, start_tunnel_docs),
        )
        .api_route(
            "/api/remote-access/tunnel/stop",
            post_with(stop_tunnel, stop_tunnel_docs),
        )
        .api_route(
            "/api/remote-access/admin-password",
            post_with(set_admin_password, set_admin_password_docs),
        )
        .layer(middleware::from_fn(require_localhost_host))
}
