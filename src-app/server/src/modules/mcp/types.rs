// MCP types

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::models::{McpServer, SetMcpServerOAuthConfigRequest, TransportType, UsageMode};

/// Inbound shape for ONE env-var entry on create/update. Mirrors
/// `EnvVarView` (response) but with a different `value` semantic:
///
/// * `value: Some(s)` — set/overwrite this entry's value to `s`.
///   For secret entries (`is_secret: true`), the new value is
///   encrypted into `environment_variables_encrypted`. For non-secret,
///   it goes into the plain `environment_variables` map.
/// * `value: None` — KEEP existing. Used by the UI when the user
///   didn't touch a saved secret (the form shows `••••• (saved)` and
///   we don't want to clobber it with a blank).
/// * `value: Some("")` — explicit empty string. Stored verbatim.
///
/// Toggling `is_secret` across saves migrates the entry between the
/// plain and encrypted columns; the repo does that move atomically.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct EnvVarEntry {
    pub key: String,
    #[serde(default)]
    pub value: Option<String>,
    pub is_secret: bool,
}

/// HTTP-header analog of `EnvVarEntry`. Identical shape; separate
/// type so the OpenAPI surface (and form-state types on the FE) stay
/// unambiguous between the two editor sections.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct HeaderEntry {
    pub key: String,
    #[serde(default)]
    pub value: Option<String>,
    pub is_secret: bool,
}

// =====================================================
// Request Types
// =====================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateMcpServerRequest {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub transport_type: TransportType,

    // stdio transport
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    /// Structured env-var entries (replaces the old flat
    /// `HashMap<String, String>` shape). Each entry's `is_secret`
    /// flag decides whether the value gets encrypted at rest. None /
    /// missing → no env vars.
    pub environment_variables_entries: Option<Vec<EnvVarEntry>>,

    // http/sse transport
    pub url: Option<String>,
    /// Structured HTTP header entries. Same per-entry secret model
    /// as `environment_variables_entries`.
    pub headers_entries: Option<Vec<HeaderEntry>>,

    // Runtime configuration
    pub timeout_seconds: Option<i32>,

    // Sampling configuration
    pub supports_sampling: Option<bool>,
    pub usage_mode: Option<UsageMode>,
    pub max_concurrent_sessions: Option<i32>,

    /// Launch the stdio subprocess inside the code_sandbox bwrap
    /// isolation. The user-create handler force-sets this to `true`
    /// for user-owned stdio servers per the active MCP user policy
    /// (any client value is ignored). Admins may set it freely on
    /// system stdio servers via the drawer toggle.
    pub run_in_sandbox: Option<bool>,

    /// Rootfs flavor (KNOWN_FLAVORS, e.g. "minimal" / "full") for the
    /// sandboxed launch. None → handler-picks default ('full' on
    /// fresh rows; the user-create handler force-overrides this with
    /// the active `mcp_user_policy.user_stdio_sandbox_flavor` for
    /// user-owned stdio regardless of what the client sent).
    pub sandbox_flavor: Option<String>,

    /// Optional Hub identifier — when set, the create handler also
    /// records the install in `hub_entities` so the Hub card's
    /// "already installed" badge keeps working. Set by the UI when
    /// the drawer was opened via "Install" / "Install for the system"
    /// on a hub MCP card; null for direct-add. Type matches the
    /// existing `CreateMcpServerFromHubRequest::hub_id` (catalog
    /// slug string, not a UUID).
    pub hub_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateMcpServerRequest {
    pub name: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,

    // stdio transport
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    /// Replaces the existing env vars wholesale when present.
    /// `None` means "don't touch" (caller didn't include this field).
    /// `Some(empty vec)` clears all entries. Per-entry `value: None`
    /// keeps the existing secret value (see `EnvVarEntry`).
    pub environment_variables_entries: Option<Vec<EnvVarEntry>>,

    // http/sse transport
    pub url: Option<String>,
    /// Same wholesale-replace semantic as `environment_variables_entries`.
    pub headers_entries: Option<Vec<HeaderEntry>>,

    // Runtime configuration
    pub timeout_seconds: Option<i32>,

    // Sampling configuration
    pub supports_sampling: Option<bool>,
    pub usage_mode: Option<UsageMode>,
    pub max_concurrent_sessions: Option<i32>,

    /// Launch the stdio subprocess inside the code_sandbox bwrap
    /// isolation. Same force-set semantics as
    /// [`CreateMcpServerRequest::run_in_sandbox`].
    pub run_in_sandbox: Option<bool>,

    /// Rootfs flavor (KNOWN_FLAVORS, e.g. "minimal" / "full") for the
    /// sandboxed launch. Same force-override semantics as
    /// [`CreateMcpServerRequest::sandbox_flavor`].
    pub sandbox_flavor: Option<String>,
}

/// Request to test an MCP server connection without persisting anything.
///
/// Carries the same transport fields as a create/update request so the UI can
/// probe the *current form values* before saving. `oauth` is the credentials
/// typed into the form (new external server); since the client secret is
/// write-only in the edit / list flows, `id` lets the server fall back to the
/// stored OAuth config for that existing server.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TestMcpConnectionRequest {
    pub transport_type: TransportType,

    // stdio transport
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    /// Same structured shape as create/update — for entries the user
    /// hasn't touched (saved-secret entries with `value: None`), the
    /// test path falls back to the decrypted stored value via `id`
    /// (mirrors the existing OAuth-secret fallback comment below).
    pub environment_variables_entries: Option<Vec<EnvVarEntry>>,

    // http transport
    pub url: Option<String>,
    pub headers_entries: Option<Vec<HeaderEntry>>,

    // Runtime configuration
    pub timeout_seconds: Option<i32>,

    /// OAuth client_credentials typed into the form (new external HTTP server).
    pub oauth: Option<SetMcpServerOAuthConfigRequest>,

    /// Existing server id — used ONLY to recover the stored OAuth secret when
    /// `oauth` is absent (edit drawer / list card). Access-checked before use.
    pub id: Option<Uuid>,
}

// =====================================================
// Response Types
// =====================================================

/// Result of a connection test — `success` is the only authoritative field.
/// On failure `message` carries the underlying error (timeout / 401 / bad
/// command). On success `tool_count` is the number of tools the server
/// advertised (best-effort; `None` if the handshake succeeded but listing
/// tools failed).
#[derive(Debug, Serialize, JsonSchema)]
pub struct TestMcpConnectionResponse {
    pub success: bool,
    pub message: String,
    pub tool_count: Option<usize>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct McpServerListResponse {
    pub servers: Vec<McpServer>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub total_pages: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ServerGroupsRequest {
    pub group_ids: Vec<Uuid>,
}

/// Response for getting system MCP servers assigned to a group
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GroupSystemServersResponse {
    pub servers: Vec<McpServer>,
}

/// Request to update system MCP servers for a group
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateGroupSystemServersRequest {
    pub server_ids: Vec<Uuid>,
}
