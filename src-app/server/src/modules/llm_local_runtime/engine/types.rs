//! Engine + device type enums for the local runtime.
//!
//! Ported from the standalone runtime's `config.rs`. The per-engine
//! *settings* vocabulary (`LlamaCppSettings` / `MistralRsSettings`) now
//! lives in `crate::modules::llm_model::models` as the single source of
//! truth shared by the API/OpenAPI schema, the UI, and
//! `deployment::local`'s arg-builders; a model's `engine_settings` JSONB
//! deserializes into the `ModelEngineSettings` wrapper there.

use serde::{Deserialize, Serialize};

/// Engine type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineType {
    #[serde(alias = "llama", alias = "llamacpp", alias = "llama-cpp")]
    Llamacpp,
    #[serde(alias = "mistral", alias = "mistralrs", alias = "mistral-rs")]
    Mistralrs,
}

impl std::fmt::Display for EngineType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Llamacpp => write!(f, "llamacpp"),
            Self::Mistralrs => write!(f, "mistralrs"),
        }
    }
}

// DeviceType removed — dead code; the GPU-detection provider now uses a
// per-engine settings-based approach (LlamaCppSettings / MistralRsSettings).
