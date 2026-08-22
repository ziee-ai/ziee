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

// ── STAY (ziee server): DB/HTTP halves + the guest-agent staging bodies (their
//    `include_bytes!` reads the SERVER `CARGO_MANIFEST_DIR`) + the provider impls.
pub mod embedded;
#[cfg(target_os = "windows")]
pub mod wsl2_agent_embedded;
pub mod handlers;
pub mod providers;
pub mod runtime_fetch;
pub mod runtime_mount;
pub mod mount_context_extension;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod streaming;
pub mod tools;
pub mod version_back;
pub mod version_handlers;
pub mod version_install_tasks;
pub mod version_manager;

// ── Engine carve: the build-DB-free sandbox ENGINE moved to
//    `ziee_sandbox` (`sdk/crates/ziee-sandbox`). Re-export its modules as
//    equivalence-preserving shims (the ziee-hardware precedent) so every
//    retained `crate::modules::code_sandbox::{sandbox,types,config,…}::…` +
//    `super::{sandbox,types,…}::…` path in the STAY halves resolves unchanged.
#[allow(unused_imports)]
pub use ziee_sandbox::{
    backend, cgroup, config, mcp_spawn, models, mount_provider, probes, provider, registry,
    resource_limits, resource_limits_cache, sandbox, sandbox_config, types, workflow_staging,
};

pub use repository::CodeSandboxRepository;

/// Deterministic UUID for the built-in sandbox MCP server row.
/// Stable across deployments so the same row is hit by every install.
pub fn code_sandbox_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"code-sandbox.ziee.internal")
}

// Chunk C1: `loopback_host` (the security-critical self-dial pin) moved to
// `ziee_framework::mcp` and is re-exported here so every
// `code_sandbox::loopback_host(...)` caller across the built-in MCP servers
// (15 modules) resolves unchanged (decision N2 shim).
pub use ziee_framework::mcp::loopback_host;

// =====================================================================
// Rootfs-flavor allow-list — ONE canonical check
// =====================================================================
//
// `KNOWN_FLAVORS` is the catalog this binary advertises (it lives in the
// `ziee-sandbox` engine crate and reaches us through the `types` shim above).
// Several surfaces need to ask "is this flavor real?", and each used to
// re-derive its own `.iter().any(…)` scan — which is exactly how the next one
// drifts, and how the two MODEL-FACING tool schemas came to advertise
// `"enum": ["minimal","full"]` while enforcing nothing at all.
//
// Only the PREDICATE is shared. Each caller still builds its own error, because
// their contracts differ and are pinned by tests (`INVALID_FLAVOR`/400 for MCP
// server create, `MCP_UNKNOWN_FLAVOR`/422 for the user policy).

/// Is `flavor` one of the rootfs flavors this binary advertises?
///
/// Matches on NAME only — the size/description fields of `FlavorMetadata` are
/// presentation, not identity.
pub fn is_known_flavor(flavor: &str) -> bool {
    types::KNOWN_FLAVORS.iter().any(|m| m.flavor == flavor)
}

/// The advertised flavor names, in advertisement order.
pub fn known_flavor_names() -> Vec<&'static str> {
    types::KNOWN_FLAVORS.iter().map(|m| m.flavor).collect()
}

/// The sandbox environment used when a model-facing tool call supplies no
/// `flavor`.
///
/// ONE definition for every entry point (the chat `execute_command`, the
/// background `sandbox_exec`). It previously existed as three separate literals
/// that nothing kept in agreement. `default_flavor_is_in_the_catalog` pins it to
/// `KNOWN_FLAVORS`, so it cannot silently become an unknown flavor — the
/// no-flavor-supplied path is the most travelled one, and an unvalidated default
/// there would rebuild the very defect this module closes.
pub const DEFAULT_TOOL_FLAVOR: &str = "minimal";

/// Enforce the `flavor` enum that a MODEL-FACING tool schema advertises.
///
/// `arg` names the argument as the model sends it (`flavor` for the chat
/// `execute_command`, `spec.flavor` for `spawn_background`); `example` is a
/// literal-JSON snippet the model can copy. The refusal therefore carries all
/// three things a model needs to fix the call: the argument, what is expected,
/// and a copyable example.
///
/// **Why here and not inside `version_manager::install_version`.** That function
/// serves two callers with different contracts. The ADMIN install path
/// (`version_handlers.rs`) validates `flavor` as a safe token rather than against
/// `KNOWN_FLAVORS`, so an operator can DOWNLOAD a flavor published after this
/// binary was built. Be precise about what that buys: the enum lives in the tool
/// schema, so a flavor outside `KNOWN_FLAVORS` is one the model is never told
/// about and — after this check — cannot invoke either. Installing it stages the
/// artifact for a future binary that knows about it; it does not make it usable
/// from a tool call today. Pushing the check down into `install_version` would
/// additionally break the staging, which is why it lives here instead.
pub fn validate_known_flavor(
    arg: &str,
    flavor: &str,
    example: &str,
) -> Result<(), crate::common::AppError> {
    if is_known_flavor(flavor) {
        return Ok(());
    }
    Err(crate::common::AppError::bad_request(
        "SANDBOX_UNKNOWN_FLAVOR",
        format!(
            "`{arg}` was `{received}`, but it must be one of {names}. Example: {example}",
            received = crate::common::tool_args::truncate_for_message(flavor),
            names = quoted_flavor_names(),
        ),
    ))
}

