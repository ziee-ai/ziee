//! LlamaCpp engine implementation

use async_trait::async_trait;
use std::fs::OpenOptions;
use std::process::{Command, Stdio};
use std::time::Duration;

use super::{Engine, EngineProcess, HealthStatus};
use crate::binary::{ensure_executable, get_engine_binary_path};
use crate::config::{DeviceType, EngineType, InstanceConfig};
use crate::error::{Result, RuntimeError};

/// LlamaCpp engine implementation
pub struct LlamaCppEngine;

impl LlamaCppEngine {
    pub fn new() -> Self {
        Self
    }

    /// Get an available port
    fn get_available_port() -> Result<u16> {
        portpicker::pick_unused_port()
            .ok_or_else(|| RuntimeError::PortUnavailable("No available ports".to_string()))
    }

    /// Build command-line arguments for llama-server
    fn build_command_args(&self, config: &InstanceConfig, port: u16) -> Vec<String> {
        let mut args = Vec::new();

        // Port and host
        args.push("--port".to_string());
        args.push(port.to_string());
        args.push("--host".to_string());
        args.push("127.0.0.1".to_string());

        // Model path
        args.push("--model".to_string());
        args.push(config.model_path.to_string_lossy().to_string());

        // Context size
        args.push("--ctx-size".to_string());
        args.push(config.settings.llamacpp.ctx_size.to_string());

        // Batch size
        args.push("--batch-size".to_string());
        args.push(config.settings.llamacpp.batch_size.to_string());

        // Device configuration
        match config.device {
            DeviceType::Cuda => {
                args.push("--device".to_string());
                args.push("cuda".to_string());
                args.push("--n-gpu-layers".to_string());
                args.push(config.settings.llamacpp.n_gpu_layers.to_string());
            }
            DeviceType::Metal => {
                args.push("--device".to_string());
                args.push("metal".to_string());
                args.push("--n-gpu-layers".to_string());
                args.push(config.settings.llamacpp.n_gpu_layers.to_string());
            }
            DeviceType::Rocm => {
                args.push("--device".to_string());
                args.push("rocm".to_string());
                args.push("--n-gpu-layers".to_string());
                args.push(config.settings.llamacpp.n_gpu_layers.to_string());
            }
            DeviceType::Vulkan => {
                args.push("--device".to_string());
                args.push("vulkan".to_string());
                args.push("--n-gpu-layers".to_string());
                args.push(config.settings.llamacpp.n_gpu_layers.to_string());
            }
            DeviceType::Opencl => {
                args.push("--device".to_string());
                args.push("opencl".to_string());
            }
            DeviceType::Cpu => {
                // CPU-only, no GPU layers
                args.push("--n-gpu-layers".to_string());
                args.push("0".to_string());
            }
        }

        // Threads (if specified)
        if let Some(threads) = config.settings.llamacpp.threads {
            args.push("--threads".to_string());
            args.push(threads.to_string());
        }

        // RoPE scaling
        if let Some(rope_freq_base) = config.settings.llamacpp.rope_freq_base {
            args.push("--rope-freq-base".to_string());
            args.push(rope_freq_base.to_string());
        }

        if let Some(rope_freq_scale) = config.settings.llamacpp.rope_freq_scale {
            args.push("--rope-freq-scale".to_string());
            args.push(rope_freq_scale.to_string());
        }

        // Embeddings
        if config.settings.llamacpp.embeddings {
            args.push("--embeddings".to_string());
        }

        args
    }

