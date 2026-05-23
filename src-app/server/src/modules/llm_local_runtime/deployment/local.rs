// Local deployment strategy (same server as chat backend)

use super::{Deployment, DeploymentResult, InstanceStatus};
use crate::common::AppError;
use crate::modules::llm_local_runtime::BinaryManager;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

type AppResult<T> = Result<T, AppError>;
use sqlx::types::Uuid;

#[derive(Debug)]
struct ProcessInfo {
    child: Child,
    port: i32,
    base_url: String,
    started_at: std::time::Instant,
    logs: Vec<String>,
}

pub struct LocalDeployment {
    processes: Arc<RwLock<HashMap<Uuid, ProcessInfo>>>,
    binary_manager: Arc<BinaryManager>,
}

impl LocalDeployment {
    pub fn new(binary_manager: Arc<BinaryManager>) -> Self {
        Self {
            processes: Arc::new(RwLock::new(HashMap::new())),
            binary_manager,
        }
    }

    /// Find an available port
    async fn find_available_port() -> AppResult<i32> {
        portpicker::pick_unused_port()
            .map(|p| p as i32)
            .ok_or_else(|| AppError::internal_error("No available ports"))
    }

    /// Apply common security hardening to a spawned engine command:
    ///   - env_clear + minimal whitelisted env (PATH, HOME, LANG, TZ)
    ///   - stdin null (no inherited stdin)
    ///   - stdout/stderr piped (so we can capture)
    ///
    /// Without env_clear, the spawned engine inherits the server's full
    /// environment including DATABASE_URL, JWT_SECRET, upstream-provider
    /// API keys, OAuth secrets, and the HuggingFace token. A compromised
    /// engine binary OR an attacker who exfiltrates env via the engine's
    /// own diagnostics endpoint can then read all of them. Closes
    /// 08-llm-local-runtime F-03 (High).
    fn apply_hardening(cmd: &mut Command) {
        cmd.env_clear();
        // Preserve only the variables the engine genuinely needs to find
        // shared libraries and respect locale / timezone.
        for var in &["PATH", "HOME", "LANG", "LC_ALL", "TZ", "CUDA_VISIBLE_DEVICES"] {
            if let Ok(val) = std::env::var(var) {
                cmd.env(var, val);
            }
        }
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
    }

    /// Build command for llama.cpp engine
    fn build_llamacpp_command(
        binary_path: &str,
        model_path: &str,
        port: i32,
        config: &serde_json::Value,
    ) -> Command {
        let mut cmd = Command::new(binary_path);
        cmd.arg("--model").arg(model_path);
        cmd.arg("--port").arg(port.to_string());
        cmd.arg("--host").arg("127.0.0.1");

        // Add context size if specified
        if let Some(ctx_size) = config.get("context_size").and_then(|v| v.as_i64()) {
            cmd.arg("--ctx-size").arg(ctx_size.to_string());
        }

        // Add number of GPU layers if specified
        if let Some(n_gpu_layers) = config.get("n_gpu_layers").and_then(|v| v.as_i64()) {
            cmd.arg("--n-gpu-layers").arg(n_gpu_layers.to_string());
        }

        Self::apply_hardening(&mut cmd);

        cmd
    }

    /// Build command for mistral.rs engine
    fn build_mistralrs_command(
        binary_path: &str,
        model_path: &str,
        port: i32,
        config: &serde_json::Value,
    ) -> Command {
        let mut cmd = Command::new(binary_path);
        cmd.arg("--model-path").arg(model_path);
        cmd.arg("--port").arg(port.to_string());
        cmd.arg("--host").arg("127.0.0.1");

        // Add model type if specified
        if let Some(model_type) = config.get("model_type").and_then(|v| v.as_str()) {
            cmd.arg("--model-type").arg(model_type);
        }

        Self::apply_hardening(&mut cmd);

        cmd
    }

    /// Capture logs from process output
    async fn capture_logs(
        model_id: Uuid,
        child: &mut Child,
        processes: Arc<RwLock<HashMap<Uuid, ProcessInfo>>>,
    ) {
        if let Some(stdout) = child.stdout.take() {
            let processes_clone = processes.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let mut procs = processes_clone.write().await;
                    if let Some(proc_info) = procs.get_mut(&model_id) {
                        proc_info.logs.push(line);
                        // Keep only last 1000 lines
                        if proc_info.logs.len() > 1000 {
                            proc_info.logs.remove(0);
                        }
                    }
                }
            });
        }

        if let Some(stderr) = child.stderr.take() {
            let processes_clone = processes.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let mut procs = processes_clone.write().await;
                    if let Some(proc_info) = procs.get_mut(&model_id) {
                        proc_info.logs.push(format!("[stderr] {}", line));
                        // Keep only last 1000 lines
                        if proc_info.logs.len() > 1000 {
                            proc_info.logs.remove(0);
                        }
                    }
                }
            });
        }
    }
}

