//! code_sandbox — bwrap-isolated code execution exposed as a built-in
//! MCP server.
//!
//! Architecture:
//! the sandbox registers as a regular row in `mcp_servers` with
//! `is_built_in=true` + `transport_type='http'`, points at a loopback
//! URL on the same axum app, and serves JSON-RPC at `/api/code-sandbox`.
//! `mcp.rs` has zero knowledge of this module by name — the integration
//! is via the regular MCP path + the JWT injection that `client/manager.rs`
//! already does for `is_built_in` servers.

use std::error::Error;
use std::sync::Arc;

use aide::axum::ApiRouter;
use linkme::distributed_slice;
use sqlx::PgPool;
use uuid::Uuid;

use crate::module_api::{AppModule, ModuleContext, ModuleEntry, MODULE_ENTRIES};

pub mod backend;
pub mod cgroup;
pub mod config;
pub mod embedded;
#[cfg(target_os = "windows")]
pub mod wsl2_agent_embedded;
pub mod handlers;
pub mod runtime_fetch;
pub mod runtime_mount;
pub mod models;
pub mod permissions;
pub mod probes;
pub mod repository;
pub mod resource_limits;
pub mod resource_limits_cache;
pub mod mcp_spawn;
pub mod routes;
pub mod sandbox;
pub mod streaming;
pub mod tools;
pub mod types;
pub mod version_handlers;
pub mod version_install_tasks;
pub mod version_manager;

pub use repository::CodeSandboxRepository;

/// Deterministic UUID for the built-in sandbox MCP server row.
/// Stable across deployments so the same row is hit by every install.
pub fn code_sandbox_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"code-sandbox.ziee.internal")
}

/// Resolve the host portion of the built-in code_sandbox MCP server's
/// URL. **Always returns a loopback address** — never the operator's
/// `server.host` config value.
///
/// SECURITY: an earlier implementation passed `server.host` through
/// unchanged when it was a concrete address. That meant a config-set
/// `server.host = attacker.com` would register the built-in MCP
/// server's URL as `http://attacker.com:port/api/code-sandbox`, and
/// the MCP client (`mcp/client/manager.rs:78-113`) would then ship
/// every JWT-signed bearer + per-call context to attacker.com. This
/// matters because config / env-var (e.g. `SERVER__HOST=...`) is
/// often less guarded than DB credentials in container orchestration.
///
/// We pin to `127.0.0.1` because the loopback endpoint is the only
/// place this server can route the call to (we're invoking ourselves
/// through the local axum stack). The operator's `server.host` value
/// controls what the server BINDS to externally — but a sandbox
/// "loopback" must, by definition, dial `127.0.0.1`.
pub fn loopback_host(_server_host: &str) -> &str {
    "127.0.0.1"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_host_always_127_0_0_1_for_wildcards() {
        assert_eq!(loopback_host("0.0.0.0"), "127.0.0.1");
        assert_eq!(loopback_host("::"), "127.0.0.1");
        assert_eq!(loopback_host("[::]"), "127.0.0.1");
        assert_eq!(loopback_host("0:0:0:0:0:0:0:0"), "127.0.0.1");
        assert_eq!(loopback_host(""), "127.0.0.1");
        assert_eq!(loopback_host("  "), "127.0.0.1");
    }

    #[test]
    fn loopback_host_pins_to_loopback_regardless_of_server_host() {
        // SECURITY regression test: the built-in MCP server's URL
        // must NEVER be configurable to a non-loopback address. If
        // server.host was `attacker.com`, an earlier implementation
        // would have passed that through and the MCP client would
        // ship JWT-signed bearer tokens to attacker.com per call.
        assert_eq!(loopback_host("attacker.com"), "127.0.0.1");
        assert_eq!(loopback_host("10.0.0.5"), "127.0.0.1");
        assert_eq!(loopback_host("169.254.169.254"), "127.0.0.1"); // IMDS
        assert_eq!(loopback_host("example.local"), "127.0.0.1");
        assert_eq!(loopback_host("[2001:db8::1]"), "127.0.0.1");
        // Even passing 127.0.0.1 itself yields the canonical form.
        assert_eq!(loopback_host("127.0.0.1"), "127.0.0.1");
    }

    #[test]
    fn code_sandbox_server_id_is_stable() {
        // The migration-36 hardcoded UUID assumes this exact value;
        // changing this constant requires a coordinated schema bump.
        assert_eq!(
            code_sandbox_server_id().to_string(),
            "b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd"
        );
    }

}

#[distributed_slice(MODULE_ENTRIES)]
static CODE_SANDBOX_MODULE_REGISTRATION: ModuleEntry = ModuleEntry {
    name: "code_sandbox",
    // After mcp (65) so the mcp_servers table is fully initialized.
    order: 70,
    description: "bwrap-isolated code execution sandbox (built-in MCP server)",
    constructor: || Box::new(CodeSandboxModule::new()),
};