    /// Wait for the engine to become healthy
    async fn wait_for_health(
        &self,
        base_url: &str,
        timeout: Duration,
    ) -> Result<()> {
        let start = std::time::Instant::now();
        let health_url = format!("{}/health", base_url);

        while start.elapsed() < timeout {
            // Try to connect
            match reqwest::get(&health_url).await {
                Ok(response) => {
                    if response.status().is_success() {
                        // Check if we get valid JSON response
                        if let Ok(json) = response.json::<serde_json::Value>().await {
                            if json.get("status").and_then(|v| v.as_str()) == Some("ok") {
                                tracing::info!("Engine is healthy");
                                return Ok(());
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("Health check failed: {}", e);
                }
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        Err(RuntimeError::health_check_failed(format!(
            "Engine did not become healthy within {} seconds",
            timeout.as_secs()
        )))
    }
}

impl Default for LlamaCppEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Engine for LlamaCppEngine {
    fn name(&self) -> &'static str {
        "llamacpp"
    }

    fn version(&self) -> String {
        // TODO: Query actual version from binary
        env!("CARGO_PKG_VERSION").to_string()
    }

    async fn start(&self, config: &InstanceConfig) -> Result<EngineProcess> {
        // Use auto-discovery to find binary
        let binary_path = get_engine_binary_path(EngineType::Llamacpp)?;
        self.start_with_binary(config, binary_path).await
    }

    async fn start_with_binary(
        &self,
        config: &InstanceConfig,
        binary_path: std::path::PathBuf,
    ) -> Result<EngineProcess> {
        tracing::info!("Starting LlamaCpp engine for instance: {}", config.id);

        ensure_executable(&binary_path)?;

        tracing::debug!("Using binary: {}", binary_path.display());

        // Get port
        let port = if let Some(p) = config.settings.port {
            p
        } else {
            Self::get_available_port()?
        };

        tracing::info!("Assigned port: {}", port);

        // Build command arguments
        let args = self.build_command_args(config, port);

        tracing::debug!("Command args: {:?}", args);

        // Prepare log file
        let log_dir = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("logs");
        std::fs::create_dir_all(&log_dir).ok();

        let log_file_path = log_dir.join(format!("{}_engine.log", config.id));
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)
            .map_err(|e| {
                RuntimeError::startup_failed(format!("Failed to open log file: {}", e))
            })?;

        tracing::info!("Logging to: {}", log_file_path.display());

        // Spawn process
        let child = Command::new(&binary_path)
            .args(&args)
            .stdout(Stdio::from(log_file.try_clone().map_err(|e| {
                RuntimeError::startup_failed(format!("Failed to clone log file: {}", e))
            })?))
            .stderr(Stdio::from(log_file))
            .spawn()
            .map_err(|e| {
                RuntimeError::startup_failed(format!("Failed to spawn process: {}", e))
            })?;

        tracing::info!("Process spawned with PID: {}", child.id());

        // Create process handle
        let process = EngineProcess::new(config.id.clone(), port, child);

        // Wait for engine to be ready
        let timeout = Duration::from_secs(300); // 5 minutes default timeout
        self.wait_for_health(&process.base_url(), timeout)
            .await?;

        tracing::info!("Engine started successfully: {}", config.id);

        Ok(process)
    }

    async fn stop(&self, process: &mut EngineProcess) -> Result<()> {
        tracing::info!("Stopping engine process: {}", process.instance_id);

        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;

            let pid = Pid::from_raw(process.pid as i32);

            // Try graceful shutdown with SIGTERM
            tracing::debug!("Sending SIGTERM to PID {}", process.pid);
            if let Err(e) = kill(pid, Signal::SIGTERM) {
                tracing::warn!("Failed to send SIGTERM: {}", e);
            }

            // Wait up to 3 seconds for graceful shutdown
            let timeout = tokio::time::sleep(Duration::from_secs(3));
            tokio::pin!(timeout);

            loop {
                tokio::select! {
                    _ = &mut timeout => {
                        // Timeout reached, force kill
                        tracing::warn!("Process did not stop gracefully, sending SIGKILL");
                        if let Err(e) = kill(pid, Signal::SIGKILL) {
                            tracing::error!("Failed to send SIGKILL: {}", e);
                        }
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        // Check if process is still running
                        if kill(pid, Signal::SIGURG).is_err() {
                            // Process is gone
                            tracing::info!("Process stopped gracefully");
                            break;
                        }
                    }
                }
            }
        }

        #[cfg(windows)]
        {
            // On Windows, use taskkill
            tracing::debug!("Killing process PID {}", process.pid);
            let output = Command::new("taskkill")
                .args(&["/F", "/PID", &process.pid.to_string()])
                .output();

            match output {
                Ok(output) => {
                    if !output.status.success() {
                        tracing::warn!(
                            "taskkill failed: {}",
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to execute taskkill: {}", e);
                }
            }
        }

        // Wait for child process to exit
        if let Err(e) = process.child.wait() {
            tracing::warn!("Failed to wait for child process: {}", e);
        }

        tracing::info!("Engine stopped: {}", process.instance_id);

        Ok(())
    }

    async fn health_check(&self, process: &EngineProcess) -> Result<HealthStatus> {
        // First check if process is still running
        if !process.is_running() {
            return Ok(HealthStatus::Crashed);
        }

        // Check /health endpoint
        let health_url = format!("{}/health", process.base_url());

        match tokio::time::timeout(
            Duration::from_secs(5),
            reqwest::get(&health_url),
        )
        .await
        {
            Ok(Ok(response)) => {
                if response.status().is_success() {
                    // Try to parse JSON
                    match response.json::<serde_json::Value>().await {
                        Ok(json) => {
                            if json.get("status").and_then(|v| v.as_str()) == Some("ok") {
                                Ok(HealthStatus::Healthy)
                            } else {
                                Ok(HealthStatus::Unhealthy(
                                    "Invalid health response".to_string(),
                                ))
                            }
                        }
                        Err(e) => Ok(HealthStatus::Unhealthy(format!(
                            "Failed to parse health response: {}",
                            e
                        ))),
                    }
                } else {
                    Ok(HealthStatus::Unhealthy(format!(
                        "HTTP status: {}",
                        response.status()
                    )))
                }
            }
            Ok(Err(e)) => Ok(HealthStatus::Unhealthy(format!("Request failed: {}", e))),
            Err(_) => Ok(HealthStatus::Unhealthy("Request timed out".to_string())),
        }
    }
}
