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

/// Response when a download task is started (or joined for an
/// already-running download of the same engine/version/backend).
/// Detached: the download keeps running on the server even after the
/// HTTP request returns or the client disconnects, so a page reload
/// can pick up the in-flight task via `GET /versions/downloads`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DownloadVersionStartedResponse {
    pub task_id: Uuid,
    /// Composite key `{engine}@{version}@{backend}` — also the path
    /// segment for the events / snapshot endpoints.
    pub key: String,
    pub engine: String,
    pub version: String,
    pub backend: String,
    /// Current status snapshot at the moment the task was started or
    /// joined. The SSE stream sends Connected immediately with the
    /// same value so a late subscriber doesn't have to round-trip.
    pub status: String,
    /// Ready-to-use SSE URL for the frontend's EventSource (relative
    /// to the API root). Includes the encoded key.
    pub events_url: String,
}

/// One entry returned by `GET /local-runtime/versions/downloads`.
/// Lists every download task currently held by the in-process
/// registry (running OR terminal-but-not-replaced). Used by the UI
/// on mount to repaint in-flight progress after a page reload.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DownloadSnapshot {
    pub task_id: Uuid,
    pub key: String,
    pub engine: String,
    pub version: String,
    pub backend: String,
    pub status: String,
    pub bytes_received: u64,
    /// `None` when the upstream omitted Content-Length.
    pub total_bytes: Option<u64>,
    /// 0..=100 when `total_bytes` is set.
    pub percent: Option<f32>,
    /// Result version when terminal=Completed; null otherwise.
    pub result_version_id: Option<Uuid>,
    pub error: Option<String>,
}

/// `GET /local-runtime/versions/downloads` response.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DownloadListResponse {
    pub downloads: Vec<DownloadSnapshot>,
}

/// One upstream release in the update-check diff, enriched with what we
/// have installed and whether its binary is published for *this host*.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AvailableVersion {
    /// Release tag (e.g. `v0.0.1-alpha`).
    pub version: String,
    /// True if at least one backend of this version is installed for the
    /// host platform/arch.
    pub installed: bool,
    /// Backends already installed for the host platform/arch (e.g. `["cpu"]`).
    pub installed_backends: Vec<String>,
    /// True if the binary for the host platform/arch is published upstream.
    /// False ⇒ the release exists but its build for this host is pending.
    pub binary_ready: bool,
    /// Backends published upstream for the host platform/arch.
    pub available_backends: Vec<String>,
    /// The backend artifact recommended for this host given its detected
    /// GPU/driver versions (the suitable major-version match), if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommended_backend: Option<String>,
    /// GitHub prerelease flag.
    pub prerelease: bool,
    /// ISO-8601 publish timestamp, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
}

/// Response for the update check: upstream releases diffed against what is
/// installed, scoped to the host platform/arch. Drafts are omitted.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AvailableUpdatesResponse {
    pub engine: String,
    /// Host platform the asset-readiness was computed for (`linux`/`macos`/`windows`).
    pub platform: String,
    /// Host architecture (`x86_64`/`aarch64`).
    pub arch: String,
    pub versions: Vec<AvailableVersion>,
}

/// Response after syncing cache with database
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SyncCacheResponse {
    pub synced_count: usize,
    pub message: String,
}

// =====================================================
// Version usage (models-by-version interface)
// =====================================================

/// A local model and how it relates to a given engine version.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModelUsageInfo {
    pub id: Uuid,
    pub name: String,
    pub display_name: String,
    pub provider_id: Uuid,
    pub provider_name: String,
    /// Engine type (`llamacpp`/`mistralrs`).
    pub engine: String,
    /// Whether a runtime instance is currently running for this model.
    pub running: bool,
    /// True if the model is explicitly pinned to this version
    /// (`required_runtime_version_id`); false if it merely inherits it via
    /// the provider/system default.
    pub pinned: bool,
}

/// One installed engine version + the models that effectively resolve to it.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct VersionUsageEntry {
    pub version: RuntimeVersionResponse,
    pub models: Vec<ModelUsageInfo>,
}

/// Models grouped by the engine version they effectively run on.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct VersionUsageResponse {
    pub versions: Vec<VersionUsageEntry>,
    /// Local models whose engine has no installed version to resolve to.
    pub unresolved: Vec<ModelUsageInfo>,
}

/// Request to swap a model onto another version of the **same** engine.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SwapRuntimeVersionRequest {
    pub version_id: Uuid,
}

/// Result of a version swap.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SwapRuntimeVersionResponse {
    pub model_id: Uuid,
    pub version_id: Uuid,
    /// True if a running instance was restarted onto the new version.
    pub restarted: bool,
}
