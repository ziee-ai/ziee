//! code_sandbox — bwrap-isolated code execution exposed as a built-in
//! MCP server.
//!
//! Architecture (see `.claude/plans/replicated-enchanting-allen.md`):
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

pub mod cgroup;
pub mod config;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod probes;
pub mod repository;
pub mod routes;
pub mod sandbox;
pub mod tools;
pub mod types;

pub use repository::CodeSandboxRepository;

/// Deterministic UUID for the built-in sandbox MCP server row.
/// Stable across deployments so the same row is hit by every install.
pub fn code_sandbox_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"code-sandbox.ziee.internal")
}

/// Normalize a host string for loopback URL construction.
/// `0.0.0.0`, `::`, empty → `127.0.0.1` (otherwise pass through).
pub fn loopback_host(server_host: &str) -> &str {
    match server_host.trim() {
        "" | "0.0.0.0" | "::" | "[::]" | "0:0:0:0:0:0:0:0" => "127.0.0.1",
        _ => server_host,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_host_normalizes_wildcards() {
        assert_eq!(loopback_host("0.0.0.0"), "127.0.0.1");
        assert_eq!(loopback_host("::"), "127.0.0.1");
        assert_eq!(loopback_host("[::]"), "127.0.0.1");
        assert_eq!(loopback_host("0:0:0:0:0:0:0:0"), "127.0.0.1");
        assert_eq!(loopback_host(""), "127.0.0.1");
        assert_eq!(loopback_host("  "), "127.0.0.1");
    }

    #[test]
    fn loopback_host_passes_through_concrete_addresses() {
        assert_eq!(loopback_host("127.0.0.1"), "127.0.0.1");
        assert_eq!(loopback_host("10.0.0.5"), "10.0.0.5");
        assert_eq!(loopback_host("example.local"), "example.local");
        assert_eq!(loopback_host("[2001:db8::1]"), "[2001:db8::1]");
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

        // ---- Boot probes (run ONCE; cached in CodeSandboxState.caps) ----
        let caps = probes::probe_all(&cfg);

        // Refuse to enable if no working PID-ns mode (rootfs missing,
        // bwrap missing, or both probes failed).
        if matches!(caps.pid_namespace, types::PidNsMode::Disabled) {
            tracing::error!(
                "code_sandbox: enabled in config but boot probes failed; \
                 the sandbox MCP row will NOT be registered. Install bwrap + \
                 mount the rootfs at {}, then restart.",
                cfg.rootfs_path
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
            caps,
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
                let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                let age = SystemTime::now()
                    .duration_since(mtime)
                    .unwrap_or(Duration::ZERO);
                if age > MAX_AGE {
                    match std::fs::remove_dir_all(&path) {
                        Ok(()) => tracing::info!(
                            "code_sandbox: reaped stale workspace {}",
                            path.display()
                        ),
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
