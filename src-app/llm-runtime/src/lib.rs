//! # LLM Runtime
//!
//! A standalone inference runtime for managing local LLM engines (LlamaCpp, MistralRS).
//!
//! ## Features
//!
//! - **Unified Interface**: Single API for multiple engine types
//! - **Process Management**: Automatic process spawning, health checks, and cleanup
//! - **YAML Configuration**: Simple, declarative configuration
//! - **Supervision**: Auto-restart on crash, periodic health checks
//! - **Stateless**: No database dependencies, config-driven
//!
//! ## Quick Start
//!
//! ```no_run
//! use llm_runtime::{RuntimeConfig, Runtime};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Load configuration
//!     let config = RuntimeConfig::from_file("config.yaml")?;
//!
//!     // Create runtime
//!     let mut runtime = Runtime::new(config).await?;
//!
//!     // Start an instance
//!     let handle = runtime.start("my-model").await?;
//!     println!("Model running at: {}", handle.base_url);
//!
//!     // Health check
//!     let health = runtime.health_check("my-model").await?;
//!     println!("Health: {:?}", health);
//!
//!     // Stop when done
//!     runtime.stop("my-model").await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Configuration Example
//!
//! ```yaml
//! global:
//!   log_dir: ./logs
//!   health_check_interval_secs: 30
//!   startup_timeout_secs: 300
//!
//! instances:
//!   - id: llama-model
//!     engine: llamacpp
//!     model_path: /models/llama-3.1-8b.gguf
//!     device: cuda
//!     settings:
//!       ctx_size: 8192
//!       n_gpu_layers: 35
//!       batch_size: 512
//!
//!   - id: mistral-model
//!     engine: mistralrs
//!     model_path: /models/mistral-7b
//!     device: metal
//!     settings:
//!       max_seqs: 64
//!       dtype: f16
//! ```

// Re-export main types
pub use config::{
    DeviceType, EngineSettings, EngineType, GlobalSettings, InstanceConfig,
    LlamaCppSettings, MistralRsSettings, RuntimeConfig,
};
pub use engine::{Engine, EngineHandle, HealthStatus, InstanceInfo};
pub use error::{Result, RuntimeError};
pub use runtime::Runtime;

// Modules
pub mod config;
pub mod engine;
pub mod error;
pub mod runtime;

// Internal modules (not public API)
mod binary;
mod health;
mod supervisor;
