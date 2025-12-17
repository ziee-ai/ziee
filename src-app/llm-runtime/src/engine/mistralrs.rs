//! MistralRS engine implementation

use async_trait::async_trait;
use std::fs::OpenOptions;
use std::process::{Command, Stdio};
use std::time::Duration;

use super::{Engine, EngineProcess, HealthStatus};
use crate::binary::{ensure_executable, get_engine_binary_path};
use crate::config::{DeviceType, EngineType, InstanceConfig};
use crate::error::{Result, RuntimeError};

/// MistralRS engine implementation
pub struct MistralRsEngine;

impl MistralRsEngine {
    pub fn new() -> Self {
        Self
    }

    /// Get an available port
    fn get_available_port() -> Result<u16> {
        portpicker::pick_unused_port()
            .ok_or_else(|| RuntimeError::PortUnavailable("No available ports".to_string()))
    }

    /// Build command-line arguments for mistralrs-server
    fn build_command_args(&self, config: &InstanceConfig, port: u16) -> Vec<String> {
        let mut args = Vec::new();
        let settings = &config.settings.mistralrs;

        // Port configuration
        args.extend(["--port".to_string(), port.to_string()]);

        // Max sequences
        args.extend(["--max-seqs".to_string(), settings.max_seqs.to_string()]);

        // Prefix cache
        args.extend([
            "--prefix-cache-n".to_string(),
            settings.prefix_cache_n.to_string(),
        ]);

        // Device configuration
        let is_cpu = matches!(config.device, DeviceType::Cpu);
        if is_cpu {
            args.push("--cpu".to_string());
        }

        // Model format subcommand (gguf or plain)
        match settings.model_format.as_str() {
            "gguf" => {
                args.push("gguf".to_string());
                args.extend([
                    "--quantized-model-id".to_string(),
                    config.model_path.to_string_lossy().to_string(),
                ]);

                // Try to use the file name as the quantized filename
                if let Some(filename) = config.model_path.file_name() {
                    args.extend([
                        "--quantized-filename".to_string(),
                        filename.to_string_lossy().to_string(),
                    ]);
                } else {
                    // Fallback to wildcard
                    args.extend(["--quantized-filename".to_string(), "*.gguf".to_string()]);
                }

                // dtype for GGUF
                args.extend(["--dtype".to_string(), settings.dtype.clone()]);
            }
            _ => {
                // Default to "plain" for safetensors/pytorch models
                args.push("plain".to_string());
                args.extend([
                    "--model-id".to_string(),
                    config.model_path.to_string_lossy().to_string(),
                ]);

                // dtype for plain models
                args.extend(["--dtype".to_string(), settings.dtype.clone()]);
            }
        }

        args
    }

