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
