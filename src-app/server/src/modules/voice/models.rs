//! Request / response DTOs for the voice dictation REST surface.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Deployment-wide voice settings (singleton row). Returned by GET.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct VoiceSettings {
    /// Runtime admin toggle (distinct from the deploy-level config kill switch).
    pub enabled: bool,
    /// Selected whisper ggml model name (tiny | base | base.en | small).
    pub model: String,
    /// Default transcription language ('auto' = whisper auto-detect).
    pub language: String,
    pub idle_unload_secs: i32,
    pub auto_start_timeout_secs: i32,
    pub drain_timeout_secs: i32,
    pub max_clip_seconds: i32,
    pub max_upload_bytes: i64,
    /// Live streaming captions available deployment-wide (also needs `enabled`).
    pub streaming_enabled: bool,
    /// Interim decode cadence in milliseconds for live captions.
    pub stream_interval_ms: i32,
    /// Trailing-window (seconds) each interim clip is clamped to before decoding
    /// — the per-tick cost bound. The final on-stop decode is unclamped.
    pub stream_max_decode_secs: i32,
    /// Admin-configurable whisper-model source repo (default `ggerganov/whisper.cpp`).
    /// The runtime catalog fetch lists downloadable models from here; an operator
    /// repoints it to an internal mirror or a moved upstream with no code change.
    pub model_source_repo: String,
    pub updated_at: DateTime<Utc>,
}

/// PUT body for the global settings. Every field optional → absent = leave.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct UpdateVoiceSettingsRequest {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub idle_unload_secs: Option<i32>,
    #[serde(default)]
    pub auto_start_timeout_secs: Option<i32>,
    #[serde(default)]
    pub drain_timeout_secs: Option<i32>,
    #[serde(default)]
    pub max_clip_seconds: Option<i32>,
    #[serde(default)]
    pub max_upload_bytes: Option<i64>,
    #[serde(default)]
    pub streaming_enabled: Option<bool>,
    #[serde(default)]
    pub stream_interval_ms: Option<i32>,
    #[serde(default)]
    pub stream_max_decode_secs: Option<i32>,
    #[serde(default)]
    pub model_source_repo: Option<String>,
}

/// Readiness snapshot for the composer mic button. Reachable by any user holding
/// `voice::transcribe` (NOT admin-gated) so a normal user can decide whether to
/// enable/disable/hide the mic without touching an admin endpoint.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct VoiceCapability {
    /// Feature enabled (config deploy switch AND runtime settings toggle).
    pub enabled: bool,
    /// A whisper-server runtime binary is installed for this host.
    pub runtime_ready: bool,
    /// The configured whisper model is present on disk.
    pub model_ready: bool,
    /// The configured model name (for display).
    pub model: String,
    /// Max clip length the user may record.
    pub max_clip_seconds: i32,
    /// True when enabled && runtime_ready && model_ready — the mic is usable.
    pub can_transcribe: bool,
    /// Live streaming captions available: the mic is usable (`can_transcribe`)
    /// AND the deployment `streaming_enabled` toggle is on. The composer runs the
    /// interim loop only when this is true.
    pub streaming_enabled: bool,
    /// Interim decode cadence (ms) the composer paces its live-caption loop at.
    pub stream_interval_ms: i32,
}

// =====================================================================
// Whisper model library (download / upload / installed set)
// =====================================================================

/// How an installed model was acquired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum VoiceModelSource {
    /// From the curated/runtime HF catalog (verified against the advertised oid).
    Catalog,
    /// From an admin-supplied arbitrary URL / HF repo (sha256 computed, unverified).
    Url,
    /// Uploaded by the admin (sha256 computed, unverified).
    Upload,
}

/// One installed whisper model (a row in `voice_models`).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct VoiceModel {
    pub id: uuid::Uuid,
    /// Short model name (also the `settings.model` pointer value).
    pub name: String,
    /// On-disk filename under `voice-models/`.
    pub filename: String,
    pub source: VoiceModelSource,
    pub source_url: Option<String>,
    pub size_bytes: i64,
    pub sha256: Option<String>,
    /// Bytes matched a source-of-truth digest (catalog/HF oid).
    pub verified: bool,
    /// This model is the one the whisper-server is configured to serve.
    pub is_active: bool,
    /// The upstream catalog now advertises a different digest → a newer file exists.
    pub update_available: bool,
    pub created_at: DateTime<Utc>,
}

/// One entry in the runtime-fetched downloadable catalog.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct VoiceCatalogModel {
    /// Short model name (derived from the ggml filename).
    pub name: String,
    pub filename: String,
    /// Advertised size in bytes (HF LFS metadata), if known.
    pub size_bytes: Option<i64>,
    /// The HF-advertised git-LFS oid (sha256) used to verify a download.
    pub sha256: Option<String>,
    /// True when an installed model already covers this catalog entry.
    pub installed: bool,
    /// English-only variant (name ends in `.en`).
    pub english_only: bool,
    /// Quantization tag (e.g. `q5_1`, `q8_0`) if the filename carries one.
    pub quantization: Option<String>,
}

/// The catalog list response (+ a source-reachability signal for graceful degrade).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct VoiceCatalogResponse {
    pub models: Vec<VoiceCatalogModel>,
    /// False when the configured source was unreachable (models will be empty).
    pub source_reachable: bool,
    /// The source repo the list was fetched from.
    pub source_repo: String,
}

/// POST body to start a model download.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct DownloadModelRequest {
    /// Catalog: the model name (e.g. `large-v3`). URL/HF: the display name to store as.
    pub name: String,
    /// For an HF-repo download: `owner/repo` (else the configured source repo).
    #[serde(default)]
    pub repository: Option<String>,
    /// For an HF-repo download: the file within the repo.
    #[serde(default)]
    pub filename: Option<String>,
    /// For a raw-URL download: the full https URL to the model file.
    #[serde(default)]
    pub url: Option<String>,
}

/// A non-SSE progress snapshot for a model download (poll fallback + active list).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SnapshotDto {
    pub task_id: uuid::Uuid,
    pub key: String,
    pub name: String,
    pub status: String,
    pub bytes_received: u64,
    pub total_bytes: Option<u64>,
    pub percent: Option<f32>,
    pub error: Option<String>,
}

/// Response echoing a started download + where to subscribe for progress.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DownloadModelStartedResponse {
    pub task_id: uuid::Uuid,
    pub key: String,
    pub name: String,
    pub events_url: String,
}

/// PATCH-ish activate/delete responses reuse `VoiceModel`; delete takes a query ack.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct DeleteModelQuery {
    /// Acknowledge deleting the currently-active model.
    #[serde(default)]
    pub ack_active: bool,
}

/// Result of a transcription request.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TranscriptionResponse {
    /// The recognized text.
    pub text: String,
    /// The language whisper used / detected.
    pub language: String,
    /// Wall-clock transcription time in milliseconds.
    pub duration_ms: i64,
}
