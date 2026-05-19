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
