// LLM Model infrastructure
#![allow(dead_code)]

// LLM Model models - copied from react-test and refactored for ziee
// Source: react-test/src-tauri/src/database/models/model.rs and download_instance.rs

use crate::common::types::JsonOption;
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Forward declaration: DownloadRequestData and DownloadProgressData are in types.rs
// but we need to reference them here for the DownloadInstance entity

// =====================================================
// ENGINE TYPE
// =====================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EngineType {
    Mistralrs,
    Llamacpp,
    None,
}

impl EngineType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "mistralrs" => Some(Self::Mistralrs),
            "llamacpp" => Some(Self::Llamacpp),
            "none" => Some(Self::None),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mistralrs => "mistralrs",
            Self::Llamacpp => "llamacpp",
            Self::None => "none",
        }
    }
}

impl std::fmt::Display for EngineType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =====================================================
// DEVICE TYPES
// =====================================================

/// Device types for ML model inference
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    /// CPU-only inference
    Cpu,
    /// NVIDIA CUDA GPU acceleration
    Cuda,
    /// Apple Metal GPU acceleration (macOS)
    Metal,
    /// AMD ROCm GPU acceleration
    Rocm,
    /// Vulkan GPU acceleration
    Vulkan,
    /// OpenCL GPU acceleration
    Opencl,
    /// Automatic device detection and selection
    Auto,
}

impl DeviceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeviceType::Cpu => "cpu",
            DeviceType::Cuda => "cuda",
            DeviceType::Metal => "metal",
            DeviceType::Rocm => "rocm",
            DeviceType::Vulkan => "vulkan",
            DeviceType::Opencl => "opencl",
            DeviceType::Auto => "auto",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cpu" => Some(DeviceType::Cpu),
            "cuda" => Some(DeviceType::Cuda),
            "metal" => Some(DeviceType::Metal),
            "rocm" => Some(DeviceType::Rocm),
            "vulkan" => Some(DeviceType::Vulkan),
            "opencl" => Some(DeviceType::Opencl),
            "auto" => Some(DeviceType::Auto),
            _ => None,
        }
    }
}

impl std::fmt::Display for DeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =====================================================
// MISTRALRS COMMAND
// =====================================================

/// MistralRS command types for different model formats and use cases
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum MistralRsCommand {
    /// Plain model format (safetensors/pytorch)
    Plain,
    /// GGUF quantized model format
    Gguf,
    /// Auto-loader for various model formats
    Run,
    /// Vision-enabled plain models for multimodal capabilities
    VisionPlain,
    /// X-LoRA (Cross-Layer LoRA) models
    XLora,
    /// LoRA (Low-Rank Adaptation) models
    Lora,
    /// TOML configuration-based models
    Toml,
}

impl MistralRsCommand {
    pub fn as_str(&self) -> &'static str {
        match self {
            MistralRsCommand::Plain => "plain",
            MistralRsCommand::Gguf => "gguf",
            MistralRsCommand::Run => "run",
            MistralRsCommand::VisionPlain => "vision-plain",
            MistralRsCommand::XLora => "x-lora",
            MistralRsCommand::Lora => "lora",
            MistralRsCommand::Toml => "toml",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "plain" => Some(MistralRsCommand::Plain),
            "gguf" => Some(MistralRsCommand::Gguf),
            "run" => Some(MistralRsCommand::Run),
            "vision-plain" => Some(MistralRsCommand::VisionPlain),
            "x-lora" => Some(MistralRsCommand::XLora),
            "lora" => Some(MistralRsCommand::Lora),
            "toml" => Some(MistralRsCommand::Toml),
            _ => None,
        }
    }
}

impl std::fmt::Display for MistralRsCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =====================================================
// FILE FORMAT
// =====================================================

/// File format types for local models
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FileFormat {
    Safetensors,
    Pytorch,
    Gguf,
}

impl FileFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileFormat::Safetensors => "safetensors",
            FileFormat::Pytorch => "pytorch",
            FileFormat::Gguf => "gguf",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "safetensors" => Some(FileFormat::Safetensors),
            "pytorch" => Some(FileFormat::Pytorch),
            "gguf" => Some(FileFormat::Gguf),
            _ => None,
        }
    }
}

impl std::fmt::Display for FileFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =====================================================
// MODEL CAPABILITIES
// =====================================================

