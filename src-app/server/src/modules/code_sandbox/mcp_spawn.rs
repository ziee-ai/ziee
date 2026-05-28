//! High-level entry point for spawning a stdio MCP subprocess inside
//! the code_sandbox bwrap isolation. Returns a transport opaque to the
//! caller (`McpSandboxTransport`); the MCP client wires it into rmcp.
//!
//! Two paths converge here:
//! - **Linux** — spawn `bwrap` directly on the host with the MCP argv
//!   built by `sandbox::build_mcp_sandbox_argv`. Wrapped in rmcp's
//!   `TokioChildProcess` (handles kill-on-drop + stdio piping).
//! - **macOS / Windows** — ask the active `SandboxBackend` for a
//!   long-lived agent session; spawn one bwrap process inside the
//!   guest VM via that session; expose the per-process duplex stream
//!   as `AsyncRead + AsyncWrite` so rmcp can wrap it in
//!   `AsyncRwTransport::new_client`.
//!
//! Both paths use the *same* `build_mcp_sandbox_argv` helper, only the
//! "where bwrap runs" differs. Hardening (clearenv / user-ns / seccomp
//! pipe / cgroup / prlimit / dotfile masks / per-server workspace
//! bind) is identical.

use std::path::PathBuf;

use rmcp::transport::TokioChildProcess;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::code_sandbox::backend;
use crate::modules::code_sandbox::backend::vm_long_lived;
use crate::modules::code_sandbox::sandbox;
use crate::modules::code_sandbox::types::CodeSandboxState;

/// Result of a successful sandboxed MCP spawn. The variant carries the
/// platform-specific lifecycle owner so dropping `McpSandboxTransport`
/// tears the sandboxed child down.
pub enum McpSandboxTransport {
    /// Linux: already-spawned bwrap subprocess, ready to feed into
    /// rmcp via `().serve(transport)`. `TokioChildProcess` owns the
    /// `Child` and kills it on drop.
    LinuxBwrap(TokioChildProcess),
    /// VM backend: long-lived agent session + the per-process duplex
    /// stream. The caller splits `io` into halves and passes them to
    /// `rmcp::transport::AsyncRwTransport::new_client(read, write)`.
    /// Drop order matters — `io` must drop BEFORE `session` so the
    /// `KillProcess` frame still has a writer to go through.
    VmSession {
        io: vm_long_lived::ProcessIo,
        session: vm_long_lived::LongLivedSession,
    },
}

/// The flavor we pin MCP-in-sandbox to in v1. Future work: surface a
/// per-server flavor selector once the full rootfs is wired in (right
/// now there's no UI for it and 'minimal' is the only safe default).
pub(crate) const DEFAULT_MCP_FLAVOR: &str = "minimal";

/// Plumbing for one sandboxed MCP spawn. The caller (typically
/// `mcp::client::stdio::StdioMcpClient::connect`) supplies the
/// already-resolved command + args; this layer handles rootfs / argv
/// construction / spawn.
pub struct McpSpawnRequest {
    /// Stable id of the MCP server row this spawn belongs to. Used to
    /// derive a unique per-server workspace under
    /// `<workspace_root>/mcp/<server_id>/`.
    pub server_id: Uuid,
    /// Already-resolved command (e.g. the embedded uv binary path for
    /// `uvx`). Path is interpreted on the host (Linux) or in the
    /// guest (mac/win) — see [`McpSandboxTransport`] for which.
    pub resolved_command: PathBuf,
    /// Args to prepend before the user-supplied args (e.g. `["tool",
    /// "run"]` for the `uvx → uv tool run …` resolution).
    pub prepended_args: Vec<String>,
    /// User-supplied args from `mcp_servers.args`.
    pub server_args: Vec<String>,
    /// Already-filtered env (BLOCKED_ENV_VARS stripped at the
    /// boundary). Injected as `--setenv` lines in the bwrap argv, NOT
    /// inherited from the host shell.
    pub extra_setenv: Vec<(String, String)>,
}

/// Spawn the MCP server inside the sandbox using whichever backend is
/// active. Returns an `McpSandboxTransport` the caller hands to rmcp.
pub async fn start_mcp_in_sandbox(
    state: &CodeSandboxState,
    req: McpSpawnRequest,
) -> Result<McpSandboxTransport, AppError> {
    // Backends opt in by overriding `open_long_lived_session`; Linux
    // inherits the default `Ok(None)` and falls through to the
    // host-bwrap path below.
    if let Some(session) = backend::active()
        .open_long_lived_session(state, DEFAULT_MCP_FLAVOR)
        .await?
    {
        return spawn_in_vm_session(state, req, session).await;
    }

    #[cfg(target_os = "linux")]
    {
        return spawn_on_linux_host(state, req).await;
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (state, req);
        Err(AppError::internal_error(
            "MCP run-in-sandbox: no backend available on this host",
        ))
    }
}