#[async_trait::async_trait]
impl Deployment for LocalDeployment {
    async fn start(
        &self,
        model_id: Uuid,
        engine_type: &str,
        model_path: &str,
        config: &serde_json::Value,
    ) -> AppResult<DeploymentResult> {
        // Check if already running
        {
            let processes = self.processes.read().await;
            if processes.contains_key(&model_id) {
                return Err(AppError::conflict("Model instance already running"));
            }
        }

        // Find available port
        let port = Self::find_available_port().await?;
        let base_url = format!("http://127.0.0.1:{}", port);

        // Normalize engine type
        let normalized_engine = match engine_type.to_lowercase().as_str() {
            "llamacpp" | "llama.cpp" => "llamacpp",
            "mistralrs" | "mistral.rs" => "mistralrs",
            _ => {
                return Err(AppError::bad_request(
                    "UNSUPPORTED_ENGINE",
                    &format!("Unsupported engine type: {}", engine_type),
                ))
            }
        };

        // Resolve binary version: try system default, fall back to latest
        let runtime_version = self
            .binary_manager
            .get_system_default(normalized_engine)
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to query system default: {}", e)))?
            .or_else(|| {
                // Fallback: try to get latest version (blocking)
                let binary_manager = self.binary_manager.clone();
                let engine = normalized_engine.to_string();
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        binary_manager.get_latest_version(&engine).await.ok().flatten()
                    })
                })
            })
            .ok_or_else(|| {
                AppError::internal_error(&format!(
                    "No runtime version available for engine '{}'. Please download a version first.",
                    normalized_engine
                ))
            })?;

        // Get binary path
        let binary_path = self
            .binary_manager
            .get_binary_path(runtime_version.id)
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to get binary path: {}", e)))?;

        tracing::info!(
            "Using runtime version: {} {} ({})",
            runtime_version.engine,
            runtime_version.version,
            runtime_version.id
        );

        // Build command based on engine type
        let mut cmd = match normalized_engine {
            "llamacpp" => Self::build_llamacpp_command(&binary_path.to_string_lossy(), model_path, port, config),
            "mistralrs" => Self::build_mistralrs_command(&binary_path.to_string_lossy(), model_path, port, config),
            _ => unreachable!(), // Already validated above
        };

        // Spawn the process
        let mut child = cmd.spawn().map_err(|e| {
            AppError::internal_error(&format!("Failed to spawn process: {}", e))
        })?;

        let pid = child
            .id()
            .ok_or_else(|| AppError::internal_error("Failed to get process ID"))?
            as i32;

        // Start log capture
        Self::capture_logs(model_id, &mut child, self.processes.clone()).await;

        // Store process info
        let proc_info = ProcessInfo {
            child,
            port,
            base_url: base_url.clone(),
            started_at: std::time::Instant::now(),
            logs: Vec::new(),
        };

        {
            let mut processes = self.processes.write().await;
            processes.insert(model_id, proc_info);
        }

        Ok(DeploymentResult {
            pid,
            port,
            base_url,
        })
    }

    async fn stop(&self, model_id: Uuid) -> AppResult<()> {
        let mut processes = self.processes.write().await;

        let mut proc_info = processes
            .remove(&model_id)
            .ok_or_else(|| AppError::not_found("Process not found"))?;

        // Try graceful shutdown first
        if let Err(e) = proc_info.child.kill().await {
            tracing::warn!("Failed to kill process for model {}: {}", model_id, e);
        }

        // Wait for process to exit (with timeout)
        match tokio::time::timeout(
            std::time::Duration::from_secs(10),
            proc_info.child.wait(),
        )
        .await
        {
            Ok(Ok(_)) => {
                tracing::info!("Process for model {} stopped gracefully", model_id);
            }
            Ok(Err(e)) => {
                tracing::warn!("Error waiting for process {}: {}", model_id, e);
            }
            Err(_) => {
                tracing::warn!("Process {} did not stop within timeout", model_id);
            }
        }

        Ok(())
    }

    async fn status(&self, model_id: Uuid) -> AppResult<InstanceStatus> {
        let processes = self.processes.read().await;

        if let Some(proc_info) = processes.get(&model_id) {
            let uptime = proc_info.started_at.elapsed().as_secs() as i64;

            // Try to get actual PID (may have changed or process may have died)
            let pid = proc_info.child.id().map(|id| id as i32);

            Ok(InstanceStatus {
                running: pid.is_some(),
                pid,
                port: Some(proc_info.port),
                uptime_seconds: Some(uptime),
            })
        } else {
            Ok(InstanceStatus {
                running: false,
                pid: None,
                port: None,
                uptime_seconds: None,
            })
        }
    }

    async fn health_check(&self, base_url: &str) -> AppResult<bool> {
        // Try to make a health check request to the server
        let health_url = format!("{}/health", base_url);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| AppError::internal_error(&format!("Failed to create HTTP client: {}", e)))?;

        match client.get(&health_url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => {
                // Try root endpoint as fallback
                match client.get(base_url).send().await {
                    Ok(response) => Ok(response.status().is_success()),
                    Err(_) => Ok(false),
                }
            }
        }
    }

    async fn get_logs(&self, model_id: Uuid, lines: usize) -> AppResult<Vec<String>> {
        let processes = self.processes.read().await;

        if let Some(proc_info) = processes.get(&model_id) {
            let total_lines = proc_info.logs.len();
            let start_index = if total_lines > lines {
                total_lines - lines
            } else {
                0
            };

            Ok(proc_info.logs[start_index..].to_vec())
        } else {
            Err(AppError::not_found("Process not found"))
        }
    }
}