pub struct CodeSandboxModule {
    pool: Option<Arc<PgPool>>,
}

impl CodeSandboxModule {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

impl Default for CodeSandboxModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AppModule for CodeSandboxModule {
    fn name(&self) -> &'static str {
        "code_sandbox"
    }

    fn description(&self) -> &'static str {
        "bwrap-isolated code execution sandbox (built-in MCP server)"
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), Box<dyn Error>> {
        self.pool = Some(ctx.db_pool.clone());

        let cfg = ctx.config.code_sandbox.clone().unwrap_or_default();
        if !cfg.enabled {
            tracing::info!(
                "code_sandbox: disabled in config; skipping init (no rootfs probe, no MCP row)"
            );
            return Ok(());
        }

        // ---- Boot probes: HOST-only (cheap; no rootfs dependence) ----
        // Rootfs-dependent probes (PID-ns, schema sentinel) are deferred
        // until the first `execute_command` call via
        // `runtime_mount::ensure_rootfs_ready`. This means users who
        // never invoke code execution pay zero FUSE-process cost and
        // zero squashfuse latency at boot.
        //
        // The one thing we still fail-loud on at boot is missing bwrap:
        // it's not something the operator can fix at runtime, and
        // surfacing it as a per-call MCP error would surprise users.
        // Boot probe routed through the cross-platform backend seam: Linux
        // checks bwrap+cgroup+seccomp (today's behavior), macOS checks
        // aarch64+launcher, Windows checks wsl.exe+v2-default. Each backend
        // logs its own "skipping registration" reason on `None`.
        let host_caps = match backend::active().probe_host(&cfg) {
            Some(h) => h,
            None => return Ok(()),
        };

        // Audit H-4: if the cloud instance metadata service is reachable
        // from the host, `--share-net` (in build_bwrap_argv) would expose it
        // to LLM-generated code — and IMDS hands out IAM credentials.
        // Refuse to register unless the operator has explicitly opted in
        // via `allow_cloud_imds_reachable: true`. Cheap host-only probe:
        // 200ms connect-timeout against 169.254.169.254:80.
        if !cfg.allow_cloud_imds_reachable && cloud_imds_reachable() {
            tracing::error!(
                "code_sandbox: cloud IMDS endpoint (169.254.169.254:80) is \
                 reachable from this host. With `--share-net` (the current \
                 sandbox network mode), LLM-generated code could fetch IAM/role \
                 credentials and exfiltrate them. Either run the server on a \
                 host where IMDS is not reachable (most on-prem / dev boxes), \
                 OR set code_sandbox.allow_cloud_imds_reachable: true to accept \
                 the risk (e.g. when behind IMDSv2 + hop-limit=1). Sandbox MCP \
                 row will NOT be registered."
            );
            return Ok(());
        }

        // ---- Workspace root + per-conversation reaper (Phase 8) ----
        let app_data_dir = crate::core::get_app_data_dir();
        let workspace_root = app_data_dir.join("sandboxes");
        if let Err(e) = std::fs::create_dir_all(&workspace_root) {
            tracing::error!(
                "code_sandbox: cannot create workspace root {}: {e}",
                workspace_root.display()
            );
            return Ok(());
        }
        // Audit H-3: deny other local users even *listing* sibling conversation
        // workspaces. Per-conversation dirs are chmod'd separately by
        // handlers::build_context (mode depends on backend); this lock is the
        // outer guard so the per-conversation 0o1777 (Mac/WSL2) isn't traversable
        // by a non-server user.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(
                &workspace_root,
                std::fs::Permissions::from_mode(0o700),
            );
        }

        // ---- Compute loopback URL (Phase 6 seeding) ----
        let host = loopback_host(&ctx.config.server.host);
        let loopback_url = format!(
            "http://{host}:{port}/api/code-sandbox",
            host = host,
            port = ctx.config.server.port,
        );

        let state = types::CodeSandboxState {
            config: cfg.clone(),
            loopback_url: loopback_url.clone(),
            workspace_root: workspace_root.clone(),
            host_caps,
            pool: Some(ctx.db_pool.clone()),
        };
        let _state_arc = config::init_state(state);

        // ---- Upsert the built-in MCP server row (Phase 6) ----
        let server_id = code_sandbox_server_id();
        let pool = ctx.db_pool.clone();
        let upsert_url = loopback_url.clone();
        tokio::spawn(async move {
            let repo = repository::CodeSandboxRepository::new((*pool).clone());
            if let Err(e) = repo.upsert_builtin_server(server_id, &upsert_url).await {
                tracing::error!("code_sandbox: upsert_builtin_server failed: {e:?}");
            } else {
                tracing::info!(
                    "code_sandbox: upsert built-in server {server_id} at {upsert_url}"
                );
            }
        });

        // ---- Workspace reaper (Phase 8) ----
        let reaper_root = workspace_root.clone();
        tokio::spawn(async move {
            workspace_reaper(reaper_root).await;
        });

        // ---- Pin-latest-on-first-run probe (Plan 5 Phase 2) ----
        // Reads the persisted pin; if NULL and GitHub is reachable,
        // sets it to the latest semver release. Soft-fail: if GitHub
        // is unreachable we log + leave the pin NULL, the next
        // `execute_command` retries via the lazy auto-fetch path.
        let pin_pool = ctx.db_pool.clone();
        tokio::spawn(async move {
            match version_manager::ensure_pin_initialized(&pin_pool).await {
                Ok(Some(pin)) => {
                    let installed =
                        version_manager::list_installed(&pin_pool).await.unwrap_or_default();
                    let downloaded: Vec<String> = installed
                        .iter()
                        .filter(|a| a.version == pin)
                        .map(|a| format!("{}-{}", a.arch, a.flavor))
                        .collect();
                    tracing::info!(
                        "code_sandbox: rootfs version pinned at v{}; downloaded flavors = {:?}",
                        pin,
                        downloaded
                    );
                }
                Ok(None) => {
                    tracing::warn!(
                        "code_sandbox: rootfs version not yet pinned — \
                         will pin on first reachable GitHub call"
                    );
                }
                Err(e) => {
                    tracing::warn!("code_sandbox: rootfs pin probe failed: {e}");
                }
            }
        });

        tracing::info!(
            "code_sandbox: registered (rootfs will mount on first execute_command)"
        );
        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(routes::code_sandbox_router())
    }
}

