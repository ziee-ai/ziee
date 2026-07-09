//! Local deployment of the SINGLE managed whisper-server instance.
//!
//! Ported from `llm_local_runtime::deployment::local`, scoped to one process
//! (whisper transcribes one model at a time). Spawns
//! `whisper-server --host 127.0.0.1 --port <ephemeral> -m <model.bin> [-l <lang>]`
//! as a HARDENED subprocess:
//!
//!   - `env_clear()` + a minimal PATH/HOME/LANG/… allow-list (so the child never
//!     inherits `DATABASE_URL`, `JWT_SECRET`, provider API keys, …);
//!   - `stdin(null)`, piped stdout/stderr for log capture;
//!   - `kill_on_drop(true)`;
//!   - on Linux, `PR_SET_PDEATHSIG=SIGTERM` via `pre_exec` so the child dies with
//!     the server even on SIGKILL/OOM.
//!
//! Every user/admin-supplied argv value flows through [`validate_argv_value`]
//! (reject leading `-` and shell metacharacters). The health probe (`GET /` →
//! 200) + a loopback-bind verification live here; the long readiness poll (up to
//! `auto_start_timeout_secs`) is driven by `auto_start`.
//!
//! [`validate_argv_value`]: LocalDeployment::validate_argv_value

use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

use crate::common::AppError;

type AppResult<T> = Result<T, AppError>;

/// Maximum number of log lines retained (FIFO ring buffer).
const LOG_BUFFER_MAX_LINES: usize = 1000;
/// Maximum bytes per captured log line (a runaway server emitting gigabyte-long
/// lines must not balloon server memory).
const LOG_LINE_MAX_BYTES: usize = 16 * 1024;

/// Result of a successful spawn — the metadata `auto_start` persists.
#[derive(Debug, Clone)]
pub struct StartOutcome {
    pub pid: i32,
    pub port: u16,
    pub base_url: String,
}

/// Coarse liveness snapshot of the instance.
///
/// `pid`/`uptime_seconds` are populated but not yet surfaced — the admin
/// `GET /voice/instance` reads persisted state from the DB row; process-level
/// liveness introspection is a deferred follow-up (see DRIFT-1).
#[derive(Debug, Clone)]
pub struct InstanceStatus {
    pub running: bool,
    #[allow(dead_code)]
    pub pid: Option<i32>,
    pub port: Option<u16>,
    #[allow(dead_code)]
    pub uptime_seconds: Option<i64>,
}

/// The running process + its captured logs.
#[derive(Debug)]
struct ProcessInfo {
    child: Child,
    port: u16,
    active_model: String,
    started_at: Instant,
    logs: std::collections::VecDeque<String>,
    /// Broadcast channel for live log streaming (SSE tail). `send` is
    /// non-blocking and drops the oldest on overflow, so a slow subscriber
    /// never backpressures capture. Reserved for the deferred live-log SSE
    /// endpoint (see DRIFT-1); capture is wired, the tail endpoint is not.
    #[allow(dead_code)]
    log_broadcast: tokio::sync::broadcast::Sender<String>,
}

/// The single-instance deployment. Holds at most ONE whisper-server.
pub struct LocalDeployment {
    process: Arc<RwLock<Option<ProcessInfo>>>,
}

impl Default for LocalDeployment {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalDeployment {
    pub fn new() -> Self {
        Self {
            process: Arc::new(RwLock::new(None)),
        }
    }

