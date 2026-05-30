//! Request/response DTOs and DB row structs for the remote_access module.
//!
//! The on-disk row stores the ngrok auth token encrypted (bytea via
//! pgcrypto); the API NEVER echoes the plaintext token. GET responses
//! report `auth_token_set: bool` instead. The domain IS echoed because
//! it becomes public the moment the tunnel starts.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// =====================================================
// DB row (private — never serialized to clients)
// =====================================================

/// Raw row from `remote_access_settings`. Plaintext-decoded token in
/// `ngrok_auth_token`; `created_at`/`updated_at` for audit. This struct
/// stays internal to the module — handlers map it to
/// `RemoteAccessSettingsResponse` which masks the token.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RemoteAccessSettingsRow {
    pub id: i16,
    pub ngrok_auth_token_enc: Option<Vec<u8>>,
    pub ngrok_domain: Option<String>,
    pub auto_start_tunnel: bool,
    pub password_auth_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// In-memory decoded view used by the tunnel layer.
#[derive(Debug, Clone)]
pub struct RemoteAccessSettings {
    pub ngrok_auth_token: Option<String>,
    pub ngrok_domain: Option<String>,
    pub auto_start_tunnel: bool,
    pub password_auth_enabled: bool,
}

// =====================================================
// HTTP responses
// =====================================================

/// GET /api/remote-access/settings response. Token is NEVER included
/// in the response body — only the `auth_token_set` boolean. The
/// domain IS included because it becomes public the moment the tunnel
/// connects.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RemoteAccessSettingsResponse {
    pub auth_token_set: bool,
    pub ngrok_domain: Option<String>,
    pub auto_start_tunnel: bool,
    pub password_auth_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// PUT /api/remote-access/settings request body. Each field uses
/// three-state semantics:
///   - missing key → don't touch (keep DB value)
///   - null → clear (set to NULL / FALSE)
///   - value → set
///
/// Implemented via `Option<Option<T>>` + a custom serde fn that
/// distinguishes "absent" from "null". The frontend matches by
/// only sending fields it actually wants to change.
#[derive(Debug, Clone, Deserialize, JsonSchema, Default)]
pub struct UpdateRemoteAccessSettingsRequest {
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub ngrok_auth_token: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub ngrok_domain: Option<Option<String>>,
    /// Booleans don't need null semantics — absent means "don't touch".
    pub auto_start_tunnel: Option<bool>,
    pub password_auth_enabled: Option<bool>,
}

/// Custom deserialize: distinguishes missing field (None) from
/// explicit null (Some(None)) from value (Some(Some(v))).
fn deserialize_nullable_field<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// GET /api/remote-access/status — combined status surface for the
/// admin page. Aggregates settings + live tunnel state + password
/// rotation status.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RemoteAccessStatusResponse {
    pub password_rotated: bool,
    pub password_auth_enabled: bool,
    pub auth_token_set: bool,
    pub ngrok_domain: Option<String>,
    pub auto_start_tunnel: bool,
    pub tunnel_state: TunnelStateKind,
    pub public_url: Option<String>,
    pub last_error: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TunnelStateKind {
    Idle,
    Starting,
    Connected,
    Error,
}

/// POST /api/remote-access/tunnel/start response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TunnelStartResponse {
    pub public_url: String,
    pub started_at: DateTime<Utc>,
}

/// POST /api/remote-access/admin-password request.
///
/// No `current_password` field — physical presence at the desktop
/// (proven by the localhost-Host middleware) IS the auth proof.
/// The standard `/api/users/me/password` flow that requires the
/// current password is still available for multi-user web deployments;
/// this endpoint exists specifically for the single-admin desktop
/// case where the bootstrap default is a published string and
/// requiring it would be friction without security benefit.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SetAdminPasswordRequest {
    pub new_password: String,
}
