//! Engine support, folded in from the former standalone `llm-runtime`
//! crate (the server was its sole consumer).
//!
//! - `download`  — fetch/extract/cache prebuilt engine binaries (GitHub
//!   releases, with a debug-only mirror hook + safe-symlink extraction).
//! - `metadata`  — GGUF / safetensors header parsing → `ModelCapabilities`.
//! - `types`     — `EngineType`, `DeviceType`, and the canonical per-engine
//!   settings structs that `deployment::local` maps onto each CLI.
//! - `health`    — the health state machine (exponential backoff +
//!   flap-detection) wired into the auto-start crash path.
//! - `binary`    — `ensure_executable`.
//! - `error`     — module-local `RuntimeError`/`Result` (mapped to
//!   `AppError` at the `binary_manager` seam).

pub mod binary;
pub mod download;
pub mod error;
pub mod health;
pub mod metadata;
pub mod types;

// Re-export the types consumers reference directly. Everything else
// (BinaryInfo, the error type, HealthSignal/backoff/window, DeviceType)
// stays reachable via its submodule path, e.g. `engine::error::RuntimeError`.
pub use download::{available_backends, BinaryDownloader};
pub use health::{HealthEvent, HealthStateMachine, InstanceState, Transition};
pub use metadata::{extract_model_capabilities, ModelCapabilities};
pub use types::{EngineType, LlamaCppSettings, MistralRsSettings};