    /// Validate that a value bound for whisper-server argv is safe.
    ///
    /// Mirrors `llm_local_runtime`'s `validate_argv_value`: reject an empty
    /// value, a leading `-` (would be re-parsed as another flag — argument
    /// injection), and shell metacharacters (`;`, `&`, `|`, `` ` ``, `$`, …)
    /// that could enable command injection on any path that reaches a shell.
    pub fn validate_argv_value(label: &str, value: &str) -> AppResult<()> {
        if value.is_empty() {
            return Err(AppError::bad_request(
                "INVALID_ARGV",
                format!("{label} cannot be empty"),
            ));
        }
        if value.starts_with('-') {
            return Err(AppError::bad_request(
                "INVALID_ARGV",
                format!("{label} cannot start with '-' (would be parsed as a flag): {value:?}"),
            ));
        }
        const BANNED: &[char] = &[';', '&', '|', '`', '$', '\n', '\r', '\0', '<', '>'];
        if value.chars().any(|c| BANNED.contains(&c)) {
            return Err(AppError::bad_request(
                "INVALID_ARGV",
                format!("{label} contains shell metacharacters: {value:?}"),
            ));
        }
        Ok(())
    }

    /// Build the whisper-server argv (everything after the binary).
    ///
    /// `--host 127.0.0.1` is forced (loopback hardening). The model path and the
    /// optional language both flow through [`validate_argv_value`]. A `lang` of
    /// `None` or `"auto"` is omitted (whisper auto-detects).
    ///
    /// [`validate_argv_value`]: LocalDeployment::validate_argv_value
    pub fn build_argv(model: &Path, port: u16, lang: Option<&str>) -> AppResult<Vec<String>> {
        let model_str = model.to_string_lossy().to_string();
        Self::validate_argv_value("model path", &model_str)?;

        let mut a = vec![
            "--host".to_string(),
            "127.0.0.1".to_string(),
            "--port".to_string(),
            port.to_string(),
            "-m".to_string(),
            model_str,
        ];

        if let Some(lang) = lang {
            let lang = lang.trim();
            // "auto" (and empty) → let whisper auto-detect; don't pass -l.
            if !lang.is_empty() && !lang.eq_ignore_ascii_case("auto") {
                Self::validate_argv_value("language", lang)?;
                a.push("-l".to_string());
                a.push(lang.to_string());
            }
        }
        Ok(a)
    }

    /// Apply subprocess hardening (env_clear + allow-list, stdin null, piped
    /// stdout/stderr, kill_on_drop, Linux PR_SET_PDEATHSIG).
    fn apply_hardening(cmd: &mut Command) {
        cmd.env_clear();
        // Preserve only the variables whisper-server genuinely needs to find
        // shared libraries and respect locale / timezone / GPU selection.
        for var in &[
            "PATH",
            "HOME",
            "LANG",
            "LC_ALL",
            "TZ",
            "CUDA_VISIBLE_DEVICES",
            "LD_LIBRARY_PATH",
        ] {
            if let Ok(val) = std::env::var(var) {
                cmd.env(var, val);
            }
        }
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        // PR_SET_PDEATHSIG makes the whisper-server subprocess die with the
        // server even on SIGKILL/OOM. Linux-only (copy of the bio_mcp /
        // code_sandbox squashfuse path).
        #[cfg(target_os = "linux")]
        unsafe {
            cmd.pre_exec(|| {
                let r = libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM, 0, 0, 0);
                if r == 0 {
                    Ok(())
                } else {
                    Err(std::io::Error::last_os_error())
                }
            });
        }
    }

    /// Spawn the whisper-server for `model` on `port` (loopback). Returns the
    /// spawn metadata; the caller (`auto_start`) drives the readiness poll via
    /// [`health_check`](Self::health_check) and the loopback verification via
    /// [`verify_loopback_bind`](Self::verify_loopback_bind).
    ///
    /// A previously-running instance is stopped first (single instance).
    pub async fn start(
        &self,
        binary: &Path,
        model: &Path,
        port: u16,
        lang: Option<&str>,
    ) -> AppResult<StartOutcome> {
        // Enforce the single-instance invariant: replace any existing process.
        self.stop().await?;

        let args = Self::build_argv(model, port, lang)?;
        let base_url = format!("http://127.0.0.1:{port}");
        let active_model = model
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| model.to_string_lossy().to_string());