/// Model capabilities configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ModelCapabilities {
    /// Vision capability - can process images
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vision: Option<bool>,
    /// Audio capability - can process audio
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<bool>,
    /// Tools capability - can use function calling/tools
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<bool>,
    /// Code interpreter capability
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_interpreter: Option<bool>,
    /// Chat capability - can engage in conversational text generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat: Option<bool>,
    /// Text embedding capability - can generate text embeddings for semantic search
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_embedding: Option<bool>,
    /// Image generation capability - can generate images from text descriptions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_generator: Option<bool>,
}

impl ModelCapabilities {
    /// Create new capabilities with all disabled
    pub fn new() -> Self {
        Self::default()
    }
}

// =====================================================
// MODEL PARAMETERS
// =====================================================

/// Model parameters for inference configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ModelParameters {
    // Context and generation parameters
    /// Context size for the model
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,

    // Sampling parameters
    /// Temperature for randomness (0.0-2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Top-K sampling parameter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,
    /// Top-P (nucleus) sampling parameter (0.0-1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Min-P sampling parameter (0.0-1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_p: Option<f32>,

    // Repetition control
    /// Number of last tokens to consider for repetition penalty
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_last_n: Option<i32>,
    /// Repetition penalty (1.0 = no penalty)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_penalty: Option<f32>,
    /// Presence penalty for new tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    /// Frequency penalty for repeated tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,

    // Generation control
    /// Random seed for reproducible outputs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i32>,
    /// Stop sequences to terminate generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
}

impl ModelParameters {
    /// Create new parameters with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Create parameters optimized for creative text generation
    pub fn creative() -> Self {
        Self {
            max_tokens: Some(4096),
            temperature: Some(0.8),
            top_k: Some(40),
            top_p: Some(0.95),
            min_p: Some(0.05),
            repeat_last_n: Some(64),
            repeat_penalty: Some(1.1),
            presence_penalty: Some(0.0),
            frequency_penalty: Some(0.0),
            seed: None,
            stop: None,
        }
    }

    /// Create parameters optimized for precise/factual generation
    pub fn precise() -> Self {
        Self {
            max_tokens: Some(4096),
            temperature: Some(0.2),
            top_k: Some(20),
            top_p: Some(0.9),
            min_p: Some(0.1),
            repeat_last_n: Some(64),
            repeat_penalty: Some(1.05),
            presence_penalty: Some(0.1),
            frequency_penalty: Some(0.1),
            seed: None,
            stop: None,
        }
    }

    /// Validate the parameters and return errors if any
    pub fn validate(&self) -> Result<(), String> {
        if let Some(temp) = self.temperature
            && !(0.0..=2.0).contains(&temp) {
                return Err("temperature must be between 0.0 and 2.0".to_string());
            }

        if let Some(top_p) = self.top_p
            && !(0.0..=1.0).contains(&top_p) {
                return Err("top_p must be between 0.0 and 1.0".to_string());
            }

        if let Some(min_p) = self.min_p
            && !(0.0..=1.0).contains(&min_p) {
                return Err("min_p must be between 0.0 and 1.0".to_string());
            }

        if let Some(repeat_penalty) = self.repeat_penalty
            && !(0.0..=2.0).contains(&repeat_penalty) {
                return Err("repeat_penalty must be between 0.0 and 2.0".to_string());
            }

        if let Some(presence_penalty) = self.presence_penalty
            && !(-2.0..=2.0).contains(&presence_penalty) {
                return Err("presence_penalty must be between -2.0 and 2.0".to_string());
            }

        if let Some(frequency_penalty) = self.frequency_penalty
            && !(-2.0..=2.0).contains(&frequency_penalty) {
                return Err("frequency_penalty must be between -2.0 and 2.0".to_string());
            }

        if let Some(stop) = &self.stop {
            if stop.len() > 4 {
                return Err("stop sequences cannot exceed 4 items".to_string());
            }
            for stop_seq in stop {
                if stop_seq.is_empty() {
                    return Err("stop sequences cannot be empty".to_string());
                }
                if stop_seq.len() > 32 {
                    return Err("stop sequences cannot exceed 32 characters each".to_string());
                }
            }
        }

        Ok(())
    }
}

// Continue in next message due to size...
// =====================================================
// MISTRALRS SETTINGS
// =====================================================

/// MistralRs-specific settings for individual model performance and batching configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
pub struct MistralRsSettings {
    // Core model configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<MistralRsCommand>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenizer_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,

    // Quantization and weights
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantized_filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight_file: Option<String>,

    // Device configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_type: Option<DeviceType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_ids: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_device_layers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<bool>,

