//! Managed BioMCP sidecar supervisor.
//!
//! Owns ONE long-lived `biomcp serve-http` process per ziee process,
//! lazily spawned on the first `/api/bio/mcp` call (like code_sandbox
//! lazy-mounts squashfuse). All proxied MCP requests funnel through this
//! single sidecar, so BioMCP's process-local rate limiting is effectively
//! deployment-wide (for the common single-process + desktop cases).
//!
//! Hardening mirrors `llm_local_runtime::deployment::local::apply_hardening`:
//! `env_clear` + a minimal PATH/HOME/LANG/TZ whitelist + the admin-configured
//! upstream API keys (read from the bio row's decrypted `headers`). No
//! `DATABASE_URL` / JWT secret / unrelated `*_API_KEY` reaches the sidecar.
//!
//! Teardown: `kill_on_drop` + `PR_SET_PDEATHSIG` (Linux) so the sidecar
//! dies with the server even on SIGKILL/OOM. macOS/Windows fall back to
//! the idle reaper + explicit `shutdown()` (same limitation as the local
//! engine runtime).

use std::process::Stdio;
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::common::AppError;
use crate::core::Repos;

use super::{bio_mcp_server_id, embedded};

/// Wait this long for the sidecar's `/readyz` to go green after spawn.
const READY_TIMEOUT: Duration = Duration::from_secs(30);
const READY_POLL: Duration = Duration::from_millis(500);
/// Evict the sidecar after this much idle time (no proxied calls).
const IDLE_EVICT: Duration = Duration::from_secs(900);
/// After a failed spawn, refuse to re-spawn for this long (flap guard).
const SPAWN_BACKOFF: Duration = Duration::from_secs(5);
/// How often the idle reaper checks.
const REAPER_TICK: Duration = Duration::from_secs(60);

struct Running {
    child: Child,
    base_url: String,
    last_used: Instant,
    /// Hash of the env injected at spawn — a change (admin edited the keys)
    /// forces a recycle so the new keys take effect.
    env_fingerprint: u64,
}

#[derive(Default)]
struct SupervisorState {
    running: Option<Running>,
    last_failure: Option<Instant>,
}

static STATE: Lazy<Mutex<SupervisorState>> = Lazy::new(|| Mutex::new(SupervisorState::default()));

/// Serializes sidecar spawns (single-flight) WITHOUT holding `STATE` across
/// the up-to-30s readiness wait, so the fast path + the idle reaper aren't
/// blocked during a cold start. Lock order is always SPAWN_LOCK → STATE.
static SPAWN_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

/// Ensure the sidecar is running and ready, returning its loopback base URL
/// (e.g. `http://127.0.0.1:34567`). Single-flight via `SPAWN_LOCK` — concurrent
/// first-callers don't double-spawn — WITHOUT holding `STATE` across the
/// readiness wait. Recycles the sidecar when the bio row is disabled mid-flight
/// or its keys changed.
pub async fn ensure_healthy() -> Result<String, AppError> {
    // `current_env` enforces the admin enable toggle + reads the keys, so
    // the disabled-path error is exercised even under the test seam below.
    let env = current_env().await?;
    let fp = fingerprint(&env);

    // Debug-only testability seam (compiled out of release builds, like
    // code_sandbox's `CODE_SANDBOX_ROOTFS_MIRROR`): point the proxy at a
    // mock sidecar without spawning the real biomcp binary.
    #[cfg(debug_assertions)]
    if let Ok(url) = std::env::var("BIO_MCP_SIDECAR_URL") {
        if !url.is_empty() {
            return Ok(url);
        }
    }

    // Fast path: a healthy sidecar with the current keys is already running.
    // Held only for the cheap check, never across a spawn.
    {
        let mut st = STATE.lock().await;
        if let Some(r) = st.running.as_mut() {
            if matches!(r.child.try_wait(), Ok(None)) && r.env_fingerprint == fp {
                r.last_used = Instant::now();
                return Ok(r.base_url.clone());
            }
        }
    }

    // Serialize spawns. Holding SPAWN_LOCK (not STATE) keeps single-flight
    // while leaving STATE free during the up-to-30s readiness wait.
    let _spawn_guard = SPAWN_LOCK.lock().await;

    // Re-check under STATE: another caller may have spawned while we waited on
    // SPAWN_LOCK; or a dead / stale-keys child must be torn down first.
    {
        let mut st = STATE.lock().await;
        if let Some(r) = st.running.as_mut() {
            if matches!(r.child.try_wait(), Ok(None)) && r.env_fingerprint == fp {
                r.last_used = Instant::now();
                return Ok(r.base_url.clone());
            }
            if let Some(mut old) = st.running.take() {
                let _ = old.child.start_kill();
            }
        }
        if let Some(t) = st.last_failure {
            if t.elapsed() < SPAWN_BACKOFF {
                return Err(AppError::internal_error(
                    "BioMCP sidecar recently failed to start; retry shortly",
                ));
            }
        }
    }

    // Spawn WITHOUT holding STATE (single-flight via SPAWN_LOCK).
    match spawn_and_wait(&env, fp).await {
        Ok(running) => {
            let url = running.base_url.clone();
            let mut st = STATE.lock().await;
            st.running = Some(running);
            st.last_failure = None;
            Ok(url)
        }
        Err(e) => {
            let mut st = STATE.lock().await;
            st.last_failure = Some(Instant::now());
            Err(e)
        }
    }
}

