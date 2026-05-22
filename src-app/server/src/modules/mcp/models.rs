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
    pub environment_variables: serde_json::Value,

    // http/sse transport
    pub url: Option<String>,
    pub headers: serde_json::Value,

    // Runtime configuration
    pub timeout_seconds: i32,

    // Sampling configuration
    pub supports_sampling: bool,
    pub usage_mode: UsageMode,
    pub max_concurrent_sessions: Option<i32>,

    // Metadata
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