    // Sequence and memory management
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_seqs: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_seq_len: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_kv_cache: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncate_sequence: Option<bool>,

    // PagedAttention configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paged_attn_gpu_mem: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paged_attn_gpu_mem_usage: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paged_ctxt_len: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paged_attn_block_size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_paged_attn: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paged_attn: Option<bool>,

    // Chat and templates
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jinja_explicit: Option<String>,

    // Performance optimization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_cache_n: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_chunksize: Option<i64>,

    // Model configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dtype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_situ_quant: Option<String>,

    // Reproducibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,

    // Vision model parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_edge: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_num_images: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_image_length: Option<i64>,

    // Server configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serve_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_file: Option<String>,

    // Search capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_search: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_bert_model: Option<String>,

    // Interactive and thinking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interactive_mode: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_thinking: Option<bool>,

    // Token source for authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_source: Option<String>,
}

impl MistralRsSettings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&self) -> Result<(), String> {
        if let Some(max_seqs) = self.max_seqs {
            if max_seqs == 0 {
                return Err("max_seqs must be greater than 0".to_string());
            }
            if max_seqs > 2048 {
                return Err("max_seqs should not exceed 2048".to_string());
            }
        }

        if let Some(paged_attn_block_size) = self.paged_attn_block_size {
            if paged_attn_block_size == 0 {
                return Err("paged_attn_block_size must be greater than 0".to_string());
            }
            if paged_attn_block_size > 512 {
                return Err("paged_attn_block_size should not exceed 512".to_string());
            }
        }

        if let Some(gpu_mem) = self.paged_attn_gpu_mem {
            if gpu_mem == 0 {
                return Err("paged_attn_gpu_mem must be greater than 0".to_string());
            }
            if gpu_mem > 65536 {
                return Err("paged_attn_gpu_mem should not exceed 65536MB (64GB)".to_string());
            }
        }

        if let Some(usage) = self.paged_attn_gpu_mem_usage
            && (!(0.0..1.0).contains(&usage) || usage == 0.0) {
                return Err("paged_attn_gpu_mem_usage must be between 0 and 1".to_string());
            }

        if let Some(prefix_cache_n) = self.prefix_cache_n
            && prefix_cache_n == 0 {
                return Err("prefix_cache_n must be greater than 0".to_string());
            }

        if let Some(max_seq_len) = self.max_seq_len
            && max_seq_len > 131072 {
                return Err("max_seq_len should not exceed 131072 tokens".to_string());
            }

        Ok(())
    }
}

// =====================================================
// LLAMACPP SETTINGS
// =====================================================

/// LlamaCpp-specific settings for llama-server configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
pub struct LlamaCppSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_type: Option<DeviceType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_ids: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ctx_size: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_size: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ubatch_size: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mlock: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_mmap: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threads: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threads_batch: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cont_batching: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flash_attn: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_kv_offload: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_gpu_layers: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_gpu: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tensor_split: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rope_freq_base: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rope_freq_scale: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rope_scaling: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_type_k: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_type_v: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numa: Option<String>,
}

impl LlamaCppSettings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&self) -> Result<(), String> {
        if let Some(ctx_size) = self.ctx_size {
            if ctx_size <= 0 {
                return Err("ctx_size must be greater than 0".to_string());
            }
            if ctx_size > 131072 {
                return Err("ctx_size should not exceed 131072 tokens".to_string());
            }
        }

        if let Some(batch_size) = self.batch_size
            && batch_size <= 0 {
                return Err("batch_size must be greater than 0".to_string());
            }

        if let Some(parallel) = self.parallel {
            if parallel <= 0 {
                return Err("parallel must be greater than 0".to_string());
            }
            if parallel > 64 {
                return Err("parallel should not exceed 64".to_string());
            }
        }

        if let Some(n_gpu_layers) = self.n_gpu_layers
            && n_gpu_layers < 0 {
                return Err("n_gpu_layers must be non-negative".to_string());
            }

        if let Some(main_gpu) = self.main_gpu
            && main_gpu < 0 {
                return Err("main_gpu must be non-negative".to_string());
            }

        if let Some(split_mode) = &self.split_mode {
            match split_mode.as_str() {
                "none" | "layer" | "row" => {}
                _ => return Err("split_mode must be 'none', 'layer', or 'row'".to_string()),
            }
        }

        if let Some(rope_scaling) = &self.rope_scaling {
            match rope_scaling.as_str() {
                "none" | "linear" | "yarn" => {}
                _ => return Err("rope_scaling must be 'none', 'linear', or 'yarn'".to_string()),
            }
        }