/// Resolve a model-supplied `flavor` argument for ANY model-facing sandbox tool.
///
/// The single resolver behind both entry points — the chat `execute_command`'s
/// top-level `flavor` and `spawn_background{sandbox_exec}`'s `spec.flavor`. They
/// were briefly two near-identical copies differing only in the argument name and
/// the example; sharing the predicate but not the resolver is how the two drift.
///
/// * absent / explicit `null` → [`DEFAULT_TOOL_FLAVOR`] (the unchanged default).
/// * a supplied string → trimmed and held to the advertised enum. An empty or
///   whitespace-only string is REFUSED, not defaulted: it was supplied.
/// * a supplied non-string → refused, naming what arrived.
///
/// Pure: no state, no I/O, no DB. It is the whole contract, unit-testable
/// without a sandbox, a rootfs, or a network.
pub fn resolve_tool_flavor(
    value: Option<&serde_json::Value>,
    arg: &str,
    example: &str,
) -> Result<String, crate::common::AppError> {
    match value {
        None | Some(serde_json::Value::Null) => Ok(DEFAULT_TOOL_FLAVOR.to_string()),
        Some(serde_json::Value::String(s)) => {
            let s = s.trim();
            validate_known_flavor(arg, s, example)?;
            Ok(s.to_string())
        }
        Some(other) => Err(crate::common::AppError::bad_request(
            "SANDBOX_UNKNOWN_FLAVOR",
            format!(
                "`{arg}` arrived as {received}, but a string naming the sandbox \
                 environment is required — one of {names}. Example: {example}",
                received = crate::common::tool_args::type_word(other),
                names = quoted_flavor_names(),
            ),
        )),
    }
}

