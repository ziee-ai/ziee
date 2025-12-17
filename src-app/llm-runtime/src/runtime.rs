//! Runtime for managing inference engine instances

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::{EngineType, InstanceConfig, RuntimeConfig};
use crate::engine::{Engine, EngineHandle, EngineProcess, HealthStatus, InstanceInfo};
use crate::error::{Result, RuntimeError};
use crate::supervisor::Supervisor;

/// Main runtime for managing engine instances
pub struct Runtime {
    /// Configuration (public for CLI access)
    pub config: RuntimeConfig,

    /// Engine implementations
    engines: Arc<HashMap<EngineType, Arc<dyn Engine>>>,

    /// Running processes
    processes: Arc<RwLock<HashMap<String, EngineProcess>>>,

    /// Process supervisor
    supervisor: Option<Supervisor>,
}

impl Runtime {
    /// Create a new runtime from configuration
    pub async fn new(config: RuntimeConfig) -> Result<Self> {
        // Register engine implementations
        let mut engines: HashMap<EngineType, Arc<dyn Engine>> = HashMap::new();

        // Register LlamaCpp engine
        engines.insert(
            EngineType::Llamacpp,
            Arc::new(crate::engine::llamacpp::LlamaCppEngine::new()),
        );

        // Register MistralRS engine
        engines.insert(
            EngineType::Mistralrs,
            Arc::new(crate::engine::mistralrs::MistralRsEngine::new()),
        );

        let engines = Arc::new(engines);
        let processes = Arc::new(RwLock::new(HashMap::new()));

        // Create supervisor
        let mut supervisor = Supervisor::new(config.global.clone());

        // Start supervisor if auto-restart is enabled
        if config.global.auto_restart {
            let instance_configs = Arc::new(config.instances.clone());
            supervisor.start(
                Arc::clone(&processes),
                Arc::clone(&engines),
                instance_configs,
            );
            tracing::info!("Supervisor started with auto-restart enabled");
        }

        Ok(Self {
            config,
            engines,
            processes,
            supervisor: Some(supervisor),
        })
    }

    /// Start an engine instance
    pub async fn start(&mut self, instance_id: &str) -> Result<EngineHandle> {
        // Find instance config
        let instance_config = self
            .config
            .instances
            .iter()
            .find(|i| i.id == instance_id)
            .ok_or_else(|| RuntimeError::InstanceNotFound(instance_id.to_string()))?;

        // Check if already running
        {
            let processes = self.processes.read().await;
            if processes.contains_key(instance_id) {
                return Err(RuntimeError::InstanceAlreadyExists(
                    instance_id.to_string(),
                ));
            }
        }

        // Get engine implementation
        let engine = self
            .engines
            .get(&instance_config.engine)
            .ok_or_else(|| {
                RuntimeError::EngineNotFound(instance_config.engine.to_string())
            })?
            .clone();

        // Start the process
        let process = engine.start(instance_config).await?;
        let handle = EngineHandle::from_process(&process);

        // Store in registry
        {
            let mut processes = self.processes.write().await;
            processes.insert(instance_id.to_string(), process);
        }

        Ok(handle)
    }

    /// Start an engine instance with an explicit binary path
    /// This is the primary method when the server manages binaries
    pub async fn start_with_binary(
        &mut self,
        instance_id: String,
        binary_path: PathBuf,
        config: InstanceConfig,
    ) -> Result<EngineHandle> {
        // Check if already running
        {
            let processes = self.processes.read().await;
            if processes.contains_key(&instance_id) {
                return Err(RuntimeError::InstanceAlreadyExists(instance_id));
            }
        }

        // Get engine implementation
        let engine = self
            .engines
            .get(&config.engine)
            .ok_or_else(|| RuntimeError::EngineNotFound(config.engine.to_string()))?
            .clone();

        // Start the process with explicit binary
        let process = engine.start_with_binary(&config, binary_path).await?;
        let handle = EngineHandle::from_process(&process);

        // Store in registry
        {
            let mut processes = self.processes.write().await;
            processes.insert(instance_id, process);
        }

        Ok(handle)
    }