        if let Some(numa) = &self.numa {
            match numa.as_str() {
                "distribute" | "isolate" | "numactl" => {}
                _ => return Err("numa must be 'distribute', 'isolate', or 'numactl'".to_string()),
            }
        }

        Ok(())
    }
}

// =====================================================
// ENGINE SETTINGS
// =====================================================

/// Engine-specific settings for model configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
pub struct ModelEngineSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mistralrs: Option<MistralRsSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llamacpp: Option<LlamaCppSettings>,
}

// =====================================================
// DATABASE ENTITIES
// =====================================================

/// LLM Model database entity
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LlmModel {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub name: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub enabled: bool,
    pub is_deprecated: bool,
    pub is_active: bool,
    pub capabilities: ModelCapabilities,
    pub parameters: ModelParameters,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size_bytes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_issues: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<i32>,
    pub engine_type: EngineType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine_settings: Option<ModelEngineSettings>,
    pub file_format: FileFormat,
    /// Required runtime version for this model
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_runtime_version_id: Option<Uuid>,
}

/// Model file database entity
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModelFile {
    pub id: Uuid,
    pub model_id: Uuid,
    pub filename: String,
    pub file_path: String,
    pub file_size_bytes: i64,
    pub file_type: String,
    pub upload_status: String,
    pub uploaded_at: DateTime<Utc>,
}

/// LLM Repository database entity
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LlmRepository {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub auth_type: String,
    pub auth_config: serde_json::Value,
    pub enabled: bool,
    pub built_in: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// =====================================================
// DOWNLOAD TYPES
// =====================================================

/// Download phase enum
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DownloadPhase {
    Created,
    Connecting,
    Analyzing,
    Downloading,
    Receiving,
    Resolving,
    CheckingOut,
    Committing,
    Complete,
    Error,
}

/// Download status enum
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Pending,
    Downloading,
    Completed,
    Failed,
    Cancelled,
}

impl DownloadStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DownloadStatus::Pending => "pending",
            DownloadStatus::Downloading => "downloading",
            DownloadStatus::Completed => "completed",
            DownloadStatus::Failed => "failed",
            DownloadStatus::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(DownloadStatus::Pending),
            "downloading" => Some(DownloadStatus::Downloading),
            "completed" => Some(DownloadStatus::Completed),
            "failed" => Some(DownloadStatus::Failed),
            "cancelled" => Some(DownloadStatus::Cancelled),
            _ => None,
        }
    }
}

/// Progress data for download tracking (stored as JSON in database)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DownloadProgressData {
    pub phase: DownloadPhase,
    pub current: i64,
    pub total: i64,
    pub message: String,
    pub speed_bps: i64,
    pub eta_seconds: i64,
}

impl Default for DownloadProgressData {
    fn default() -> Self {
        Self {
            phase: DownloadPhase::Created,
            current: 0,
            total: 0,
            message: String::new(),
            speed_bps: 0,
            eta_seconds: 0,
        }
    }
}

/// Request data for initiating a download (stored as JSON in database)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct DownloadRequestData {
    pub model_name: String,
    pub revision: Option<String>,
    pub files: Option<Vec<String>>,
    pub quantization: Option<String>,
    pub repository_path: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub file_format: Option<String>,
    pub main_filename: Option<String>,
    pub capabilities: Option<ModelCapabilities>,
    pub parameters: Option<ModelParameters>,
    pub engine_type: Option<EngineType>,
    pub engine_settings: Option<ModelEngineSettings>,
}

/// Download instance database entity
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DownloadInstance {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub repository_id: Uuid,
    pub request_data: DownloadRequestData, // NOT NULL in database
    pub status: DownloadStatus,
    pub progress_data: JsonOption<DownloadProgressData>, // Nullable in database
    pub error_message: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub model_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Implement From traits for SQLx query_as! to work
crate::impl_json_from!(DownloadRequestData);
crate::impl_json_from!(DownloadProgressData);
crate::impl_string_to_enum!(DownloadStatus);
crate::impl_json_option_from!(DownloadProgressData);

impl DownloadInstance {
    /// Check if the download is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
        )
    }

    /// Check if the download can be cancelled
    pub fn can_cancel(&self) -> bool {
        matches!(
            self.status,
            DownloadStatus::Pending | DownloadStatus::Downloading
        )
    }
}
