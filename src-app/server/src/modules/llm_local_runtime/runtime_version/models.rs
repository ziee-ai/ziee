//! Request/Response models for runtime version management API

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// =====================================================
// Database Entity
// =====================================================

/// Runtime version database entity
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeVersion {
    pub id: Uuid,
    pub engine: String,
    pub version: String,
    pub platform: String,
    pub arch: String,
    pub backend: String,
    pub binary_path: String,
    pub is_system_default: bool,
    pub created_at: DateTime<Utc>,
}

// =====================================================
// Request Models
// =====================================================

/// Request to download and register a runtime version
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DownloadVersionRequest {
    /// Engine type (llamacpp or mistralrs)
    pub engine: String,
    /// Version tag (e.g., "v1.0.0")
    pub version: String,
    /// Platform (linux, macos, windows)
    pub platform: String,
    /// Architecture (x86_64, arm64)
    pub arch: String,
    /// Backend (cpu, cuda, rocm, metal)
    pub backend: String,
}

/// Request to set a version as system default
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SetSystemDefaultRequest {
    /// Runtime version ID
    pub version_id: Uuid,
}

// =====================================================
// Response Models
// =====================================================

/// Response containing a single runtime version
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeVersionResponse {
    pub id: Uuid,
    pub engine: String,
    pub version: String,
    pub platform: String,
    pub arch: String,
    pub backend: String,
    pub binary_path: String,
    pub is_system_default: bool,
    pub created_at: DateTime<Utc>,
}

impl From<RuntimeVersion> for RuntimeVersionResponse {
    fn from(v: RuntimeVersion) -> Self {
        Self {
            id: v.id,
            engine: v.engine,
            version: v.version,
            platform: v.platform,
            arch: v.arch,
            backend: v.backend,
            binary_path: v.binary_path,
            is_system_default: v.is_system_default,
            created_at: v.created_at,
        }
    }
}

/// Response containing a list of runtime versions
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeVersionListResponse {
    pub versions: Vec<RuntimeVersionResponse>,
}

/// Response after downloading and registering a version
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DownloadVersionResponse {
    pub version: RuntimeVersionResponse,
    pub downloaded: bool,
    pub message: String,
}

/// Response containing available updates from GitHub
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AvailableUpdatesResponse {
    pub engine: String,
    pub available_versions: Vec<String>,
}

/// Response after syncing cache with database
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SyncCacheResponse {
    pub synced_count: usize,
    pub message: String,
}
