// Auth routes configuration

use aide::axum::{
    ApiRouter,
    routing::{get_with, post_with, put_with},
};
use axum::routing::{get, post};

use super::handlers::*;

/// Public + user auth routes — mounted at `/auth`.
///
/// NOTE: `GET /auth/config`, `POST /auth/login-password-only`, and
/// the magic-link `issue` + `exchange` endpoints all live in the
/// **desktop tauri crate** (see
/// `desktop/tauri/src/modules/{remote_access,magic_link}/`). They
/// depend on the `remote_access_settings` and `magic_link_tokens`
/// tables that only the desktop binary migrates, so housing them
/// outside the server crate keeps server-only deployments lean
/// (no ngrok dep, no orphaned admin-only routes).
pub fn auth_routes() -> ApiRouter {
    ApiRouter::new()
        .api_route("/register", post_with(register, register_docs))
        .api_route("/login", post_with(login, login_docs))
        .api_route("/refresh", post_with(refresh, refresh_docs))
        .api_route("/logout", post_with(logout, logout_docs))
        .api_route("/me", get_with(me, me_docs))
        // Self-service profile edit + password change for the current
        // user. Gated on `profile::edit` (held by the default group),
        // scoped to the caller. Siblings of /me.
        .api_route("/profile", post_with(update_profile, update_profile_docs))
        .api_route("/password", post_with(change_password, change_password_docs))
        // Public list of enabled providers for the login page.
        .api_route(
            "/providers",
            get_with(list_public_providers, list_public_providers_docs),
        )
        // First-Broker-Login confirmation: user proves ownership of
        // an existing local account by re-entering its password.
        .api_route("/link-account", post_with(link_account, link_account_docs))
        // OAuth redirects return raw HTTP redirects (not JSON), so they
        // skip aide. Apple uses POST (form_post); everything else uses GET.
        .route("/oauth/{provider_name}/authorize", get(oauth_authorize))
        .route(
            "/oauth/{provider_name}/callback",
            get(oauth_callback).post(oauth_callback_post),
        )
}

/// Admin auth-provider CRUD. Uses full paths (no nest) to match the
/// project's convention (see modules/user/routes.rs). Every handler
/// is gated through `RequirePermissions<...>` so this router carries
/// no additional middleware.
pub fn auth_admin_routes() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/admin/auth-providers",
            get_with(admin_list_providers, admin_list_providers_docs)
                .post_with(admin_create_provider, admin_create_provider_docs),
        )
        .api_route(
            "/admin/auth-providers/{id}",
            put_with(admin_update_provider, admin_update_provider_docs)
                .delete_with(admin_delete_provider, admin_delete_provider_docs),
        )
        .api_route(
            "/admin/auth-providers/{id}/test",
            post_with(admin_test_provider, admin_test_provider_docs),
        )
        .api_route(
            "/admin/auth-providers/test-config",
            post_with(admin_test_provider_config, admin_test_provider_config_docs),
        )
}
