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
pub mod handlers;
pub mod prefetch;
pub mod runtime_fetch;
pub mod runtime_mount;
pub mod models;
pub mod permissions;
pub mod probes;
pub mod repository;
pub mod routes;
pub mod sandbox;
pub mod streaming;
pub mod tools;
pub mod types;

/// Embedded rootfs ↔ server compat matrix (single source of truth lives
/// in `src-app/sandbox-rootfs/compat.toml`). The server include_str!s it
/// at compile time so a server build is locked to the schema knowledge
/// it was built with.
pub const SANDBOX_COMPAT_TOML: &str =
    include_str!("../../../../sandbox-rootfs/compat.toml");

/// Embedded yanked-revision catalog. See `src-app/sandbox-rootfs/yanks.toml`.
pub const SANDBOX_YANKS_TOML: &str =
    include_str!("../../../../sandbox-rootfs/yanks.toml");

/// Embedded known-revision sha256 map. Populated by the post-release
/// PR-bot workflow; until then it's an empty scaffold and `fetch`
/// callers must supply `--sha256` explicitly.
pub const SANDBOX_KNOWN_REVISIONS_TOML: &str = include_str!("known_revisions.toml");

/// Schema version this server binary expects from the mounted rootfs.
/// Bumped only on ABI-breaking rootfs changes (Python major bump,
/// binary path changes, layout changes). Mirrors `current_schema` in
/// `src-app/sandbox-rootfs/compat.toml`.
pub const SANDBOX_ROOTFS_SCHEMA_VERSION: u32 = 1;

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

/// Read the schema-version sentinel inside the mounted rootfs.
/// The file lives at `<rootfs>/.ziee-sandbox-rootfs-schema` and
/// contains a single decimal integer (e.g. `1`). Whitespace is
/// trimmed.
///
/// Returns `Err` if the file is missing, is a symlink, exceeds the
/// size cap, or its content is not a base-10 u32.
///
/// SECURITY: rejects symlinks AND caps read size at 64 bytes. The
/// rootfs path is operator-configurable; a misconfig (or a stale
/// unmount that exposes a host dir) could place a symlink at the
/// sentinel pointing at `/dev/zero` (infinite read → boot hang) or
/// `/proc/kcore` (multi-GB allocation → boot OOM). Bounded read +
/// no-follow defeats both.
pub fn probe_rootfs_schema(rootfs_path: &str) -> Result<u32, String> {
    use std::io::Read;
    let sentinel = std::path::Path::new(rootfs_path).join(".ziee-sandbox-rootfs-schema");
    // Reject symlinks before opening — `read_to_string` follows them.
    match std::fs::symlink_metadata(&sentinel) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(format!(
                    "{} is a symlink; refusing to follow",
                    sentinel.display()
                ));
            }
            // Bound size BEFORE reading. A sentinel that's anything
            // other than ~5 bytes is corrupt or hostile.
            if meta.len() > 64 {
                return Err(format!(
                    "{} is {} bytes; refusing (cap 64)",
                    sentinel.display(),
                    meta.len()
                ));
            }
        }
        Err(e) => return Err(format!("stat {}: {e}", sentinel.display())),
    }
    let mut f = std::fs::File::open(&sentinel)
        .map_err(|e| format!("open {}: {e}", sentinel.display()))?;
    // Read into a tiny buffer so even if the metadata check above
    // was racing a symlink swap, we still cap the read.
    let mut buf = String::new();
    f.take(64).read_to_string(&mut buf)
        .map_err(|e| format!("read {}: {e}", sentinel.display()))?;
    buf.trim()
        .parse::<u32>()
        .map_err(|e| format!("parse schema sentinel {:?}: {e}", buf.trim()))
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

    // ─── rootfs schema-version probe ────────────────────────────────

    #[test]
    fn probe_rootfs_schema_reads_sentinel() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".ziee-sandbox-rootfs-schema"), "1\n").unwrap();
        let got = probe_rootfs_schema(dir.path().to_str().unwrap()).expect("read");
        assert_eq!(got, 1);
    }

    #[test]
    fn probe_rootfs_schema_trims_whitespace() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".ziee-sandbox-rootfs-schema"), "  42  \n\n").unwrap();
        let got = probe_rootfs_schema(dir.path().to_str().unwrap()).expect("read");
        assert_eq!(got, 42);
    }

    #[test]
    fn probe_rootfs_schema_errors_on_missing_sentinel() {
        let dir = tempfile::tempdir().unwrap();
        // No sentinel file written.
        let err =
            probe_rootfs_schema(dir.path().to_str().unwrap()).expect_err("must error");
        // "stat" (no such file) is the first thing we try; "open" /
        // "read" are also valid if stat passed but later steps fail.
        assert!(
            err.contains("stat") || err.contains("open") || err.contains("read"),
            "err: {err}"
        );
    }

    #[test]
    fn probe_rootfs_schema_errors_on_malformed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".ziee-sandbox-rootfs-schema"), "not-a-number").unwrap();
        let err =
            probe_rootfs_schema(dir.path().to_str().unwrap()).expect_err("must error");
        assert!(err.contains("parse"), "err: {err}");
    }

    #[test]
    #[cfg(unix)]
    fn probe_rootfs_schema_rejects_symlink_sentinel() {
        // SECURITY regression test: a misconfigured rootfs path could
        // expose a symlink to /dev/zero or /proc/kcore at the sentinel,
        // which `read_to_string` would happily follow.
        let dir = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(
            "/etc/hostname", // any existing file; we just need a symlink
            dir.path().join(".ziee-sandbox-rootfs-schema"),
        ).unwrap();
        let err =
            probe_rootfs_schema(dir.path().to_str().unwrap()).expect_err("must reject");
        assert!(err.contains("symlink"), "err: {err}");
    }

    #[test]
    fn probe_rootfs_schema_rejects_huge_sentinel() {
        let dir = tempfile::tempdir().unwrap();
        // 65 bytes of '1' — just over the 64-byte cap.
        std::fs::write(
            dir.path().join(".ziee-sandbox-rootfs-schema"),
            "1".repeat(65),
        ).unwrap();
        let err =
            probe_rootfs_schema(dir.path().to_str().unwrap()).expect_err("must reject");
        assert!(err.contains("cap") || err.contains("64"), "err: {err}");
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
            host_caps,
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
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name == "attachments" || name == "identity" {
                        continue;
                    }
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