        let mut cmd = Command::new(binary);
        cmd.args(&args);
        Self::apply_hardening(&mut cmd);

        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::internal_error(format!("failed to spawn whisper-server: {e}")))?;

        let pid = child
            .id()
            .ok_or_else(|| AppError::internal_error("failed to get whisper-server process ID"))?
            as i32;

        // Fail fast if it exited immediately (bad model, missing shared lib).
        if let Ok(Some(status)) = child.try_wait() {
            return Err(AppError::internal_error(format!(
                "whisper-server exited during startup: {status}"
            )));
        }

        let (log_broadcast, _) = tokio::sync::broadcast::channel::<String>(256);
        Self::capture_logs(&mut child, self.process.clone(), log_broadcast.clone());

        let info = ProcessInfo {
            child,
            port,
            active_model,
            started_at: Instant::now(),
            logs: std::collections::VecDeque::new(),
            log_broadcast,
        };
        {
            let mut slot = self.process.write().await;
            *slot = Some(info);
        }

        Ok(StartOutcome {
            pid,
            port,
            base_url,
        })
    }

    /// Stop the running instance (graceful kill + bounded wait). No-op when
    /// nothing is running.
    pub async fn stop(&self) -> AppResult<()> {
        let mut slot = self.process.write().await;
        let Some(mut info) = slot.take() else {
            return Ok(());
        };
        if let Err(e) = info.child.kill().await {
            tracing::warn!("voice: failed to kill whisper-server: {e}");
        }
        match tokio::time::timeout(std::time::Duration::from_secs(10), info.child.wait()).await {
            Ok(Ok(_)) => tracing::info!("voice: whisper-server stopped gracefully"),
            Ok(Err(e)) => tracing::warn!("voice: error waiting for whisper-server: {e}"),
            Err(_) => tracing::warn!("voice: whisper-server did not stop within timeout"),
        }
        Ok(())
    }

    /// The model file name (`ggml-<name>.bin`) the running instance was started
    /// with, or `None` when nothing is running. Used to detect a model change.
    pub async fn active_model(&self) -> Option<String> {
        self.process
            .read()
            .await
            .as_ref()
            .map(|p| p.active_model.clone())
    }

    /// Coarse liveness snapshot (running / pid / port / uptime).
    pub async fn status(&self) -> InstanceStatus {
        let slot = self.process.read().await;
        if let Some(info) = slot.as_ref() {
            let pid = info.child.id().map(|id| id as i32);
            InstanceStatus {
                running: pid.is_some(),
                pid,
                port: Some(info.port),
                uptime_seconds: Some(info.started_at.elapsed().as_secs() as i64),
            }
        } else {
            InstanceStatus {
                running: false,
                pid: None,
                port: None,
                uptime_seconds: None,
            }
        }
    }

    /// Probe whisper-server readiness. whisper-server answers `200` on `/`
    /// (the web UI root) once loaded and also serves `/load`; we treat a
    /// success on `/` (falling back to `/load`) as healthy.
    pub async fn health_check(&self, base_url: &str) -> AppResult<bool> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .no_proxy()
            .build()
            .map_err(AppError::internal_with_id)?;

        match client.get(base_url).send().await {
            Ok(resp) if resp.status().is_success() => Ok(true),
            _ => {
                let load_url = format!("{}/load", base_url.trim_end_matches('/'));
                match client.get(&load_url).send().await {
                    Ok(resp) => Ok(resp.status().is_success()),
                    Err(_) => Ok(false),
                }
            }
        }
    }

    /// Return up to `lines` most-recent captured log lines.
    /// Reserved for the deferred admin instance-logs endpoint (see DRIFT-1).
    #[allow(dead_code)]
    pub async fn logs(&self, lines: usize) -> Vec<String> {
        let slot = self.process.read().await;
        match slot.as_ref() {
            Some(info) => {
                let total = info.logs.len();
                let start = total.saturating_sub(lines);
                info.logs.iter().skip(start).cloned().collect()
            }
            None => Vec::new(),
        }
    }

    /// Subscribe to live logs: a broadcast receiver for new lines + a snapshot
    /// of the already-captured buffer for initial replay. Returns `None` when
    /// nothing is running. Reserved for the deferred live-log SSE endpoint
    /// (see DRIFT-1).
    #[allow(dead_code)]
    pub async fn subscribe_logs(
        &self,
    ) -> Option<(tokio::sync::broadcast::Receiver<String>, Vec<String>)> {
        let slot = self.process.read().await;
        slot.as_ref().map(|info| {
            let snapshot: Vec<String> = info.logs.iter().cloned().collect();
            (info.log_broadcast.subscribe(), snapshot)
        })
    }

    /// Fan the child's stdout+stderr into the ring buffer + broadcaster.
    fn capture_logs(
        child: &mut Child,
        process: Arc<RwLock<Option<ProcessInfo>>>,
        broadcaster: tokio::sync::broadcast::Sender<String>,
    ) {
        if let Some(stdout) = child.stdout.take() {
            let process = process.clone();
            let broadcaster = broadcaster.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = broadcaster.send(line.clone());
                    if let Some(info) = process.write().await.as_mut() {
                        push_capped(&mut info.logs, line);
                    }
                }
            });
        }
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let line = format!("[stderr] {line}");
                    let _ = broadcaster.send(line.clone());
                    if let Some(info) = process.write().await.as_mut() {
                        push_capped(&mut info.logs, line);
                    }
                }
            });
        }
    }

    /// Confirm the whisper-server process bound a LOOPBACK listener on `port`.
    /// The spawn args already force `--host 127.0.0.1`, but a buggy/malicious
    /// binary could ignore that; the proxy/transcribe path is the only thing on
    /// the box that should reach it.
    ///
    /// Linux reads `/proc/net/tcp{,6}` for a LISTEN socket on `port` and checks
    /// the local address is `127.0.0.1` / `::1`. Verdict semantics mirror the
    /// LLM runtime:
    ///   loopback listener on `port` → true;
    ///   non-loopback listener       → false (security violation);
    ///   no listener on `port`       → true (still starting / `/health` is
    ///                                       authoritative);
    ///   cannot enumerate at all     → false (strict).
    ///
    /// Non-Linux hosts are best-effort `true` (the `--host` arg is the guard).
    #[cfg(target_os = "linux")]
    pub fn verify_loopback_bind(_pid: i32, port: u16) -> bool {
        // Returns Some(true)=loopback listener, Some(false)=non-loopback
        // listener, None=no listener found. Err on unreadable /proc.
        fn scan(path: &str, port: u16, ipv6: bool) -> Result<Option<bool>, ()> {
            let content = std::fs::read_to_string(path).map_err(|_| ())?;
            let mut found_other = false;
            for line in content.lines().skip(1) {
                let mut cols = line.split_whitespace();
                let local = match cols.next() {
                    // idx 0 is the row number "0:"; local_address is idx 1
                    Some(_) => cols.next(),
                    None => None,
                };
                let Some(local) = local else { continue };
                let state = cols.next();
                // TCP state 0A == LISTEN.
                if state != Some("0A") {
                    continue;
                }
                let Some((addr_hex, port_hex)) = local.split_once(':') else {
                    continue;
                };
                let Ok(row_port) = u16::from_str_radix(port_hex, 16) else {
                    continue;
                };
                if row_port != port {
                    continue;
                }
                let is_loopback = if ipv6 {
                    // ::1 → 31 hex zeros + "1" (128-bit, little-endian dwords);
                    // whisper never binds v6 loopback in practice, so accept the
                    // canonical rendering only.
                    addr_hex.eq_ignore_ascii_case("00000000000000000000000001000000")
                } else {
                    // IPv4 s_addr, little-endian hex. 127.0.0.1 → "0100007F".
                    addr_hex.eq_ignore_ascii_case("0100007F")
                };
                if is_loopback {
                    return Ok(Some(true));
                }
                found_other = true;
            }
            Ok(if found_other { Some(false) } else { None })
        }

        let v4 = scan("/proc/net/tcp", port, false);
        let v6 = scan("/proc/net/tcp6", port, true);

        // Any non-loopback listener on the port is a hard failure.
        if v4 == Ok(Some(false)) || v6 == Ok(Some(false)) {
            return false;
        }
        // A loopback listener → verified.
        if v4 == Ok(Some(true)) || v6 == Ok(Some(true)) {
            return true;
        }
        // No match anywhere. If BOTH files were unreadable → strict false.
        if v4.is_err() && v6.is_err() {
            return false;
        }
        // Clean enumeration, no listener yet → not a failure (/health decides).
        true
    }

    #[cfg(not(target_os = "linux"))]
    pub fn verify_loopback_bind(_pid: i32, _port: u16) -> bool {
        // Best-effort — the spawn args already force `--host 127.0.0.1`.
        true
    }
}

