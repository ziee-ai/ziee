//! Request/Response models for whisper runtime version management.
//!
//! Mirrors `llm_local_runtime::runtime_version::models` minus the `engine`
//! field — voice manages a SINGLE engine (whisper), so the version identity is
//! just (version, platform, arch, backend).

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// =====================================================
// Database Entity
// =====================================================

/// Whisper runtime version database entity (`voice_runtime_versions`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeVersion {
    pub id: Uuid,
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

/// Request to download and register a whisper runtime version. `platform` /
/// `arch` default to the detected host and `backend` defaults to `cpu` (the
/// v1 whisper backend) when omitted — an admin installs the host's build with
/// just a version tag.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DownloadVersionRequest {
    /// Version tag (e.g., "v1.0.0", or "latest").
    pub version: String,
    /// Platform (linux, macos, windows). Defaults to the host platform.
    #[serde(default)]
    pub platform: Option<String>,
    /// Architecture (x86_64, aarch64). Defaults to the host arch.
    #[serde(default)]
    pub arch: Option<String>,
    /// Backend (cpu, cuda, metal). Defaults to `cpu`.
    #[serde(default)]
    pub backend: Option<String>,
}

// =====================================================
// Response Models
// =====================================================

/// Response containing a single whisper runtime version.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeVersionResponse {
    pub id: Uuid,
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

/// Response containing a list of whisper runtime versions.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeVersionListResponse {
    pub versions: Vec<RuntimeVersionResponse>,
}

/// Response when a download task is started (or joined for an already-running
/// download of the same version/backend). Detached: the download keeps running
/// on the server even after the HTTP request returns.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DownloadVersionStartedResponse {
    pub task_id: Uuid,
    /// Composite key `whisper@{version}@{backend}` — also the path segment for
    /// the events / snapshot endpoints.
    pub key: String,
    pub version: String,
    pub backend: String,
    /// Current status snapshot at the moment the task was started or joined.
    pub status: String,
    /// Ready-to-use SSE URL for the frontend's EventSource (relative to the API
    /// root). Includes the encoded key.
    pub events_url: String,
}

/// One entry returned by `GET /voice/versions/downloads`. Lists every download
/// task currently held by the in-process registry (running OR
/// terminal-but-not-replaced).
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DownloadSnapshot {
    pub task_id: Uuid,
    pub key: String,
    pub version: String,
    pub backend: String,
    pub status: String,
    pub bytes_received: u64,
    /// `None` when the upstream omitted Content-Length.
    pub total_bytes: Option<u64>,
    /// 0..=100 when `total_bytes` is set.
    pub percent: Option<f32>,
    /// Result version id when terminal=Completed; null otherwise.
    pub result_version_id: Option<Uuid>,
    pub error: Option<String>,
}

/// `GET /voice/versions/downloads` response.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DownloadListResponse {
    pub downloads: Vec<DownloadSnapshot>,
}

/// One upstream release in the update-check diff, enriched with what we have
/// installed and whether its binary is published for *this host*.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AvailableVersion {
    /// Release tag (e.g. `v0.0.1-alpha`).
    pub version: String,
    /// True if at least one backend of this version is installed for the host.
    pub installed: bool,
    /// Backends already installed for the host platform/arch (e.g. `["cpu"]`).
    pub installed_backends: Vec<String>,
    /// True if the binary for the host platform/arch is published upstream.
    /// False ⇒ the release exists but its build for this host is pending.
    pub binary_ready: bool,
    /// Backends published upstream for the host platform/arch.
    pub available_backends: Vec<String>,
    /// The backend artifact recommended for this host given detected GPU/driver.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommended_backend: Option<String>,
    /// Byte size of the archive the inline Install button would fetch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
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
    /// Host platform the asset-readiness was computed for (`linux`/`macos`/`windows`).
    pub platform: String,
    /// Host architecture (`x86_64`/`aarch64`).
    pub arch: String,
    pub versions: Vec<AvailableVersion>,
}

/// Response after syncing cache with database.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SyncCacheResponse {
    pub synced_count: usize,
    pub message: String,
}
