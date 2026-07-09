//! Whisper ggml model management: resolve → (air-gap detect | direct-URL
//! download + sha256 verify) → cache under `<app_data>/voice-models/`.
//!
//! Unlike `llm_model` (git-LFS/HF-repo), whisper models are single files fetched
//! by direct URL. The streaming download + cap + SSE progress land in a later
//! layer; this file owns the on-disk resolution + presence check + the supported
//! set so the settings validator, capability endpoint, and deployment layer can
//! all agree on where a model lives.

use std::path::PathBuf;

/// The whisper models the admin may select. Multilingual unless `.en`.
pub const SUPPORTED_MODELS: &[&str] = &["tiny", "base", "base.en", "small"];

/// True when `name` is an offered model.
pub fn is_supported_model(name: &str) -> bool {
    SUPPORTED_MODELS.contains(&name)
}

/// `<app_data>/voice-models/` — the model cache (also the air-gap pre-stage dir).
pub fn models_dir() -> PathBuf {
    crate::core::get_app_data_dir().join("voice-models")
}

/// The ggml filename for a model, e.g. `ggml-base.bin`.
pub fn model_filename(name: &str) -> String {
    format!("ggml-{name}.bin")
}

/// The on-disk path a model resolves to (present or not).
pub fn model_path(name: &str) -> PathBuf {
    models_dir().join(model_filename(name))
}

/// True when a non-empty model file exists on disk (downloaded or pre-staged).
pub fn model_present(name: &str) -> bool {
    match std::fs::metadata(model_path(name)) {
        Ok(m) => m.is_file() && m.len() > 0,
        Err(_) => false,
    }
}
