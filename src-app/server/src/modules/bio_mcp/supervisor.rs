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
        if flap_backoff_active(st.last_failure) {
            return Err(AppError::internal_error(
                "BioMCP sidecar recently failed to start; retry shortly",
            ));
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

/// Flap guard: after a failed spawn we refuse to re-spawn for `SPAWN_BACKOFF`.
/// Extracted from `ensure_healthy` so the timing decision is unit-testable (the
/// surrounding spawn path needs a real sidecar binary + config to drive).
fn flap_backoff_active(last_failure: Option<Instant>) -> bool {
    matches!(last_failure, Some(t) if t.elapsed() < SPAWN_BACKOFF)
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
#[allow(dead_code)]
pub async fn shutdown() {
    let mut st = STATE.lock().await;
    if let Some(mut r) = st.running.take() {
        let _ = r.child.start_kill();
    }
}
#[cfg(test)]
mod tests {
    use super::{
        current_env, env_pairs_from_headers, fingerprint, flap_backoff_active, is_unsafe_env_name,
        shutdown, spawn_idle_reaper, SPAWN_BACKOFF, STATE,
    };

    use crate::modules::bio_mcp::{bio_mcp_server_id, repository::BioMcpRepository};

    use sqlx::postgres::PgPoolOptions;

    use std::time::{Duration, Instant};


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


    /// Env-fingerprint recycling (gap ca2a70a5189c): REMOVING a key (admin
    /// clears an API key) must change the fingerprint so the supervisor
    /// recycles the sidecar with the new env; an empty env is stable + distinct.
    #[test]
    fn fingerprint_recycles_on_key_removal_and_handles_empty() {
        let with_key = vec![
            ("NCBI_API_KEY".to_string(), "abc".to_string()),
            ("S2_API_KEY".to_string(), "k".to_string()),
        ];
        let removed = vec![("NCBI_API_KEY".to_string(), "abc".to_string())];
        assert_ne!(
            fingerprint(&with_key),
            fingerprint(&removed),
            "removing a key must change the fingerprint (forces a recycle)"
        );
        let empty: Vec<(String, String)> = vec![];
        assert_eq!(fingerprint(&empty), fingerprint(&[]), "empty env fp is stable");
        assert_ne!(fingerprint(&empty), fingerprint(&removed));
    }


    /// The loader-hijack denylist (security control): PATH/HOME/LD_*/DYLD_* and
    /// friends are rejected as injectable sidecar env names (case-insensitive),
    /// while ordinary API-key names are allowed.
    #[test]
    fn is_unsafe_env_name_blocks_loader_hijack_vars() {
        for bad in [
            "PATH", "path", "Home", "LD_PRELOAD", "ld_library_path",
            "DYLD_INSERT_LIBRARIES", "LC_ALL", "TZ",
        ] {
            assert!(is_unsafe_env_name(bad), "{bad} must be rejected");
        }
        for ok in ["NCBI_API_KEY", "S2_API_KEY", "OPENFDA_API_KEY", "ONCOKB_TOKEN"] {
            assert!(!is_unsafe_env_name(ok), "{ok} must be allowed");
        }
    }


    /// The loader-hijack / whitelist-override env filter (security-critical: an
    /// admin must not inject these via the bio row's headers).
    #[test]
    fn is_unsafe_env_name_blocks_loader_and_whitelist_vars() {
        // Whitelist vars (case-insensitive) are blocked.
        for n in ["PATH", "path", "Home", "LANG", "lc_all", "TZ"] {
            assert!(is_unsafe_env_name(n), "{n} must be rejected");
        }
        // Dynamic-loader hijack prefixes (any case) are blocked.
        for n in [
            "LD_PRELOAD",
            "ld_library_path",
            "LD_AUDIT",
            "DYLD_INSERT_LIBRARIES",
            "dyld_library_path",
        ] {
            assert!(is_unsafe_env_name(n), "{n} must be rejected");
        }
        // Legitimate upstream API keys are allowed.
        for n in [
            "NCBI_API_KEY",
            "S2_API_KEY",
            "OPENFDA_API_KEY",
            "ONCOKB_TOKEN",
            "PATHOLOGY", // not exactly PATH; substring must not match
            "MYLD_KEY",  // does not start with LD_ / DYLD_
        ] {
            assert!(!is_unsafe_env_name(n), "{n} must be allowed");
        }
    }


    /// The recycle decision (`env_fingerprint == fp` in `ensure_healthy`) must
    /// recycle the sidecar when a key is REMOVED and must NOT recycle on a mere
    /// header-ordering difference — `current_env` sorts its `(name, value)`
    /// pairs before fingerprinting precisely so an admin re-saving the same keys
    /// in a different order doesn't force a spurious respawn. Both properties
    /// were unasserted by `fingerprint_is_stable_and_value_sensitive` (which only
    /// covered same/changed/added).
    #[test]
    fn fingerprint_recycles_on_removed_key_but_is_order_insensitive_after_sort() {
        // Two upstream keys, configured in opposite insertion orders. The real
        // `current_env` sorts before calling `fingerprint`, so model that here.
        let mut two_ab = vec![
            ("NCBI_API_KEY".to_string(), "n".to_string()),
            ("S2_API_KEY".to_string(), "s".to_string()),
        ];
        let mut two_ba = vec![
            ("S2_API_KEY".to_string(), "s".to_string()),
            ("NCBI_API_KEY".to_string(), "n".to_string()),
        ];
        two_ab.sort();
        two_ba.sort();
        // Same set of pairs, different original order → identical fingerprint
        // (no spurious recycle when the admin re-saves the same keys).
        assert_eq!(fingerprint(&two_ab), fingerprint(&two_ba));

        // Removing a key (admin cleared S2_API_KEY) must change the fingerprint
        // so the sidecar is recycled and no longer sees the dropped key.
        let mut one = vec![("NCBI_API_KEY".to_string(), "n".to_string())];
        one.sort();
        assert_ne!(fingerprint(&two_ab), fingerprint(&one));

        // The empty env (all keys removed → unauthenticated mode) is distinct
        // from any keyed config, so dropping the last key also recycles.
        let none: Vec<(String, String)> = Vec::new();
        assert_ne!(fingerprint(&one), fingerprint(&none));
    }


    /// Drives the REAL `current_env()` end-to-end against the bio row in the DB
    /// (the existing tests only cover its building blocks — the `is_unsafe_env_name`
    /// denylist and the `fingerprint` of its output — never `current_env` itself).
    /// Seeds the bio row's plain `headers` with a mix of legitimate upstream
    /// API keys, a denylisted loader/whitelist name, and an empty value, then
    /// asserts the function's three guarantees:
    ///   - denylisted names (`PATH`, `LD_PRELOAD`) are filtered out (loader-hijack
    ///     prevention),
    ///   - empty values are skipped, legitimate keys are injected, and
    ///   - the output is SORTED (the contract `fingerprint` relies on),
    /// plus the disabled-row early return → `BIO_DISABLED`.
    ///
    /// DB-gated: soft-skips (mirroring the suite's env-gated tests) when no
    /// Postgres is reachable, so `cargo test --lib` without a DB stays green;
    /// runs for real wherever `DATABASE_URL` points at a migrated DB.
    #[tokio::test]
    async fn current_env_filters_denylist_and_empties_and_sorts() {
        let url = match std::env::var("DATABASE_URL") {
            Ok(u) => u,
            Err(_) => {
                eprintln!("skip: DATABASE_URL unset — no DB to exercise current_env against");
                return;
            }
        };
        let pool = match PgPoolOptions::new().max_connections(2).connect(&url).await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("skip: DB unreachable ({e})");
                return;
            }
        };
        // `current_env` reads the bio row via the global `Repos`; init is
        // idempotent (no-op if another lib test already won the race).
        crate::core::init_repositories(pool.clone());

        let bio_id = bio_mcp_server_id();
        // Ensure the bio row exists (boot normally upserts it; recreate here so
        // the test is self-contained on a fresh DB).
        BioMcpRepository::new(pool.clone())
            .upsert_builtin_server(bio_id, "http://127.0.0.1:1/api/bio/mcp")
            .await
            .expect("upsert bio builtin row");

        // Seed the plain (non-secret) `headers` column with a deliberate mix:
        // two legitimate keys (out of alpha order so the sort is observable), a
        // denylisted whitelist var + a loader-hijack var, and an empty value.
        let headers = serde_json::json!({
            "S2_API_KEY": "sval",
            "NCBI_API_KEY": "nval",
            "PATH": "/evil/bin",
            "LD_PRELOAD": "/evil/lib.so",
            "OPENFDA_API_KEY": "",
        });
        sqlx::query("UPDATE mcp_servers SET headers = $1, enabled = true WHERE id = $2")
            .bind(&headers)
            .bind(bio_id)
            .execute(&pool)
            .await
            .expect("seed bio headers + enable");

        let env = current_env().await.expect("current_env should succeed when enabled");
        // Denylisted (PATH, LD_PRELOAD) and empty (OPENFDA_API_KEY) are gone;
        // the legitimate keys survive, SORTED by name.
        assert_eq!(
            env,
            vec![
                ("NCBI_API_KEY".to_string(), "nval".to_string()),
                ("S2_API_KEY".to_string(), "sval".to_string()),
            ],
            "current_env must filter denylisted + empty headers and return the rest sorted"
        );

        // Disabled row → early-return BIO_DISABLED (not an env map).
        sqlx::query("UPDATE mcp_servers SET enabled = false WHERE id = $1")
            .bind(bio_id)
            .execute(&pool)
            .await
            .expect("disable bio row");
        let err = current_env().await.expect_err("disabled bio row must error");
        assert_eq!(err.error_code(), "BIO_DISABLED");

        // Hygiene: leave the shared bio row re-enabled with empty headers so
        // sibling tests / the running server see a clean default.
        sqlx::query("UPDATE mcp_servers SET enabled = true, headers = '{}'::jsonb WHERE id = $1")
            .bind(bio_id)
            .execute(&pool)
            .await
            .ok();
    }


    /// The idle reaper's first `interval.tick()` fires immediately, so spawning
    /// it runs one iteration right away. Over an EMPTY state (no sidecar in this
    /// unit-test process) that iteration must be a harmless no-op: it must not
    /// panic, poison the STATE mutex, or fabricate a `running` sidecar.
    #[tokio::test]
    async fn idle_reaper_first_tick_is_a_safe_noop_over_empty_state() {
        // Clean baseline (tests share the process-global STATE).
        shutdown().await;
        assert!(STATE.lock().await.running.is_none(), "baseline: no sidecar");

        spawn_idle_reaper();
        // The first tick is immediate; give the spawned task a moment to run it.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // STATE must still be lockable (not poisoned) and hold no sidecar.
        let st = STATE.lock().await;
        assert!(
            st.running.is_none(),
            "the reaper must not fabricate a sidecar when none is running"
        );
    }


    /// `shutdown()` is the graceful-shutdown hook: with no sidecar running it
    /// must be a safe no-op (no panic / no lock poisoning), idempotent across
    /// repeated calls, and leave the supervisor state with no `running` sidecar
    /// so the next `ensure_healthy()` would respawn cleanly.
    #[tokio::test]
    async fn shutdown_is_idempotent_noop_when_idle() {
        // Nothing has spawned a sidecar in this unit-test process.
        shutdown().await;
        shutdown().await; // idempotent — second call must not panic either.
        let st = STATE.lock().await;
        assert!(
            st.running.is_none(),
            "after shutdown the supervisor holds no running sidecar"
        );
    }


    /// Flap guard: refuse re-spawn only while a recent failure is inside the
    /// SPAWN_BACKOFF window; no failure (None) or an old one never backs off.
    #[test]
    fn flap_backoff_window() {
        assert!(!flap_backoff_active(None), "no prior failure → never back off");
        assert!(flap_backoff_active(Some(Instant::now())), "a just-now failure must back off");
        let old = Instant::now().checked_sub(SPAWN_BACKOFF + Duration::from_secs(1)).unwrap();
        assert!(!flap_backoff_active(Some(old)), "a failure older than SPAWN_BACKOFF must NOT back off");
    }


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
}
