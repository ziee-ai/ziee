// Auth request/response type definitions

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::jwt::TokenPair;
use crate::modules::user::User;

// =====================================================
// Request Types
// =====================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    /// Optional provider name for LDAP/OAuth authentication
    /// If not specified, defaults to local password authentication
    pub provider: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OAuthAuthorizeQuery {
    /// Same-origin SPA path the user wants to land on after a
    /// successful login. Validated server-side as a same-origin path
    /// (no `//`, no absolute URLs) and stored in `oauth_sessions.return_to`
    /// — never forwarded through the provider URL.
    pub return_to: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OAuthCallbackQuery {
    /// Authorization code from provider
    pub code: String,
    /// State parameter for CSRF protection
    pub state: String,
}

/// Apple Sign In `form_post` body. Apple POSTs the callback when
/// scope includes `name` or `email` (mandatory per their HIG).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AppleCallbackForm {
    pub code: String,
    pub state: String,
    /// Apple sends the id_token in the form_post too, but we ignore
    /// it — the server-side flow exchanges `code` for a token via
    /// Apple's /auth/token endpoint and verifies that token against
    /// the JWKS. Field is captured for schema completeness.
    #[serde(default)]
    #[allow(dead_code)]
    pub id_token: Option<String>,
    /// First-auth-only JSON blob: `{"name":{"firstName":..,"lastName":..},"email":..}`.
    /// Apple sends this exactly ONCE, in the POST body, the very
    /// first time a user authorizes our app. We must persist it on
    /// arrival or lose access to the user's name forever.
    #[serde(default)]
    pub user: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LinkAccountRequest {
    /// Single-use token from /auth/link-account?token=...
    pub link_token: String,
    /// User's existing local password — the auth proof for binding
    /// the social identity.
    pub password: String,
}

// =====================================================
// Response Types
// =====================================================

#[derive(Debug, Serialize, JsonSchema)]
pub struct AuthResponse {
    pub user: User,
    #[serde(flatten)]
    pub tokens: TokenPair,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MeResponse {
    pub user: User,
    /// Effective permissions (union of user's direct permissions + all active group permissions)
    pub permissions: Vec<String>,
}

/// Public-safe summary of an enabled auth provider — what the login
/// page needs to render a provider button. NEVER includes client_id,
/// client_secret, or any other config.
#[derive(Debug, Serialize, JsonSchema)]
pub struct PublicProvider {
    pub name: String,
    pub provider_type: String,
    pub display_name: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PublicProvidersResponse {
    pub providers: Vec<PublicProvider>,
}

/// Admin-view summary of an auth provider. The `config` JSONB is
/// returned with `client_secret` and any sensitive key contents
/// MASKED — see handlers.rs::mask_provider_config.
#[derive(Debug, Serialize, JsonSchema)]
pub struct AuthProviderResponse {
    pub id: uuid::Uuid,
    pub name: String,
    pub provider_type: String,
    pub enabled: bool,
    pub config: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// When the admin last clicked Test on this row (null = never).
    pub last_test_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Outcome of the last test (null = never tested).
    pub last_test_ok: Option<bool>,
    /// Human-readable detail from the last test.
    pub last_test_message: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateAuthProviderRequest {
    pub name: String,
    /// One of: `oidc`, `oauth2`, `apple`, `ldap`, `local`.
    pub provider_type: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub config: serde_json::Value,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateAuthProviderRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Patch the provider's JSONB config. Empty `client_secret`
    /// field is treated as "leave the existing value unchanged" so
    /// admins can edit other fields without re-entering secrets.
    #[serde(default)]
    pub config: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TestProviderResponse {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DeleteProviderResponse {
    pub deleted: bool,
    pub affected_user_links: i64,
}

fn default_true() -> bool {
    true
}

// =====================================================
// Change password
// =====================================================
//
// NOTE: `AuthConfigResponse` and `PasswordOnlyLoginRequest` (which
// used to live here) moved with their handlers into the desktop
// tauri crate (`desktop/tauri/src/modules/tunnel_auth/`) — they
// depend on the desktop-only `remote_access_settings` table.

// NOTE: `ChangePasswordRequest` moved to the desktop tauri crate
// (`desktop/tauri/src/modules/tunnel_auth/models.rs`) alongside its
// only handler (`change_password`).