    /// Wait for the server to be healthy
    async fn wait_for_health(&self, base_url: &str, timeout: Duration) -> Result<()> {
        let start = std::time::Instant::now();
        let health_url = format!("{}/health", base_url);
        let models_url = format!("{}/v1/models", base_url);

        while start.elapsed() < timeout {
            // Check health endpoint
            if let Ok(response) = reqwest::get(&health_url).await {
                if response.status().is_success() {
                    // Health check passed, now verify models endpoint
                    if let Ok(models_response) = reqwest::get(&models_url).await {
                        if models_response.status().is_success() {
                            tracing::info!("MistralRS server is healthy and ready");
                            return Ok(());
                        }
                    }
                }
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        Err(RuntimeError::health_check_failed(
            "MistralRS server health check timeout",
        ))
    }
}

#[async_trait]
impl Engine for MistralRsEngine {
    fn name(&self) -> &'static str {
        "mistralrs"
    }

    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    async fn start(&self, config: &InstanceConfig) -> Result<EngineProcess> {
        // Use auto-discovery to find binary
        let binary_path = get_engine_binary_path(EngineType::Mistralrs)?;
        self.start_with_binary(config, binary_path).await
    }

    async fn start_with_binary(
        &self,
        config: &InstanceConfig,
        binary_path: std::path::PathBuf,
    ) -> Result<EngineProcess> {
        ensure_executable(&binary_path)?;

        // Get available port
        let port = Self::get_available_port()?;

        // Build command arguments
        let args = self.build_command_args(config, port);

        // Setup logging
        let log_dir = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("logs");
        std::fs::create_dir_all(&log_dir).ok();

        let log_file_path = log_dir.join(format!("{}_engine.log", config.id));

        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)
            .map_err(|e| RuntimeError::internal(format!("Failed to open log file: {}", e)))?;

        tracing::info!(
            "Starting MistralRS instance '{}' on port {} (log: {})",
            config.id,
            port,
            log_file_path.display()
        );
        tracing::debug!("Command: {} {:?}", binary_path.display(), args);

        // Spawn the process
        let child = Command::new(&binary_path)
            .args(&args)
            .stdout(Stdio::from(log_file.try_clone().map_err(|e| {
                RuntimeError::internal(format!("Failed to clone log file handle: {}", e))
            })?))
            .stderr(Stdio::from(log_file))
            .spawn()
            .map_err(|e| RuntimeError::startup_failed(format!("Failed to spawn process: {}", e)))?;

        let pid = child.id();
        tracing::info!(
            "MistralRS process spawned (PID: {}, instance: {})",
            pid,
            config.id
        );

        // Create process handle
        let process = EngineProcess::new(config.id.clone(), port, child);

        // Wait for health check (longer timeout for model loading)
        let timeout = Duration::from_secs(300); // 5 minutes default timeout
        self.wait_for_health(&process.base_url(), timeout)
            .await?;

        tracing::info!("MistralRS instance '{}' is healthy and ready", config.id);

        Ok(process)
    }

    async fn stop(&self, process: &mut EngineProcess) -> Result<()> {
        tracing::info!(
            "Stopping MistralRS instance '{}' (PID: {})",
            process.instance_id,
            process.pid
        );

        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;

            let pid = Pid::from_raw(process.pid as i32);

            // Send SIGTERM for graceful shutdown
            kill(pid, Signal::SIGTERM).map_err(|e| {
                RuntimeError::shutdown_failed(format!("Failed to send SIGTERM: {}", e))
            })?;

            // Wait up to 3 seconds for graceful shutdown
            let start = std::time::Instant::now();
            let grace_period = Duration::from_secs(3);

            while start.elapsed() < grace_period {
                if let Ok(Some(_)) = process.child.try_wait() {
                    tracing::info!("MistralRS instance '{}' stopped gracefully", process.instance_id);
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(100));
            }

            // Force kill if not stopped
            tracing::warn!(
                "MistralRS instance '{}' did not stop gracefully, sending SIGKILL",
                process.instance_id
            );
            kill(pid, Signal::SIGKILL).map_err(|e| {
                RuntimeError::shutdown_failed(format!("Failed to send SIGKILL: {}", e))
            })?;
        }

        #[cfg(windows)]
        {
            use std::process::Command;

            Command::new("taskkill")
                .args(&["/F", "/PID", &process.pid.to_string()])
                .output()
                .map_err(|e| {
                    RuntimeError::shutdown_failed(format!("Failed to terminate process: {}", e))
                })?;
        }

        // Wait for process to exit
        process.child.wait().map_err(|e| {
            RuntimeError::shutdown_failed(format!("Failed to wait for process: {}", e))
        })?;

        tracing::info!("MistralRS instance '{}' stopped", process.instance_id);

        Ok(())
    }

    async fn health_check(&self, process: &EngineProcess) -> Result<HealthStatus> {
        // Check if process is still running
        if !process.is_running() {
            return Ok(HealthStatus::Crashed);
        }

        // Quick health check
        let health_url = format!("{}/health", process.base_url());
        let timeout = Duration::from_secs(5);

        match tokio::time::timeout(timeout, reqwest::get(&health_url)).await {
            Ok(Ok(response)) => {
                if response.status().is_success() {
                    Ok(HealthStatus::Healthy)
                } else {
                    Ok(HealthStatus::Unhealthy(format!(
                        "HTTP {}",
                        response.status()
                    )))
                }
            }
            Ok(Err(e)) => Ok(HealthStatus::Unhealthy(format!("Request failed: {}", e))),
            Err(_) => Ok(HealthStatus::Unhealthy("Timeout".to_string())),
        }
    }
}
