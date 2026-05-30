// Request/response models for local runtime API

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use schemars::JsonSchema;
use chrono::{DateTime, Utc};

// =====================================================
// Instance Management Models
// =====================================================

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct StartInstanceRequest {
    // Currently no parameters needed - deployment is always local
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InstanceResponse {
    pub id: Uuid,
    pub model_id: Uuid,
    pub provider_id: Uuid,
    pub runtime_version_id: Option<Uuid>,
    pub local_port: i32,
    pub base_url: String,
    pub status: String,
    pub error_message: Option<String>,
    pub started_at: DateTime<Utc>,
    pub last_health_check: Option<DateTime<Utc>>,
    pub stopped_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InstanceStatusResponse {
    pub model_id: Uuid,
    pub status: String,
    pub base_url: Option<String>,
    pub uptime_seconds: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HealthCheckResponse {
    pub healthy: bool,
    pub message: Option<String>,
    pub response_time_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LogsResponse {
    pub model_id: Uuid,
    pub logs: Vec<String>,
}

// =====================================================
// Provider Instances Models
// =====================================================

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ProviderInstancesResponse {
    pub provider_id: Uuid,
    pub instances: Vec<InstanceResponse>,
}

// =====================================================
// Deployment Configuration
// =====================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type")]
pub enum DeploymentConfig {
    #[serde(rename = "local")]
    Local {
        binary_path: Option<String>,
    },
}

// =====================================================
// SSE event types for live engine log streaming (P2)
// =====================================================

/// A single captured engine log line.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSELogLineData {
    pub line: String,
}

/// Emitted when the broadcast buffer overflowed and the subscriber
/// missed lines (slow reader).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SSELogLagData {
    pub message: String,
    pub dropped: u64,
}

// Typed SSE event stream for `GET /local-runtime/models/{id}/logs/stream`.
// The macro generates camelCase event names (`Log` → "log",
// `Lag` → "lag"), `Into<axum::sse::Event>`, and drives the typed
// `SSECallback` in the generated TS client (no `as never` casts).
crate::sse_event_enum! {
    #[derive(Debug, Clone, Serialize, JsonSchema)]
    pub enum SSELogEvent {
        Log(SSELogLineData),
        Lag(SSELogLagData),
    }
}