/// Push a log line with both line-size and ring-buffer caps.
fn push_capped(buf: &mut std::collections::VecDeque<String>, mut line: String) {
    if line.len() > LOG_LINE_MAX_BYTES {
        let mut end = LOG_LINE_MAX_BYTES;
        while !line.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        line.truncate(end);
        line.push_str("…[truncated]");
    }
    if buf.len() >= LOG_BUFFER_MAX_LINES {
        buf.pop_front();
    }
    buf.push_back(line);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn pair(a: &[String], flag: &str, val: &str) -> bool {
        a.windows(2).any(|w| w[0] == flag && w[1] == val)
    }
    fn has(a: &[String], flag: &str) -> bool {
        a.iter().any(|x| x == flag)
    }

    #[test]
    fn argv_forces_loopback_and_model() {
        let m = PathBuf::from("/models/ggml-base.bin");
        let a = LocalDeployment::build_argv(&m, 5599, None).unwrap();
        assert!(pair(&a, "--host", "127.0.0.1"));
        assert!(pair(&a, "--port", "5599"));
        assert!(pair(&a, "-m", "/models/ggml-base.bin"));
        // No language flag when None.
        assert!(!has(&a, "-l"));
    }

    #[test]
    fn argv_omits_lang_for_auto() {
        let m = PathBuf::from("/models/ggml-base.bin");
        for lang in ["auto", "AUTO", "  ", ""] {
            let a = LocalDeployment::build_argv(&m, 1, Some(lang)).unwrap();
            assert!(!has(&a, "-l"), "lang {lang:?} should be omitted");
        }
    }

    #[test]
    fn argv_includes_explicit_lang() {
        let m = PathBuf::from("/models/ggml-base.bin");
        let a = LocalDeployment::build_argv(&m, 1, Some("en")).unwrap();
        assert!(pair(&a, "-l", "en"));
    }

    #[test]
    fn argv_value_rejects_metachars_and_leading_dash() {
        assert!(LocalDeployment::validate_argv_value("x", "en").is_ok());
        assert!(LocalDeployment::validate_argv_value("x", "-l").is_err());
        assert!(LocalDeployment::validate_argv_value("x", "a;b").is_err());
        assert!(LocalDeployment::validate_argv_value("x", "a`b`").is_err());
        assert!(LocalDeployment::validate_argv_value("x", "$(x)").is_err());
        assert!(LocalDeployment::validate_argv_value("x", "").is_err());
    }

    #[test]
    fn build_argv_rejects_dangerous_lang() {
        let m = PathBuf::from("/models/ggml-base.bin");
        assert!(LocalDeployment::build_argv(&m, 1, Some("en;rm -rf /")).is_err());
    }
}