/// Background task: every 6 hours, remove subdirectories of
/// `workspace_root` whose `mtime` is older than 30 days. Best-effort:
/// any IO error is logged and the task continues.
async fn workspace_reaper(root: std::path::PathBuf) {
    use std::time::{Duration, SystemTime};
    const TICK: Duration = Duration::from_secs(6 * 60 * 60);
    const MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

    tracing::info!(
        "code_sandbox: workspace reaper started; root={} max_age=30d tick=6h",
        root.display()
    );

    loop {
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                let meta = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                if !meta.is_dir() {
                    continue;
                }
                // Skip shared subsystem dirs (not per-conversation):
                //   `attachments/` is shared staging for
                //   bind-mounted user attachments;
                //   `identity/` is the shared synthetic passwd/group.
                if let Some(name) = path.file_name().and_then(|n| n.to_str())
                    && (name == "attachments" || name == "identity") {
                        continue;
                    }
                // Prefer the explicit `.last_used` sentinel: every
                // `run_in_sandbox` call writes the current Unix
                // timestamp here, so a long-running conversation that
                // only reads/edits existing files keeps the sentinel
                // mtime fresh. Fall back to the directory mtime if
                // the sentinel doesn't exist (workspace created but
                // no call yet, or pre-sentinel-era workspaces).
                let sentinel = path.join(".last_used");
                let mtime = std::fs::metadata(&sentinel)
                    .and_then(|m| m.modified())
                    .or_else(|_| meta.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                let age = SystemTime::now()
                    .duration_since(mtime)
                    .unwrap_or(Duration::ZERO);
                if age > MAX_AGE {
                    match std::fs::remove_dir_all(&path) {
                        Ok(()) => {
                            // L3: bound CONVERSATION_LOCKS — drop the lock entry
                            // for the reaped conversation (dir name = conv UUID).
                            if let Some(cid) = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .and_then(|n| uuid::Uuid::parse_str(n).ok())
                            {
                                handlers::prune_conversation_lock(cid);
                            }
                            tracing::info!(
                                "code_sandbox: reaped stale workspace {} (age={}d)",
                                path.display(),
                                age.as_secs() / 86_400
                            )
                        }
                        Err(e) => tracing::warn!(
                            "code_sandbox: failed to reap {}: {e}",
                            path.display()
                        ),
                    }
                }
            }
        }
        tokio::time::sleep(TICK).await;
    }
}

/// Audit H-4: synchronous TCP connect to the cloud instance metadata
/// endpoint with a tight timeout. Used at boot to refuse-to-register when
/// `--share-net` would expose the IMDS to LLM-generated code. Returns
/// `true` when the endpoint accepted a TCP connection within 200 ms —
/// covers AWS EC2, GCP Compute, Azure VM, OCI, DigitalOcean droplets
/// (all expose 169.254.169.254:80). The probe never sends an HTTP request
/// — just a TCP handshake — so it doesn't itself trigger anything
/// IMDSv2 audit logs would flag.
fn cloud_imds_reachable() -> bool {
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;
    let addr: SocketAddr = ([169, 254, 169, 254], 80).into();
    TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok()
}
