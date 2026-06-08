use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::{Decode, Encode, Postgres, Type};
use uuid::Uuid;

// =====================================================
// Enums
// =====================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransportType {
    Stdio,
    Http,
    Sse,
}

impl<'r> Decode<'r, Postgres> for TransportType {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as Decode<Postgres>>::decode(value)?;
        match s {
            "stdio" => Ok(TransportType::Stdio),
            "http" => Ok(TransportType::Http),
            "sse" => Ok(TransportType::Sse),
            _ => Err(format!("Unknown transport type: {}", s).into()),
        }
    }
}

impl<'q> Encode<'q, Postgres> for TransportType {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        let s = match self {
            TransportType::Stdio => "stdio",
            TransportType::Http => "http",
            TransportType::Sse => "sse",
        };
        <&str as Encode<Postgres>>::encode_by_ref(&s, buf)
    }
}

impl Type<Postgres> for TransportType {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <&str as Type<Postgres>>::type_info()
    }
}

impl TransportType {
    pub fn from_str(s: &str) -> Result<Self, crate::common::AppError> {
        match s {
            "stdio" => Ok(TransportType::Stdio),
            "http" => Ok(TransportType::Http),
            "sse" => Ok(TransportType::Sse),
            _ => Err(crate::common::AppError::internal_error(format!(
                "Unknown transport type: {}",
                s
            ))),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            TransportType::Stdio => "stdio".to_string(),
            TransportType::Http => "http".to_string(),
            TransportType::Sse => "sse".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UsageMode {
    Auto,
    Always,
}

impl<'r> Decode<'r, Postgres> for UsageMode {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as Decode<Postgres>>::decode(value)?;
        match s {
            "auto" => Ok(UsageMode::Auto),
            "always" => Ok(UsageMode::Always),
            _ => Err(format!("Unknown usage mode: {}", s).into()),
        }
    }
}

impl<'q> Encode<'q, Postgres> for UsageMode {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        let s = match self {
            UsageMode::Auto => "auto",
            UsageMode::Always => "always",
        };
        <&str as Encode<Postgres>>::encode_by_ref(&s, buf)
    }
}

impl Type<Postgres> for UsageMode {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <&str as Type<Postgres>>::type_info()
    }
}

impl UsageMode {
    pub fn from_str(s: &str) -> Result<Self, crate::common::AppError> {
        match s {
            "auto" => Ok(UsageMode::Auto),
            "always" => Ok(UsageMode::Always),
            _ => Err(crate::common::AppError::internal_error(format!(
                "Unknown usage mode: {}",
                s
            ))),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            UsageMode::Auto => "auto".to_string(),
            UsageMode::Always => "always".to_string(),
        }
    }
}

// =====================================================
// Database Models
// =====================================================

/// A single env var or HTTP header entry on a MCP server, as
/// exposed in API RESPONSES. The wire shape replaces the old
/// flat `{KEY: "value"}` map so the UI knows per-entry which
/// keys are secrets without seeing their values.
///
/// `value` is `None` when the entry is a secret (write-only — see
/// `crate::common::secret` for the pattern) or when the entry is
/// genuinely empty. Non-secret entries always include the plain
/// value. The UI renders `Some` as a text input pre-filled with the
/// value; `None` + `is_secret: true` as an `Input.Password` with
/// `••••• (saved)` placeholder.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EnvVarView {
    pub key: String,
    pub value: Option<String>,
    pub is_secret: bool,
}

