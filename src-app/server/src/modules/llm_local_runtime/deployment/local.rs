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

/// Process-global map of model_id → per-instance bearer token. Chat
/// code calls `get_instance_api_key(model_id)` to retrieve the token
/// for outbound calls. Closes 08-llm-local-runtime F-04 (High) at
/// the runtime layer; the chat-side wiring that actually presents
/// the bearer to the local engine is a follow-up.
static INSTANCE_API_KEYS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<Uuid, String>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Return the bearer token assigned to the engine instance for
/// `model_id`, or None if no instance is running.
pub fn get_instance_api_key(model_id: Uuid) -> Option<String> {
    INSTANCE_API_KEYS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(&model_id)
        .cloned()
}

/// Maximum number of log lines retained per engine instance. When the
/// buffer fills, the oldest line is popped (FIFO) in O(1) via
/// VecDeque. The previous Vec::remove(0) was O(n) per push past the
/// cap → O(n²) over the buffer lifetime. Closes 08-llm-local-runtime
/// F-08 (Medium).
const LOG_BUFFER_MAX_LINES: usize = 1000;

/// Maximum bytes per captured log line. Without this, a runaway
/// engine that emits gigabyte-long lines would balloon server memory
/// (each WriteGuard + line allocation). Closes 08-llm-local-runtime
/// F-08's per-line-size sub-finding.
const LOG_LINE_MAX_BYTES: usize = 16 * 1024;

#[derive(Debug)]
struct ProcessInfo {
    child: Child,
    port: i32,
    base_url: String,
    started_at: std::time::Instant,
    logs: std::collections::VecDeque<String>,
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

    /// Validate that a value bound for engine argv is safe. Closes
    /// 08-llm-local-runtime F-02 (High): model.name flows from
    /// admin-uploaded model metadata into `--model VALUE` argv. If
    /// VALUE starts with `-` it could be re-interpreted as another
    /// flag (argument injection); shell metachars (`;`, `&`, `|`,
    /// `\``, `$()`) could enable command injection on engines that
    /// pass through to a shell. We reject either at deploy-time.
    fn validate_argv_value(label: &str, value: &str) -> AppResult<()> {
        if value.is_empty() {
            return Err(AppError::bad_request(
                "INVALID_ARGV",
                format!("{} cannot be empty", label),
            ));
        }
        if value.starts_with('-') {
            return Err(AppError::bad_request(
                "INVALID_ARGV",
                format!(
                    "{} cannot start with '-' (would be parsed as a flag): {:?}",
                    label, value
                ),
            ));
        }
        const BANNED: &[char] = &[';', '&', '|', '`', '$', '\n', '\r', '\0', '<', '>'];
        if value.chars().any(|c| BANNED.contains(&c)) {
            return Err(AppError::bad_request(
                "INVALID_ARGV",
                format!("{} contains shell metacharacters: {:?}", label, value),
            ));
        }
        Ok(())
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

    /// Build command for llama.cpp engine.
    ///
    /// Closes 08-llm-local-runtime F-04 (High) for llama.cpp's
    /// HTTP surface: `--api-key TOKEN` makes the engine require
    /// `Authorization: Bearer TOKEN` on every request. Without this,
    /// any local process (or an SSRF in a co-located service) can
    /// reach 127.0.0.1:port and run inferences against the loaded
    /// model. The chat-side wiring (so authenticated chat requests
    /// actually present this token to the engine) is the follow-up
    /// piece — see INSTANCE_API_KEYS getter exposed for future use.
    fn build_llamacpp_command(
        binary_path: &str,
        model_path: &str,
        port: i32,
        config: &serde_json::Value,
        api_key: &str,
    ) -> Command {
        let mut cmd = Command::new(binary_path);
        cmd.arg("--model").arg(model_path);
        cmd.arg("--port").arg(port.to_string());
        cmd.arg("--host").arg("127.0.0.1");
        cmd.arg("--api-key").arg(api_key);

        // Add context size if specified
        if let Some(ctx_size) = config.get("context_size").and_then(|v| v.as_i64()) {
            cmd.arg("--ctx-size").arg(ctx_size.to_string());
        }

        // Add number of GPU layers if specified
        if let Some(n_gpu_layers) = config.get("n_gpu_layers").and_then(|v| v.as_i64()) {
            cmd.arg("--n-gpu-layers").arg(n_gpu_layers.to_string());
        }

        // Embedding mode for memory module + RAG. llama-server's
        // `--embeddings` flag is mutually exclusive with `--chat-template`
        // — the engine returns 768-d (or model-specific) float vectors
        // on POST `/embedding` instead of streaming chat tokens. Memory
        // dispatcher detects this via `llm_models.capabilities.text_embedding`
        // and sets `config.embeddings = true` before calling start().
        if config
            .get("embeddings")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            cmd.arg("--embeddings");
        }

        Self::apply_hardening(&mut cmd);

        cmd
    }

