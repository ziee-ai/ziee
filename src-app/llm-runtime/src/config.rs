//! Configuration types for the LLM runtime

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

use crate::error::{Result, RuntimeError};

/// Root configuration for the LLM runtime
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Global settings
    #[serde(default)]
    pub global: GlobalSettings,

    /// List of engine instances to manage
    pub instances: Vec<InstanceConfig>,
}

impl RuntimeConfig {
    /// Load configuration from YAML file
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: RuntimeConfig = serde_yaml::from_str(&contents)?;
        config.validate()?;
        Ok(config)
    }

    /// Load configuration from YAML string
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let config: RuntimeConfig = serde_yaml::from_str(yaml)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Check for duplicate instance IDs
        let mut ids = std::collections::HashSet::new();
        for instance in &self.instances {
            if !ids.insert(&instance.id) {
                return Err(RuntimeError::config(format!(
                    "Duplicate instance ID: {}",
                    instance.id
                )));
            }
        }

        // Validate each instance
        for instance in &self.instances {
            instance.validate()?;
        }

        Ok(())
    }
}

/// Global runtime settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    /// Directory for log files
    #[serde(default = "default_log_dir")]
    pub log_dir: PathBuf,

    /// Health check interval in seconds
    #[serde(default = "default_health_check_interval")]
    pub health_check_interval_secs: u64,

    /// Startup timeout in seconds
    #[serde(default = "default_startup_timeout")]
    pub startup_timeout_secs: u64,

    /// Shutdown timeout in seconds
    #[serde(default = "default_shutdown_timeout")]
    pub shutdown_timeout_secs: u64,

    /// Enable auto-restart on crash
    #[serde(default)]
    pub auto_restart: bool,

    /// Maximum restart attempts before giving up
    #[serde(default = "default_max_restart_attempts")]
    pub max_restart_attempts: u32,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            log_dir: default_log_dir(),
            health_check_interval_secs: default_health_check_interval(),
            startup_timeout_secs: default_startup_timeout(),
            shutdown_timeout_secs: default_shutdown_timeout(),
            auto_restart: false,
            max_restart_attempts: default_max_restart_attempts(),
        }
    }
}

impl GlobalSettings {
    /// Get health check interval as Duration
    pub fn health_check_interval(&self) -> Duration {
        Duration::from_secs(self.health_check_interval_secs)
    }

    /// Get startup timeout as Duration
    pub fn startup_timeout(&self) -> Duration {
        Duration::from_secs(self.startup_timeout_secs)
    }

    /// Get shutdown timeout as Duration
    pub fn shutdown_timeout(&self) -> Duration {
        Duration::from_secs(self.shutdown_timeout_secs)
    }
}

/// Configuration for a single engine instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceConfig {
    /// Unique identifier for this instance
    pub id: String,

    /// Engine type (llamacpp or mistralrs)
    pub engine: EngineType,

    /// Path to the model file or directory
    pub model_path: PathBuf,

    /// Device to run on
    pub device: DeviceType,

    /// Engine-specific settings
    #[serde(default)]
    pub settings: EngineSettings,
}

impl InstanceConfig {
    /// Validate instance configuration
    pub fn validate(&self) -> Result<()> {
        // Check model path exists
        if !self.model_path.exists() {
            return Err(RuntimeError::config(format!(
                "Model path does not exist: {}",
                self.model_path.display()
            )));
        }

        // Validate engine-specific settings
        self.settings.validate(&self.engine)?;

        Ok(())
    }
}

/// Engine type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineType {
    /// LlamaCpp engine
    #[serde(alias = "llama", alias = "llamacpp", alias = "llama-cpp")]
    Llamacpp,

    /// MistralRS engine
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

/// Device type for running models
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    /// CPU only
    Cpu,
    /// NVIDIA CUDA
    Cuda,
    /// Apple Metal
    Metal,
    /// AMD ROCm
    Rocm,
    /// Vulkan
    Vulkan,
    /// OpenCL
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

/// Engine-specific settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineSettings {
    /// Port to bind to (optional, auto-assigned if not specified)
    pub port: Option<u16>,

    /// LlamaCpp-specific settings
    #[serde(default)]
    pub llamacpp: LlamaCppSettings,

    /// MistralRS-specific settings
    #[serde(default)]
    pub mistralrs: MistralRsSettings,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            port: None,
            llamacpp: LlamaCppSettings::default(),
            mistralrs: MistralRsSettings::default(),
        }
    }
}

impl EngineSettings {
    /// Validate settings for the given engine type
    pub fn validate(&self, engine_type: &EngineType) -> Result<()> {
        match engine_type {
            EngineType::Llamacpp => self.llamacpp.validate(),
            EngineType::Mistralrs => self.mistralrs.validate(),
        }
    }
}

/// LlamaCpp engine settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaCppSettings {
    /// Context size in tokens
    #[serde(default = "default_ctx_size")]
    pub ctx_size: u32,

    /// Number of GPU layers to offload
    #[serde(default)]
    pub n_gpu_layers: u32,

    /// Batch size
    #[serde(default = "default_batch_size")]
    pub batch_size: u32,

    /// Number of threads
    #[serde(default)]
    pub threads: Option<u32>,

    /// Enable embeddings endpoint
    #[serde(default)]
    pub embeddings: bool,

    /// RoPE frequency base
    #[serde(default)]
    pub rope_freq_base: Option<f32>,

    /// RoPE frequency scale
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
    /// Validate LlamaCpp settings
    pub fn validate(&self) -> Result<()> {
        if self.ctx_size == 0 || self.ctx_size > 131072 {
            return Err(RuntimeError::config(
                "ctx_size must be between 1 and 131072",
            ));
        }

        if self.batch_size == 0 || self.batch_size > 4096 {
            return Err(RuntimeError::config(
                "batch_size must be between 1 and 4096",
            ));
        }

        Ok(())
    }
}

/// MistralRS engine settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MistralRsSettings {
    /// Maximum number of sequences
    #[serde(default = "default_max_seqs")]
    pub max_seqs: u32,

    /// PagedAttention GPU memory in MB
    #[serde(default)]
    pub pa_gpu_mem_mb: Option<u32>,

    /// Prefix cache size
    #[serde(default = "default_prefix_cache_n")]
    pub prefix_cache_n: u32,

    /// Data type (f16, f32, bf16)
    #[serde(default = "default_dtype")]
    pub dtype: String,

    /// Model format (auto, gguf, safetensors)
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
    /// Validate MistralRS settings
    pub fn validate(&self) -> Result<()> {
        if self.max_seqs == 0 || self.max_seqs > 256 {
            return Err(RuntimeError::config("max_seqs must be between 1 and 256"));
        }

        // Validate dtype
        let valid_dtypes = ["f16", "f32", "bf16", "auto"];
        if !valid_dtypes.contains(&self.dtype.as_str()) {
            return Err(RuntimeError::config(format!(
                "Invalid dtype '{}'. Must be one of: {}",
                self.dtype,
                valid_dtypes.join(", ")
            )));
        }

        // Validate model_format
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

// Default value functions

fn default_log_dir() -> PathBuf {
    PathBuf::from("./logs")
}

fn default_health_check_interval() -> u64 {
    30
}

fn default_startup_timeout() -> u64 {
    300
}

fn default_shutdown_timeout() -> u64 {
    10
}

fn default_max_restart_attempts() -> u32 {
    3
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