    /// Restart an instance with a new binary (for engine updates)
    /// This performs a graceful restart: stop old → start new → health check
    pub async fn restart_with_binary(
        &mut self,
        instance_id: &str,
        new_binary_path: PathBuf,
    ) -> Result<EngineHandle> {
        // Get the instance config from the old process
        let config = {
            let processes = self.processes.read().await;
            let process = processes
                .get(instance_id)
                .ok_or_else(|| RuntimeError::InstanceNotFound(instance_id.to_string()))?;

            // Find config for this instance
            self.config
                .instances
                .iter()
                .find(|i| i.id == process.instance_id)
                .ok_or_else(|| RuntimeError::InstanceNotFound(instance_id.to_string()))?
                .clone()
        };

        // Stop the old instance
        self.stop(instance_id).await?;

        // Start new instance with new binary
        self.start_with_binary(instance_id.to_string(), new_binary_path, config)
            .await
    }

    /// Stop an engine instance
    pub async fn stop(&mut self, instance_id: &str) -> Result<()> {
        // Remove from registry
        let mut process = {
            let mut processes = self.processes.write().await;
            processes
                .remove(instance_id)
                .ok_or_else(|| RuntimeError::InstanceNotFound(instance_id.to_string()))?
        };

        // Get engine implementation
        let engine_type = self
            .config
            .instances
            .iter()
            .find(|i| i.id == instance_id)
            .map(|i| i.engine)
            .ok_or_else(|| RuntimeError::InstanceNotFound(instance_id.to_string()))?;

        let engine = self
            .engines
            .get(&engine_type)
            .ok_or_else(|| RuntimeError::EngineNotFound(engine_type.to_string()))?;

        // Stop the process
        engine.stop(&mut process).await?;

        Ok(())
    }

    /// Check health of an engine instance
    pub async fn health_check(&self, instance_id: &str) -> Result<HealthStatus> {
        let processes = self.processes.read().await;
        let process = processes
            .get(instance_id)
            .ok_or_else(|| RuntimeError::InstanceNotFound(instance_id.to_string()))?;

        // Get engine implementation
        let engine_type = self
            .config
            .instances
            .iter()
            .find(|i| i.id == instance_id)
            .map(|i| i.engine)
            .ok_or_else(|| RuntimeError::InstanceNotFound(instance_id.to_string()))?;

        let engine = self
            .engines
            .get(&engine_type)
            .ok_or_else(|| RuntimeError::EngineNotFound(engine_type.to_string()))?;

        engine.health_check(process).await
    }

    /// List all running instances
    pub async fn list_instances(&self) -> Vec<InstanceInfo> {
        let processes = self.processes.read().await;
        let mut instances = Vec::new();

        for process in processes.values() {
            // Get engine for health check
            if let Some(config) = self
                .config
                .instances
                .iter()
                .find(|i| i.id == process.instance_id)
            {
                if let Some(engine) = self.engines.get(&config.engine) {
                    let health = engine
                        .health_check(process)
                        .await
                        .unwrap_or(HealthStatus::Unhealthy(
                            "Health check failed".to_string(),
                        ));

                    instances.push(InstanceInfo::from_process(process, health));
                }
            }
        }

        instances
    }

    /// Shutdown all engine instances
    pub async fn shutdown(&mut self) -> Result<()> {
        // Stop supervisor first
        if let Some(supervisor) = self.supervisor.as_mut() {
            tracing::info!("Stopping supervisor");
            supervisor.stop().await;
        }

        let instance_ids: Vec<String> = {
            let processes = self.processes.read().await;
            processes.keys().cloned().collect()
        };

        for instance_id in instance_ids {
            if let Err(e) = self.stop(&instance_id).await {
                tracing::error!("Failed to stop instance {}: {}", instance_id, e);
            }
        }

        Ok(())
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        tracing::debug!("Runtime being dropped");
    }
}
