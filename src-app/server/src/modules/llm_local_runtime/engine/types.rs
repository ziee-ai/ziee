//! Engine types + the canonical per-engine settings vocabulary.
//!
//! Ported from the standalone runtime's `config.rs` — only the parts the
//! server needs (the CLI-only `RuntimeConfig`/`GlobalSettings`/
//! `InstanceConfig` were dropped). The settings structs are the single
//! source of truth for the knobs `deployment::local`'s arg-builders map
//! onto each engine's CLI; a model's `engine_settings` JSONB deserializes
//! into them.

use serde::{Deserialize, Serialize};

use super::error::{Result, RuntimeError};

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

/// Device type for running models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Cpu,
    Cuda,
    Metal,
    Rocm,
    Vulkan,
    Opencl,
}

impl std::fmt::Display for DeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cpu => write!(f, "cpu"),
            Self::Cuda => write!(f, "cuda"),
            Self::Metal => write!(f, "metal"),
            Self::Rocm => write!(f, "rocm"),
            Self::Vulkan => write!(f, "vulkan"),
            Self::Opencl => write!(f, "opencl"),
        }
    }
}

impl Default for DeviceType {
    fn default() -> Self {
        Self::Cpu
    }
}

/// LlamaCpp engine settings (the canonical `engine_settings` keys).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaCppSettings {
    #[serde(default = "default_ctx_size")]
    pub ctx_size: u32,
    #[serde(default)]
    pub n_gpu_layers: u32,
    #[serde(default = "default_batch_size")]
    pub batch_size: u32,
    #[serde(default)]
    pub threads: Option<u32>,
    #[serde(default)]
    pub embeddings: bool,
    #[serde(default)]
    pub rope_freq_base: Option<f32>,
    #[serde(default)]
    pub rope_freq_scale: Option<f32>,
}

impl Default for LlamaCppSettings {
    fn default() -> Self {
        Self {
            ctx_size: default_ctx_size(),
            n_gpu_layers: 0,
            batch_size: default_batch_size(),
            threads: None,
            embeddings: false,
            rope_freq_base: None,
            rope_freq_scale: None,
        }
    }
}

impl LlamaCppSettings {
    pub fn validate(&self) -> Result<()> {
        if self.ctx_size == 0 || self.ctx_size > 131072 {
            return Err(RuntimeError::config("ctx_size must be between 1 and 131072"));
        }
        if self.batch_size == 0 || self.batch_size > 4096 {
            return Err(RuntimeError::config("batch_size must be between 1 and 4096"));
        }
        Ok(())
    }
}

/// MistralRS engine settings (the canonical `engine_settings` keys).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MistralRsSettings {
    #[serde(default = "default_max_seqs")]
    pub max_seqs: u32,
    #[serde(default)]
    pub pa_gpu_mem_mb: Option<u32>,
    #[serde(default = "default_prefix_cache_n")]
    pub prefix_cache_n: u32,
    /// Data type (f16, f32, bf16, auto).
    #[serde(default = "default_dtype")]
    pub dtype: String,
    /// Model format (auto, gguf, safetensors, pytorch).
    #[serde(default = "default_model_format")]
    pub model_format: String,
}

impl Default for MistralRsSettings {
    fn default() -> Self {
        Self {
            max_seqs: default_max_seqs(),
            pa_gpu_mem_mb: None,
            prefix_cache_n: default_prefix_cache_n(),
            dtype: default_dtype(),
            model_format: default_model_format(),
        }
    }
}

impl MistralRsSettings {
    pub fn validate(&self) -> Result<()> {
        if self.max_seqs == 0 || self.max_seqs > 256 {
            return Err(RuntimeError::config("max_seqs must be between 1 and 256"));
        }
        let valid_dtypes = ["f16", "f32", "bf16", "auto"];
        if !valid_dtypes.contains(&self.dtype.as_str()) {
            return Err(RuntimeError::config(format!(
                "Invalid dtype '{}'. Must be one of: {}",
                self.dtype,
                valid_dtypes.join(", ")
            )));
        }
        let valid_formats = ["auto", "gguf", "safetensors", "pytorch"];
        if !valid_formats.contains(&self.model_format.as_str()) {
            return Err(RuntimeError::config(format!(
                "Invalid model_format '{}'. Must be one of: {}",
                self.model_format,
                valid_formats.join(", ")
            )));
        }
        Ok(())
    }
}

fn default_ctx_size() -> u32 {
    8192
}
fn default_batch_size() -> u32 {
    512
}
fn default_max_seqs() -> u32 {
    64
}
fn default_prefix_cache_n() -> u32 {
    32
}
fn default_dtype() -> String {
    "f16".to_string()
}
fn default_model_format() -> String {
    "auto".to_string()
}
