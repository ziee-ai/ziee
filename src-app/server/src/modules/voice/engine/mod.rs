//! whisper-server engine binary support.
//!
//! - `download` — fetch/extract/cache the prebuilt `whisper-server` binary from
//!   the `ziee-ai/whisper.cpp` fork's GitHub releases (MANDATORY sha256 sidecar
//!   verify, debug-only mirror hooks, safe-symlink extraction). Single engine,
//!   so there is no `EngineType` seam — the repo slug + binary name are fixed.
//! - `health`   — the health state machine (owned by the lifecycle-layer agent;
//!   declared here so `voice::engine::health` resolves once that file lands).

pub mod download;
pub mod health;

// Re-export the surface the version registry + binary_manager reference so
// callers use `voice::engine::…` rather than the submodule path. The remaining
// public types (`AssetInfo`/`BinaryInfo`/`ReleaseInfo`) stay reachable via
// `engine::download::…`.
pub use download::{WhisperDownloader, asset_size_for_backend, available_backends};
