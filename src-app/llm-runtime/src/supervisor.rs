//! Process supervisor for auto-restart and health monitoring

use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio::task::JoinHandle;

use crate::config::{GlobalSettings, InstanceConfig};
use crate::engine::{Engine, EngineProcess, HealthStatus};

/// Supervisor for monitoring and restarting engine processes
pub struct Supervisor {
    /// Global settings
    settings: GlobalSettings,

    /// Shutdown signal sender
    shutdown_tx: Option<mpsc::Sender<()>>,

    /// Background task handle
    task_handle: Option<JoinHandle<()>>,
}

impl Supervisor {
    /// Create a new supervisor
    pub fn new(settings: GlobalSettings) -> Self {
        Self {
            settings,
            shutdown_tx: None,
            task_handle: None,
        }
    }

    /// Start the supervisor
    pub fn start(
        &mut self,
        processes: Arc<RwLock<std::collections::HashMap<String, EngineProcess>>>,
        engines: Arc<std::collections::HashMap<crate::config::EngineType, Arc<dyn Engine>>>,
        instance_configs: Arc<Vec<InstanceConfig>>,
    ) {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        let settings = self.settings.clone();

        let task_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(settings.health_check_interval());

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        Self::health_check_cycle(
                            &processes,
                            &engines,
                            &instance_configs,
                            &settings,
                        ).await;
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::info!("Supervisor shutting down");
                        break;
                    }
                }
            }
        });

        self.task_handle = Some(task_handle);
    }

    /// Perform one health check cycle
    async fn health_check_cycle(
        processes: &Arc<RwLock<std::collections::HashMap<String, EngineProcess>>>,
        engines: &Arc<std::collections::HashMap<crate::config::EngineType, Arc<dyn Engine>>>,
        instance_configs: &Arc<Vec<InstanceConfig>>,
        settings: &GlobalSettings,
    ) {
        let processes_read = processes.read().await;

        for (instance_id, process) in processes_read.iter() {
            // Find the instance config
            let config = match instance_configs.iter().find(|c| c.id == *instance_id) {
                Some(c) => c,
                None => continue,
            };

            // Get the engine
            let engine = match engines.get(&config.engine) {
                Some(e) => e,
                None => continue,
            };

            // Check health
            match engine.health_check(process).await {
                Ok(HealthStatus::Healthy) => {
                    // All good
                    tracing::debug!("Instance {} is healthy", instance_id);
                }
                Ok(HealthStatus::Crashed) => {
                    tracing::error!("Instance {} has crashed", instance_id);

                    if settings.auto_restart && process.restart_count < settings.max_restart_attempts {
                        let instance_id_owned = instance_id.clone();
                        let engine_clone = engine.clone();
                        let config_clone = config.clone();

                        tracing::info!(
                            "Attempting to restart instance {} (attempt {}/{})",
                            instance_id_owned,
                            process.restart_count + 1,
                            settings.max_restart_attempts
                        );

                        drop(processes_read);

                        Self::restart_instance(
                            &instance_id_owned,
                            processes,
                            engine_clone,
                            &config_clone,
                        ).await;

                        return; // Exit early since we modified the map
                    } else {
                        tracing::error!(
                            "Instance {} will not be restarted (restart_count={}, max={})",
                            instance_id,
                            process.restart_count,
                            settings.max_restart_attempts
                        );
                    }
                }
                Ok(HealthStatus::Unhealthy(reason)) => {
                    tracing::warn!("Instance {} is unhealthy: {}", instance_id, reason);
                }
                Ok(HealthStatus::Starting) => {
                    tracing::debug!("Instance {} is starting", instance_id);
                }
                Err(e) => {
                    tracing::error!("Health check failed for {}: {}", instance_id, e);
                }
            }
        }
    }

    /// Restart a crashed instance
    async fn restart_instance(
        instance_id: &str,
        processes: &Arc<RwLock<std::collections::HashMap<String, EngineProcess>>>,
        engine: Arc<dyn Engine>,
        config: &InstanceConfig,
    ) {
        // Remove the crashed process
        let old_process = {
            let mut processes_write = processes.write().await;
            processes_write.remove(instance_id)
        };

        if old_process.is_none() {
            tracing::error!("Process {} not found in registry", instance_id);
            return;
        }

        let restart_count = old_process.unwrap().restart_count;

        // Try to start a new process
        match engine.start(config).await {
            Ok(mut new_process) => {
                new_process.restart_count = restart_count + 1;

                tracing::info!(
                    "Successfully restarted instance {} (PID: {})",
                    instance_id,
                    new_process.pid
                );

                let mut processes_write = processes.write().await;
                processes_write.insert(instance_id.to_string(), new_process);
            }
            Err(e) => {
                tracing::error!("Failed to restart instance {}: {}", instance_id, e);
            }
        }
    }

    /// Stop the supervisor
    pub async fn stop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(()).await;
        }

        if let Some(task_handle) = self.task_handle.take() {
            let _ = task_handle.await;
        }
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        // Send shutdown signal if still active
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            // Can't await in Drop, so just try to send
            let _ = shutdown_tx.try_send(());
        }

        tracing::debug!("Supervisor dropped");
    }
}