/// Read the bio row's `enabled` flag + decrypted `headers` (the upstream
/// API keys) and return them as `(ENV_NAME, value)` pairs to inject. Errors
/// if the row is missing or the admin disabled it.
async fn current_env() -> Result<Vec<(String, String)>, AppError> {
    let server = Repos
        .mcp
        .get_any_server(bio_mcp_server_id())
        .await?
        .ok_or_else(|| AppError::internal_error("BioMCP server row not found"))?;

    if !server.enabled {
        return Err(AppError::bad_request(
            "BIO_DISABLED",
            "BioMCP is disabled by the administrator",
        ));
    }

    Ok(env_pairs_from_headers(&server.headers))
}

/// Pure projection of the bio row's `headers` JSON into the `(ENV_NAME, value)`
/// pairs to inject: header names map 1:1 to upstream env-var names
/// (e.g. NCBI_API_KEY); empty values are skipped; and unsafe names
/// (`is_unsafe_env_name`) are rejected so a misconfigured/compromised row can't
/// inject `LD_PRELOAD` or replace `PATH`. Output is sorted for a stable
/// fingerprint. Extracted from `current_env` so the filtering can be unit-tested
/// without a DB.
fn env_pairs_from_headers(headers: &serde_json::Value) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    if let Some(map) = headers.as_object() {
        for (k, v) in map {
            if let Some(s) = v.as_str() {
                if s.is_empty() {
                    continue;
                }
                if is_unsafe_env_name(k) {
                    tracing::warn!("bio_mcp: ignoring unsafe env-var name in headers: {}", k);
                    continue;
                }
                out.push((k.clone(), s.to_string()));
            }
        }
    }
    out.sort();
    out
}

/// True for env-var names that the admin must not be able to inject via the
/// bio row's headers — the env_clear whitelist vars (overriding PATH would
/// let biomcp exec arbitrary binaries) and the dynamic-loader hijack vars.
fn is_unsafe_env_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    const PROTECTED: &[&str] = &["PATH", "HOME", "LANG", "LC_ALL", "TZ"];
    PROTECTED.contains(&upper.as_str())
        || upper.starts_with("LD_")
        || upper.starts_with("DYLD_")
}

fn fingerprint(env: &[(String, String)]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (k, v) in env {
        k.hash(&mut h);
        v.hash(&mut h);
    }
    h.finish()
}