/// `` `minimal`, `full` `` — the advertised names, quoted, for a refusal.
pub fn quoted_flavor_names() -> String {
    known_flavor_names()
        .iter()
        .map(|n| format!("`{n}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    // The two `loopback_host` tests moved with the function to
    // `ziee_framework::mcp` (Chunk C1).

    // TEST-9 [acceptance] [invariant: INV-3]: the canonical allow-list accepts
    // EXACTLY what `KNOWN_FLAVORS` advertises and refuses everything else.
    //
    // The accepted set is DERIVED from the const rather than hardcoded, so a
    // flavor added to the catalog tomorrow is covered by this test the moment it
    // lands — a hardcoded `["minimal","full"]` here would quietly stop testing
    // the real list.
    #[test]
    fn known_flavor_allow_list_matches_the_advertised_catalog() {
        let advertised = known_flavor_names();
        assert!(!advertised.is_empty(), "the catalog must advertise at least one flavor");
        for name in &advertised {
            assert!(is_known_flavor(name), "the advertised flavor `{name}` must be accepted");
            assert!(
                validate_known_flavor("flavor", name, "EXAMPLE").is_ok(),
                "…and must not be refused: `{name}`"
            );
        }

        // Everything else is refused: the invented value from the live rig, an
        // empty string, a case variant (the catalog is case-SENSITIVE — a
        // lookalike must not slip through), a whitespace-padded name, and a
        // traversal-shaped value that would otherwise land in a download URL.
        let first = advertised[0];
        for bad in [
            "zee-workflow",
            "",
            " ",
            &first.to_ascii_uppercase(),
            &format!(" {first} "),
            "../../etc",
            "minimal;rm -rf /",
        ] {
            assert!(!is_known_flavor(bad), "`{bad}` must NOT be a known flavor");
            let msg = validate_known_flavor("spec.flavor", bad, "EXAMPLE")
                .expect_err("an unknown flavor must be refused")
                .to_string();
            assert!(msg.contains("spec.flavor"), "the refusal names the argument: {msg}");
            for name in &advertised {
                assert!(msg.contains(name), "…and enumerates the valid flavors: {msg}");
            }
            assert!(msg.contains("EXAMPLE"), "…and carries the copyable example: {msg}");
        }
    }

    // TEST-17: the delegation in `mcp/user_policy/repository.rs` must not change
    // the `MCP_UNKNOWN_FLAVOR` message an admin sees. That site renders the name
    // list with `{names:?}`, so pin the rendering of what it now receives.
    #[test]
    fn known_flavor_names_render_identically_for_the_user_policy_message() {
        let names = known_flavor_names();
        // The LITERAL string the pre-refactor site produced. Deriving the
        // expectation from `known_flavor_names`' own body — which an earlier
        // revision of this test did — is a tautology: it cannot fail for ANY
        // edit to the function under test, which is the one thing it exists to
        // protect.
        assert_eq!(
            format!("{names:?}"),
            r#"["minimal", "full"]"#,
            "the user-policy MCP_UNKNOWN_FLAVOR message renders this list with \
             `{{names:?}}`; a change here silently changes an admin-facing error"
        );
        // Identity is the NAME only — the size/description fields are
        // presentation and must not leak into the allow-list decision.
        assert!(is_known_flavor(types::KNOWN_FLAVORS[0].flavor));
        assert!(!is_known_flavor(types::KNOWN_FLAVORS[0].description));
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

        // Windows: ensure the LocalSystem sandbox-helper service is installed
        // on first run, BEFORE the `enabled` check — the user can turn the
        // sandbox on later without restarting, so we set this up regardless
        // of the current config. The helper lets the unprivileged server
        // resolve the WSL utility-VM id + register the vsock GUIDs WITHOUT
        // Hyper-V Administrators membership (and with no log-out/in). The
        // command is self-checking + self-elevating: a silent no-op once
        // installed, one UAC prompt the first time the app runs. Runs for
        // both the standalone `ziee` binary and the Tauri-embedded server
        // (both hit this module init). Windows is always interactive (GUI
        // install — no headless Windows deployment), so a boot-time UAC is
        // fine. Best-effort: a declined prompt just defers the failure to the
        // first sandboxed exec, which surfaces the same install instruction.
        // Skipped when `ZIEE_WSL_VM_ID` is set (the dev/test bypass that needs
        // no helper service).
        #[cfg(windows)]
        if std::env::var("ZIEE_WSL_VM_ID").is_err() {
            match backend::helper_service::install::install(false) {
                Ok(()) => {
                    tracing::info!("code_sandbox: sandbox-helper service ready")
                }
                Err(e) => tracing::warn!(
                    "code_sandbox: sandbox-helper auto-install skipped ({e}); \
                     run `ziee --install-sandbox-helper` as Administrator if \
                     sandboxed execution fails"
                ),
            }
        }

        let cfg = crate::module_api::app_config(ctx)
            .code_sandbox
            .clone()
            .unwrap_or_default();
        if !cfg.enabled {
            config::set_init_status(config::SandboxAvailability::DisabledInConfig);
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
            None => {
                // The backend logs its own "skipping registration" reason
                // (Linux: bwrap missing). Record it so the admin list can say so.
                config::set_init_status(config::SandboxAvailability::HostUnsupported);
                return Ok(());
            }
        };

        // Audit H-4: if the cloud instance metadata service is reachable
        // from the host, `--share-net` (in build_bwrap_argv) would expose it
        // to LLM-generated code — and IMDS hands out IAM credentials.
        // Refuse to register unless the operator has explicitly opted in
        // via `allow_cloud_imds_reachable: true`. Cheap host-only probe:
        // 200ms connect-timeout against 169.254.169.254:80.
        if !cfg.allow_cloud_imds_reachable && cloud_imds_reachable() {
            config::set_init_status(config::SandboxAvailability::CloudImdsRefused);
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
            config::set_init_status(config::SandboxAvailability::WorkspaceInitFailed);
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

        // Engine carve: the de-`pool`-ed engine state now carries the three
        // injected provider seams (`crate::modules::code_sandbox::providers`),
        // which hold the DB pool + boot-probed host caps + resolved config and
        // delegate back to the retained `runtime_mount`/`runtime_fetch`/
        // `repository`/`embedded` halves.
        let pool = ctx.db_pool.clone();
        let state = types::CodeSandboxState {
            config: cfg.clone(),
            loopback_url: loopback_url.clone(),
            workspace_root: workspace_root.clone(),
            host_caps: host_caps.clone(),
            rootfs: Arc::new(providers::ZieeRootfsProvider {
                pool: pool.clone(),
                host_caps: host_caps.clone(),
                config: cfg.clone(),
            }),
            limits: Arc::new(providers::ZieeResourceLimitsProvider { pool: pool.clone() }),
            guest_agent: Arc::new(providers::ZieeGuestAgentProvider),
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

        config::set_init_status(config::SandboxAvailability::Ready);
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

/// Cascade fs cleanup: remove a conversation's sandbox workspace dir on
/// conversation delete (instead of waiting for the 30d reaper) and drop its
/// in-memory lock entry. Best-effort + idempotent.
pub fn cleanup_conversation_workspace(conversation_id: uuid::Uuid) {
    let dir = crate::core::get_app_data_dir()
        .join("sandboxes")
        .join(conversation_id.to_string());
    if dir.exists()
        && let Err(e) = std::fs::remove_dir_all(&dir)
    {
        tracing::warn!(
            "code_sandbox: failed to clean workspace {} on conversation delete: {e}",
            dir.display()
        );
    }
    handlers::prune_conversation_lock(conversation_id);
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
