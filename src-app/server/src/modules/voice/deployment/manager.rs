//! Deployment manager for the single managed whisper-server.
//!
//! Mirrors `llm_local_runtime`'s `DeploymentManager` + the global
//! `DEPLOYMENT_MANAGER` singleton, scoped to ONE instance. Unlike the LLM
//! manager it needs no DB pool at construction — the whisper binary is resolved
//! lazily at spawn time via `binary_manager::ensure_binary_path()` and all DB
//! access goes through the global `crate::core::Repos`.
//!
//! The accessor lazily initializes on first use (the voice `mod.rs::init` is
//! owned by the foundation layer and does not construct this), so callers can
//! simply `get_deployment_manager()` without an explicit boot step.

use std::sync::Arc;

use once_cell::sync::OnceCell;

use super::LocalDeployment;

/// Orchestrates the single local whisper-server instance.
pub struct DeploymentManager {
    local: Arc<LocalDeployment>,
}

impl Default for DeploymentManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DeploymentManager {
    pub fn new() -> Self {
        Self {
            local: Arc::new(LocalDeployment::new()),
        }
    }

    /// The single local deployment strategy.
    pub fn local(&self) -> Arc<LocalDeployment> {
        self.local.clone()
    }
}

/// Global singleton — there is exactly one whisper-server per process.
static DEPLOYMENT_MANAGER: OnceCell<Arc<DeploymentManager>> = OnceCell::new();

/// Get the global deployment manager, lazily initializing it on first use.
pub fn get_deployment_manager() -> Arc<DeploymentManager> {
    DEPLOYMENT_MANAGER
        .get_or_init(|| Arc::new(DeploymentManager::new()))
        .clone()
}