async fn spawn_and_wait(env: &[(String, String)], fp: u64) -> Result<Running, AppError> {
    let binary = embedded::ensure_biomcp_extracted()?;
    let port = portpicker::pick_unused_port()
        .ok_or_else(|| AppError::internal_error("No available port for BioMCP sidecar"))?;
    let base_url = format!("http://127.0.0.1:{}", port);

    let mut cmd = Command::new(binary);
    cmd.arg("serve-http")
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string());

    // Hardening: wipe the inherited env, restore only what the sidecar
    // needs to find shared libs + respect locale, then inject the
    // admin-configured upstream API keys.
    cmd.env_clear();
    for var in &["PATH", "HOME", "LANG", "LC_ALL", "TZ"] {
        if let Ok(val) = std::env::var(var) {
            cmd.env(var, val);
        }
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);

    // PR_SET_PDEATHSIG makes the sidecar die with the server even on
    // SIGKILL/OOM. Linux-only (copy of the code_sandbox squashfuse path).
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

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::internal_error(format!("Failed to spawn biomcp sidecar: {}", e)))?;

    drain_logs(&mut child);

    // Poll /readyz until ready or timeout (mirrors auto_start's loop).
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .no_proxy()
        .build()
        .map_err(|e| AppError::internal_error(format!("reqwest build failed: {}", e)))?;
    let ready_url = format!("{}/readyz", base_url);
    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return Err(AppError::internal_error(format!(
                "biomcp sidecar exited during startup: {}",
                status
            )));
        }
        if let Ok(resp) = client.get(&ready_url).send().await {
            if resp.status().is_success() {
                break;
            }
        }
        if Instant::now() >= deadline {
            let _ = child.start_kill();
            return Err(AppError::internal_error(
                "biomcp sidecar did not become ready within 30s (offline?)",
            ));
        }
        tokio::time::sleep(READY_POLL).await;
    }

    tracing::info!("bio_mcp: sidecar ready at {}", base_url);
    Ok(Running {
        child,
        base_url,
        last_used: Instant::now(),
        env_fingerprint: fp,
    })
}

/// Stream the sidecar's stdout/stderr into tracing so the pipe never
/// fills (which would block the child) and so operators can debug.
fn drain_logs(child: &mut Child) {
    use tokio::io::{AsyncBufReadExt, BufReader};
    if let Some(out) = child.stdout.take() {
        tokio::spawn(async move {
            let mut lines = BufReader::new(out).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::debug!("bio_mcp[out]: {}", line);
            }
        });
    }
    if let Some(err) = child.stderr.take() {
        tokio::spawn(async move {
            let mut lines = BufReader::new(err).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::debug!("bio_mcp[err]: {}", line);
            }
        });
    }
}

/// Background task: evict the sidecar after `IDLE_EVICT` of no calls, and
/// reap a sidecar that died on its own. Spawned once at module init.
pub fn spawn_idle_reaper() {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(REAPER_TICK);
        loop {
            interval.tick().await;
            // `try_lock` (not `lock().await`): if `ensure_healthy` holds the
            // lock across a cold-start spawn, just skip this tick rather than
            // queue behind the up-to-30s readiness wait.
            let mut st = match STATE.try_lock() {
                Ok(st) => st,
                Err(_) => continue,
            };
            let drop_it = match st.running.as_mut() {
                Some(r) => {
                    let dead = !matches!(r.child.try_wait(), Ok(None));
                    dead || r.last_used.elapsed() >= IDLE_EVICT
                }
                None => false,
            };
            if drop_it {
                if let Some(mut r) = st.running.take() {
                    tracing::info!("bio_mcp: evicting idle/dead sidecar");
                    let _ = r.child.start_kill();
                }
            }
        }
    });
}

/// Kill the sidecar (graceful-shutdown hook / tests). The next
/// `ensure_healthy()` respawns it.
pub async fn shutdown() {
    let mut st = STATE.lock().await;
    if let Some(mut r) = st.running.take() {
        let _ = r.child.start_kill();
    }
}

#[cfg(test)]
mod tests {
    use super::{env_pairs_from_headers, fingerprint, is_unsafe_env_name};