/// HTTP-header analog of `EnvVarView`. Same shape; separate type so
/// the OpenAPI surface is unambiguous (env vs header inputs render
/// in different drawer sections, with different default
/// `is_secret` defaults at the UI layer). `Deserialize` is implemented
/// only because `McpServer` carries this in a `Vec<HeaderView>` and
/// `McpServer` derives `Deserialize` for sqlx + test plumbing — no
/// inbound API path takes `HeaderView` (use `HeaderEntry` in
/// `types.rs` for that).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HeaderView {
    pub key: String,
    pub value: Option<String>,
    pub is_secret: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpServer {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub is_system: bool,
    pub is_built_in: bool,
    pub transport_type: TransportType,

    // stdio transport
    pub command: Option<String>,
    pub args: serde_json::Value,
    /// FLAT decrypted env-var map. Always populated by the repo with
    /// the FULLY decrypted values (plain entries merged with the
    /// decrypted secrets) so internal runtime callsites — stdio
    /// subprocess spawn, http header `${VAR}` interpolation — keep
    /// working unchanged. NEVER serialized — the public response
    /// shape is `environment_variables_entries` with secret values
    /// redacted to `value: None`.
    #[serde(default, skip_serializing)]
    pub environment_variables: serde_json::Value,

    // http/sse transport
    pub url: Option<String>,
    /// FLAT decrypted header map. Same write-only semantics as
    /// `environment_variables` — see that field's doc comment.
    /// `parse_header_map` runs `${VAR}` interpolation against the
    /// (decrypted) env map at request-build time.
    #[serde(default, skip_serializing)]
    pub headers: serde_json::Value,

    /// Public structured view of `environment_variables` for the UI
    /// form editor. Per-entry secret marker; secret values redacted
    /// to `None`. Populated by the repo from the triple-column
    /// storage (`environment_variables` + `environment_variables_encrypted` +
    /// `environment_variables_secret_keys`).
    #[serde(default)]
    pub environment_variables_entries: Vec<EnvVarView>,

    /// Public structured view of `headers` for the UI form editor.
    /// Sibling of `environment_variables_entries`.
    #[serde(default)]
    pub headers_entries: Vec<HeaderView>,

    // Runtime configuration
    pub timeout_seconds: i32,

    // Sampling configuration
    pub supports_sampling: bool,
    pub usage_mode: UsageMode,
    pub max_concurrent_sessions: Option<i32>,

    /// Launch the stdio subprocess inside the code_sandbox bwrap
    /// isolation. Honored for any `transport_type == Stdio` row (both
    /// system and user-owned) — the user-create handler force-sets this
    /// to `true` for user-owned stdio servers per the active MCP user
    /// policy; admins choose freely on system servers via the drawer
    /// toggle. The spawn path ignores it for non-stdio servers.
    pub run_in_sandbox: bool,

    /// Rootfs flavor (KNOWN_FLAVORS, e.g. `minimal`/`full`) used when
    /// `run_in_sandbox` launches this stdio server inside the
    /// code_sandbox. Defaults to `full` (the flavor that ships Node +
    /// uv + python3 + R). Ignored when not sandboxed. For user-owned
    /// stdio rows the user-create handler force-overwrites this with
    /// the active `mcp_user_policy.user_stdio_sandbox_flavor` (the
    /// drawer hides the picker on the user side); admins choose it on
    /// the system drawer.
    pub sandbox_flavor: String,

    /// Persisted result of the last connection probe — populated by
    /// `connection_health` at boot (startup health check), at
    /// create-time (post-create probe in `enforce_on_create`), at
    /// enable-transition time (probe in `enforce_on_update_transition`),
    /// and on every explicit "Test Connection" button press.
    /// See migration 82.
    #[serde(default)]
    pub last_health_check_at: Option<DateTime<Utc>>,
    /// "untested" | "healthy" | "unhealthy". Defaults to "untested"
    /// for freshly-created rows that have never been probed.
    #[serde(default = "default_health_status")]
    pub last_health_check_status: String,
    /// Human reason on unhealthy. `None` on healthy / untested.
    #[serde(default)]
    pub last_health_check_reason: Option<String>,

    // Metadata
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn default_health_status() -> String {
    "untested".to_string()
}

/// Stored OAuth 2.1 `client_credentials` configuration for an external HTTP MCP
/// server (one row per server; built-in servers never use this). The secret is
/// plaintext at rest, mirroring the `llm_providers.api_key` precedent.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpServerOAuthConfig {
    pub server_id: Uuid,
    pub client_id: String,
    pub client_secret: String,
    /// Space-separated OAuth scopes; `None` when the server requires none.
    pub scopes: Option<String>,
    /// RFC 8707 resource indicator (usually the server URL); `None` = omit.
    pub resource: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl McpServerOAuthConfig {
    /// Convert to the transport-layer client config used by `HttpMcpClient`.
    pub fn into_client_config(self) -> crate::modules::mcp::client::auth::OAuthClientConfig {
        crate::modules::mcp::client::auth::OAuthClientConfig {
            client_id: self.client_id,
            client_secret: self.client_secret,
            scopes: self.scopes,
            resource: self.resource,
        }
    }

    /// Convert to the API response shape — deliberately omits the secret value.
    pub fn to_response(&self) -> McpServerOAuthConfigResponse {
        McpServerOAuthConfigResponse {
            server_id: self.server_id,
            client_id: self.client_id.clone(),
            has_client_secret: !self.client_secret.is_empty(),
            scopes: self.scopes.clone(),
            resource: self.resource.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// Request to set (create or replace) a server's OAuth config.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SetMcpServerOAuthConfigRequest {
    pub client_id: String,
    pub client_secret: String,
    #[serde(default)]
    pub scopes: Option<String>,
    #[serde(default)]
    pub resource: Option<String>,
}

/// API view of a server's OAuth config — **never** includes the secret value.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpServerOAuthConfigResponse {
    pub server_id: Uuid,
    pub client_id: String,
    /// Whether a client secret is stored (the value itself is never returned).
    pub has_client_secret: bool,
    pub scopes: Option<String>,
    pub resource: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