/// VM-backend spawn. Builds the bwrap argv with **guest paths** and
/// sends it through the long-lived agent session as a `StartProcess`.
/// Returns the [`McpSandboxTransport::VmSession`] holding the session
/// alive and the per-process duplex stream.
async fn spawn_in_vm_session(
    _state: &CodeSandboxState,
    req: McpSpawnRequest,
    session: vm_long_lived::LongLivedSession,
) -> Result<McpSandboxTransport, AppError> {
    // The VM backends (mac libkrun, WSL2) intentionally re-bind the
    // *guest* uv/bun isn't a thing for v1 — the rootfs supplies the
    // runtime (python3 ships in 'minimal'; node/uv require a future
    // rootfs flavor or a virtio-fs bind that's not in this PR).
    // Surface this loudly so an operator who toggles `run_in_sandbox`
    // on an npx/uvx/node/deno server learns the v1 limitation instead
    // of getting a cryptic "bwrap: exec: not found" inside the VM.
    let resolved = req.resolved_command.to_string_lossy();
    if !resolved.contains("python") {
        return Err(AppError::internal_error(format!(
            "MCP run-in-sandbox VM path (macOS / Windows): only python-based \
             servers are supported in v1. Command '{}' is not yet available \
             inside the sandbox VM rootfs. Run on Linux for full support, or \
             switch this server to a python-based MCP package.",
            resolved
        )));
    }

    // Build the bwrap argv for the guest. Hardcoded guest paths come
    // from the agent's mount contract (sandbox-guest-agent/src/main.rs).
    let guest_argv = build_guest_mcp_argv(&req)?;
    let bwrap_path = "/usr/bin/bwrap".to_string();

    let io = session
        .spawn(bwrap_path, guest_argv, None, None)
        .await?;
    Ok(McpSandboxTransport::VmSession { io, session })
}

/// Build the guest-side bwrap argv for a MCP spawn. Keeps a separate
/// (minimal, hand-rolled) argv from the Linux host path because:
///   - There's no `HardeningCapabilities` snapshot for the guest at
///     this point in the lifecycle (no plumbed `runtime_mount` on the
///     VM backends; the agent does its own mounts).
///   - The guest doesn't have an embedded uv/bun bind to plumb in.
///   - prlimit + the rootfs binds match the agent's
///     `build_bwrap_argv`-issued one-shot path.
fn build_guest_mcp_argv(req: &McpSpawnRequest) -> Result<Vec<String>, AppError> {
    let resolved = req.resolved_command.to_string_lossy().to_string();
    let mut argv: Vec<String> = vec![
        "--clearenv".into(),
        "--unshare-user".into(),
        "--uid".into(), "1001".into(),
        "--gid".into(), "1001".into(),
        "--unshare-uts".into(),
        "--unshare-ipc".into(),
        "--unshare-pid".into(),
        "--unshare-cgroup-try".into(),
        "--share-net".into(),
        "--die-with-parent".into(),
        "--new-session".into(),
        "--ro-bind".into(), "/sandbox-rootfs/usr".into(), "/usr".into(),
        "--symlink".into(), "usr/bin".into(), "/bin".into(),
        "--symlink".into(), "usr/sbin".into(), "/sbin".into(),
        "--symlink".into(), "usr/lib".into(), "/lib".into(),
        "--symlink".into(), "usr/lib64".into(), "/lib64".into(),
        "--dev".into(), "/dev".into(),
        "--tmpfs".into(), "/tmp".into(),
        "--proc".into(), "/proc".into(),
        "--bind".into(), format!("/workspace/mcp/{}", req.server_id), "/home/sandboxuser".into(),
        "--chdir".into(), "/home/sandboxuser".into(),
        "--setenv".into(), "HOME".into(), "/home/sandboxuser".into(),
        "--setenv".into(), "USER".into(), "sandboxuser".into(),
        "--setenv".into(), "PATH".into(),
        "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".into(),
        "--setenv".into(), "LANG".into(), "C.UTF-8".into(),
        "--setenv".into(), "LC_ALL".into(), "C.UTF-8".into(),
    ];
    for (k, v) in &req.extra_setenv {
        argv.push("--setenv".into());
        argv.push(k.clone());
        argv.push(v.clone());
    }
    argv.push("--".into());
    argv.push(resolved);
    for a in &req.prepended_args { argv.push(a.clone()); }
    for a in &req.server_args { argv.push(a.clone()); }
    Ok(argv)
}