    #[test]
    fn env_pairs_from_headers_filters_empty_and_unsafe_and_sorts() {
        // current_env()'s core projection: legit API keys pass through, empty
        // values are dropped, loader/whitelist-hijack names are rejected, and
        // the output is sorted (stable fingerprint).
        let headers = serde_json::json!({
            "NCBI_API_KEY": "ncbi-secret",
            "S2_API_KEY": "s2-secret",
            "EMPTY_KEY": "",            // dropped: empty value
            "PATH": "/evil/bin",        // rejected: env_clear whitelist var
            "LD_PRELOAD": "/x.so",      // rejected: loader hijack
            "DYLD_INSERT_LIBRARIES": "/y.dylib", // rejected: loader hijack
            "NUMERIC": 5,               // dropped: non-string value
        });
        let pairs = env_pairs_from_headers(&headers);

        assert_eq!(
            pairs,
            vec![
                ("NCBI_API_KEY".to_string(), "ncbi-secret".to_string()),
                ("S2_API_KEY".to_string(), "s2-secret".to_string()),
            ],
            "only non-empty, safe, string-valued headers survive, sorted"
        );
        // None of the rejected names leak into the injected env.
        let names: Vec<&str> = pairs.iter().map(|(k, _)| k.as_str()).collect();
        for blocked in ["PATH", "LD_PRELOAD", "DYLD_INSERT_LIBRARIES", "EMPTY_KEY", "NUMERIC"] {
            assert!(!names.contains(&blocked), "{blocked} must not be injected");
        }
    }

    #[test]
    fn env_pairs_from_headers_empty_object_is_empty() {
        assert!(env_pairs_from_headers(&serde_json::json!({})).is_empty());
        assert!(env_pairs_from_headers(&serde_json::Value::Null).is_empty());
    }

    #[test]
    fn is_unsafe_env_name_blocks_loader_and_whitelist_vars() {
        // The env_clear whitelist vars must never be admin-injectable
        // (case-insensitive) — overriding PATH/HOME would let biomcp exec
        // arbitrary binaries.
        for v in ["PATH", "HOME", "LANG", "LC_ALL", "TZ"] {
            assert!(is_unsafe_env_name(v), "{v} must be protected");
            assert!(
                is_unsafe_env_name(&v.to_ascii_lowercase()),
                "{v} must be protected case-insensitively"
            );
        }
        // Dynamic-loader hijack prefixes (LD_*, DYLD_*).
        assert!(is_unsafe_env_name("LD_PRELOAD"));
        assert!(is_unsafe_env_name("LD_LIBRARY_PATH"));
        assert!(is_unsafe_env_name("ld_preload"));
        assert!(is_unsafe_env_name("DYLD_INSERT_LIBRARIES"));
        assert!(is_unsafe_env_name("dyld_insert_libraries"));

        // Legitimate upstream API-key vars (and other names) are allowed.
        for v in [
            "NCBI_API_KEY",
            "S2_API_KEY",
            "OPENFDA_API_KEY",
            "ONCOKB_TOKEN",
            "SOME_OTHER_VAR",
            "PATHOLOGY", // starts with PATH but is NOT exactly PATH
        ] {
            assert!(!is_unsafe_env_name(v), "{v} must be allowed");
        }
    }

    #[test]
    fn fingerprint_is_stable_and_value_sensitive() {
        let a = vec![("NCBI_API_KEY".to_string(), "abc".to_string())];
        let b = vec![("NCBI_API_KEY".to_string(), "abc".to_string())];
        let c = vec![("NCBI_API_KEY".to_string(), "xyz".to_string())];
        // Same keys → same fingerprint (no needless recycle).
        assert_eq!(fingerprint(&a), fingerprint(&b));
        // Changed value → different fingerprint (recycle picks up new key).
        assert_ne!(fingerprint(&a), fingerprint(&c));
        // Added key → different fingerprint.
        let mut d = a.clone();
        d.push(("S2_API_KEY".to_string(), "k".to_string()));
        assert_ne!(fingerprint(&a), fingerprint(&d));
    }
}
