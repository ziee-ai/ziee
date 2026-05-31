//! Request/response DTOs for the tunnel-aware auth endpoints.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Public, unauthenticated config that drives how the login page
/// renders for a given request. Returned by `GET /api/auth/config`.
///
/// Computed from the `remote_access_settings` singleton + the
/// inbound request's `Host` header. Tunneled requests (Host is not
/// localhost) get `hide_username: true` and only see the password
/// form when `password_auth_enabled: true`. Localhost requests get
/// the full multi-user UI behavior the web bundle would normally use.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AuthConfigResponse {
    pub password_auth_enabled: bool,
    pub magic_link_enabled: bool,
    pub hide_username: bool,
}

/// Body of `POST /api/auth/login-password-only`. Authenticates as the
/// single admin user using just a password (no username). Used by
/// the remote-served login page when `hide_username: true`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PasswordOnlyLoginRequest {
    pub password: String,
}

/// Body of `POST /api/users/me/password`. Current + new password.
/// The endpoint sets `users.password_changed_at` so the Remote
/// Access module can allow enabling password authentication.
///
/// This handler lives in the desktop crate because its sole consumer
/// is the Remote Access flow — only the desktop installs the
/// migration that adds `password_changed_at`, and only the desktop
/// gates password-auth toggling on it.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}