    /// Build command for mistral.rs engine.
    ///
    /// Note: mistral.rs doesn't expose a `--api-key` flag at time of
    /// writing (verified against v0.x); the api_key parameter is
    /// accepted to match the llama.cpp signature but ignored, with a
    /// warn-once log. Closes 08-llm-local-runtime F-04 (High) for
    /// llama.cpp; mistral.rs requires an upstream feature first.
    fn build_mistralrs_command(
        binary_path: &str,
        model_path: &str,
        port: i32,
        config: &serde_json::Value,
        _api_key: &str,
    ) -> Command {
        tracing::warn!(
            "08-llm-local-runtime F-04: mistral.rs engine has no \
             built-in --api-key flag; the local 127.0.0.1:{} port is \
             reachable from any process. Track upstream feature for \
             bearer-auth support.",
            port
        );

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
                        push_capped(&mut proc_info.logs, line);
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
                        push_capped(&mut proc_info.logs, format!("[stderr] {}", line));
                    }
                }
            });
        }
    }
}

/// Push a log line with both line-size and ring-buffer caps. Closes
/// 08-llm-local-runtime F-08 (Medium).
fn push_capped(buf: &mut std::collections::VecDeque<String>, mut line: String) {
    if line.len() > LOG_LINE_MAX_BYTES {
        // Truncate at a UTF-8 boundary just under the cap.
        let mut end = LOG_LINE_MAX_BYTES;
        while !line.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        line.truncate(end);
        line.push_str("…[truncated]");
    }
    while buf.len() >= LOG_BUFFER_MAX_LINES {
        buf.pop_front();
    }
    buf.push_back(line);
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

        // Validate model_path before it flows into engine argv.
        // Closes 08-llm-local-runtime F-02 (High): model.name (which
        // becomes model_path in handlers.rs) is admin-uploaded and
        // unvalidated; without this check, a name like `--exec ...`
        // is parsed by some engines as an additional flag.
        Self::validate_argv_value("model_path", model_path)?;

        // Concurrent-engine quota. Closes 08-llm-local-runtime F-07
        // (Medium): without this, an admin (or an automated client
        // hitting the start endpoint in a loop) can spin up dozens of
        // local engines and OOM the host. 8 matches a typical
        // workstation's GPU count + the per-model VRAM ceiling.
        // Operators with bigger boxes can raise this via a future
        // config; the hardcoded value below is the safe ceiling.
        const MAX_CONCURRENT_ENGINES: usize = 8;
        {
            let processes = self.processes.read().await;
            if processes.len() >= MAX_CONCURRENT_ENGINES {
                return Err(AppError::bad_request(
                    "TOO_MANY_INSTANCES",
                    format!(
                        "{} engine instances are already running (cap {}); stop one before starting another",
                        processes.len(),
                        MAX_CONCURRENT_ENGINES
                    ),
                ));
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
                    format!("Unsupported engine type: {}", engine_type),
                ))
            }
        };

        // Resolve binary version: try system default, fall back to latest
        let runtime_version = self
            .binary_manager
            .get_system_default(normalized_engine)
            .await
            .map_err(|e| AppError::internal_error(format!("Failed to query system default: {}", e)))?
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
                AppError::internal_error(format!(
                    "No runtime version available for engine '{}'. Please download a version first.",
                    normalized_engine
                ))
            })?;

        // Get binary path
        let binary_path = self
            .binary_manager
            .get_binary_path(runtime_version.id)
            .await
            .map_err(|e| AppError::internal_error(format!("Failed to get binary path: {}", e)))?;

        tracing::info!(
            "Using runtime version: {} {} ({})",
            runtime_version.engine,
            runtime_version.version,
            runtime_version.id
        );

        // Mint a per-instance bearer token. Stored in the
        // process-global INSTANCE_API_KEYS map so chat-side code can
        // look it up via get_instance_api_key(model_id) when
        // dispatching to the local engine. Closes
        // 08-llm-local-runtime F-04 (High) for llama.cpp; chat-side
        // wiring is the follow-up that actually presents the bearer.
        let api_key = uuid::Uuid::new_v4()
            .to_string()
            .replace('-', "");
        INSTANCE_API_KEYS
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(model_id, api_key.clone());

        // Build command based on engine type
        let mut cmd = match normalized_engine {
            "llamacpp" => Self::build_llamacpp_command(&binary_path.to_string_lossy(), model_path, port, config, &api_key),
            "mistralrs" => Self::build_mistralrs_command(&binary_path.to_string_lossy(), model_path, port, config, &api_key),
            _ => unreachable!(), // Already validated above
        };

        // Spawn the process
        let mut child = cmd.spawn().map_err(|e| {
            AppError::internal_error(format!("Failed to spawn process: {}", e))
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
            logs: std::collections::VecDeque::new(),
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
        // Drop the per-instance bearer token. Closes
        // 08-llm-local-runtime F-04 (High) — keeping the token alive
        // past process death would let a future model_id collision
        // accidentally reuse it.
        INSTANCE_API_KEYS
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&model_id);

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
            .map_err(|e| AppError::internal_error(format!("Failed to create HTTP client: {}", e)))?;

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
            let start_index = total_lines.saturating_sub(lines);
            Ok(proc_info.logs.iter().skip(start_index).cloned().collect())
        } else {
            Err(AppError::not_found("Process not found"))
        }
    }
}
