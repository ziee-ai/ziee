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