#[cfg(target_os = "linux")]
async fn spawn_on_linux_host(
    state: &CodeSandboxState,
    req: McpSpawnRequest,
) -> Result<McpSandboxTransport, AppError> {
    use std::os::unix::process::CommandExt;
    use std::path::Path;
    use std::process::Stdio;
    use tokio::process::Command;

    use crate::modules::code_sandbox::resource_limits_cache;
    use crate::modules::code_sandbox::runtime_mount;
    use crate::modules::code_sandbox::sandbox::{
        build_mcp_sandbox_argv, HardeningArgvParams, SyntheticIdentity,
    };
    use crate::modules::code_sandbox::types::SeccompMode;

    // Lazy-mount the flavor's rootfs (cheap on cache hit).
    let ensure = runtime_mount::ensure_rootfs_ready(state, DEFAULT_MCP_FLAVOR).await?;
    let caps = ensure.caps.clone();
    let rootfs_dir = ensure.mount_dir;

    let synthetic = SyntheticIdentity::ensure_for(&state.workspace_root)?;

    // Per-server workspace: <workspace_root>/mcp/<server_id>/
    let server_workspace = state
        .workspace_root
        .join("mcp")
        .join(req.server_id.to_string());
    std::fs::create_dir_all(&server_workspace).map_err(|e| {
        AppError::internal_error(format!("create mcp server workspace: {e}"))
    })?;

    // Bind the embedded uv/bun parent dir (host abs path) into the
    // sandbox at the same absolute path so `resolved_command` resolves
    // both inside the sandbox and outside. We bind the *parent*
    // directory of the resolved binary, RW would be wrong; --ro-bind-try
    // is fine (`build_hardening_prefix` emits ro-bind-try entries).
    let mut extra_ro_binds: Vec<(String, String)> = Vec::new();
    if let Some(parent) = req.resolved_command.parent() {
        let p = parent.to_string_lossy().to_string();
        extra_ro_binds.push((p.clone(), p));
    }

    let limits = resource_limits_cache::get().await?;

    // Per-spawn seccomp pipe (Linux only; piped to the child fd 7).
    // Drop the pipe AFTER spawn — until then the pre_exec dup2 needs
    // the read fd, and the writer task pumps the bytes.
    let seccomp_pipe = match &caps.seccomp {
        SeccompMode::Loaded(bpf) => Some(sandbox::SeccompPipe::install_pub(bpf.clone())?),
        _ => None,
    };

    let argv = build_mcp_sandbox_argv(
        &HardeningArgvParams {
            caps: &caps,
            rootfs_dir: &rootfs_dir,
            passwd_path: synthetic.passwd(),
            group_path: synthetic.group(),
            mask_path: synthetic.mask(),
            home_bind_source: &server_workspace,
            seccomp_fd: seccomp_pipe.as_ref().map(|p| p.target_fd_pub()),
            extra_setenv: &req.extra_setenv,
            extra_ro_binds: &extra_ro_binds,
        },
        Path::new(&req.resolved_command),
        &{
            let mut all = req.prepended_args.clone();
            all.extend(req.server_args.clone());
            all
        },
        &limits,
    );

    let mut cmd = Command::new(&caps.bwrap_path);
    cmd.args(&argv);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);

    if let Some(p) = seccomp_pipe.as_ref() {
        let source_fd = p.read_fd_pub();
        let target_fd = p.target_fd_pub();
        // SAFETY: dup2 + fcntl are async-signal-safe; source_fd is
        // owned by `SeccompPipe` and stays valid through spawn.
        unsafe {
            cmd.pre_exec(move || {
                if libc::dup2(source_fd, target_fd) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                let flags = libc::fcntl(target_fd, libc::F_GETFD);
                if flags < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::fcntl(target_fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    let child = TokioChildProcess::new(cmd).map_err(|e| {
        AppError::internal_error(format!("spawn bwrap for MCP server: {e}"))
    })?;
    // Spawn done — child has its own dup of read_fd; drop ours.
    drop(seccomp_pipe);

    Ok(McpSandboxTransport::LinuxBwrap(child))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guest_mcp_argv_emits_clearenv_and_workspace_bind() {
        let req = McpSpawnRequest {
            server_id: Uuid::nil(),
            resolved_command: PathBuf::from("/usr/bin/python3"),
            prepended_args: vec!["-m".into(), "mymcp".into()],
            server_args: vec![],
            extra_setenv: vec![("MY_VAR".into(), "yes".into())],
        };
        let argv = build_guest_mcp_argv(&req).unwrap();

        assert!(argv.contains(&"--clearenv".to_string()));
        assert!(argv.contains(&"--die-with-parent".to_string()));
        assert!(argv.contains(&"--unshare-user".to_string()));
        assert!(argv.contains(&format!("/workspace/mcp/{}", Uuid::nil())));
        // env vars layered after the static block
        assert!(argv.windows(3).any(|w| w == ["--setenv", "MY_VAR", "yes"]));
        // resolved command appended after `--`
        let dashdash = argv.iter().position(|s| s == "--").expect("-- terminator");
        assert_eq!(argv[dashdash + 1], "/usr/bin/python3");
        assert_eq!(argv[dashdash + 2], "-m");
        assert_eq!(argv[dashdash + 3], "mymcp");
    }

    #[test]
    fn guest_mcp_argv_pid_namespace_is_strict() {
        // We must --unshare-pid AND --proc /proc inside the VM; the
        // VM has no fallback because the guest kernel always supports
        // PID namespaces. A regression here would leak host PIDs to
        // the sandboxed child.
        let req = McpSpawnRequest {
            server_id: Uuid::nil(),
            resolved_command: PathBuf::from("/usr/bin/python3"),
            prepended_args: vec![],
            server_args: vec![],
            extra_setenv: vec![],
        };
        let argv = build_guest_mcp_argv(&req).unwrap();
        assert!(argv.contains(&"--unshare-pid".to_string()));
        // --proc immediately followed by /proc
        let i = argv.iter().position(|s| s == "--proc").unwrap();
        assert_eq!(argv[i + 1], "/proc");
    }
}
