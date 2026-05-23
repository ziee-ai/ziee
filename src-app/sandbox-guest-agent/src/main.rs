//! `ziee-sandbox-agent` — the in-VM init + control agent for the microVM
//! sandbox backend (macOS libkrun; later WSL2).
//!
//! libkrun runs this as the guest entrypoint (`krun_set_exec`). Its job:
//!   1. Mount the pieces the host wired in: `/proc`, the sandbox **squashfs**
//!      (added by the host via `krun_add_disk`, appears as a virtio-blk device),
//!      and the **workspace** (shared via `krun_add_virtiofs`).
//!   2. Listen on a vsock port (libkrun bridges it to a unix socket on the
//!      macOS host) and, per connection, run the host-supplied **bwrap argv**
//!      (built by `build_bwrap_argv` with guest paths) and stream stdout/stderr
//!      + the exit status back over `sandbox-vm-protocol` frames.
//!
//! The agent is a *dumb executor*: all hardening lives in the argv the host
//! sends, so `build_bwrap_argv` stays the single source of truth across
//! Linux/macOS/Windows backends.
//!
//! ## Guest contract (must match the host backend's libkrun configuration)
//!  - vsock port [`VSOCK_PORT`] — agent listens; host connects via the bridge.
//!  - [`ROOTFS_DEVICE`] — the squashfs block device; mounted read-only at
//!    [`ROOTFS_MOUNT`] (the host passes this as the rootfs dir to
//!    `build_bwrap_argv`, so the argv's `--ro-bind <rootfs>/usr /usr` resolves).
//!  - virtio-fs tag [`WORKSPACE_TAG`] — mounted at [`WORKSPACE_MOUNT`]; the
//!    host points the workspace bind in the argv at a path under it.

use std::sync::Arc;
use std::time::Duration;

use sandbox_vm_protocol::{encode, CgroupLimits, Decoder, ExitStatus, Frame, PROTOCOL_VERSION};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_vsock::{VsockAddr, VsockListener};

/// vsock port the agent listens on. libkrun bridges this to a host unix socket.
const VSOCK_PORT: u32 = 1024;
/// The squashfs disk the host adds via `krun_add_disk` (virtio-blk).
const ROOTFS_DEVICE: &str = "/dev/vda";
/// Where the agent mounts the squashfs; matches the rootfs dir the host passes
/// to `build_bwrap_argv`.
const ROOTFS_MOUNT: &str = "/sandbox-rootfs";
/// virtio-fs tag the host shares the workspace root under.
const WORKSPACE_TAG: &str = "workspace";
const WORKSPACE_MOUNT: &str = "/workspace";

/// Chunk size for streaming child stdout/stderr.
const READ_CHUNK: usize = 64 * 1024;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .init();

    init_mounts();
    cgroup_init();

    // Build the seccomp BPF once from the shared policy crate (identical to the
    // Linux host). Per-exec we pipe these bytes to bwrap's --seccomp fd. If the
    // build fails (a broken guest image), execs that request seccomp fail
    // closed rather than running unfiltered.
    let bpf: Option<Arc<Vec<u8>>> = match sandbox_seccomp::build_bpf() {
        Ok((bytes, unresolved)) => {
            if !unresolved.is_empty() {
                tracing::warn!(?unresolved, "agent: some seccomp DENY entries unresolved on this kernel");
            }
            Some(Arc::new(bytes))
        }
        Err(e) => {
            tracing::error!("agent: seccomp filter build FAILED: {e} — seccomp'd execs will fail closed");
            None
        }
    };

    // The control transport differs per backend (the agent is the single guest
    // executor for both): libkrun (macOS) bridges a vsock port to a host unix
    // socket; WSL2 (Windows) reaches the agent over localhost TCP (WSL2
    // auto-forwards). Default to vsock:1024 so the macOS launcher needs no arg.
    match parse_listen() {
        Listen::Vsock(port) => serve_vsock(port, bpf).await,
        Listen::Tcp(addr) => serve_tcp(&addr, bpf).await,
    }
}

/// Which control transport to listen on.
enum Listen {
    Vsock(u32),
    Tcp(String),
}

/// Parse `--listen vsock:<port>` / `--listen tcp:<addr>` from argv; default
/// `vsock:1024` (macOS/libkrun back-compat).
fn parse_listen() -> Listen {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--listen" {
            if let Some(spec) = args.next() {
                if let Some(port) = spec.strip_prefix("vsock:") {
                    return Listen::Vsock(port.parse().unwrap_or(VSOCK_PORT));
                }
                if let Some(addr) = spec.strip_prefix("tcp:") {
                    return Listen::Tcp(addr.to_string());
                }
            }
        }
    }
    Listen::Vsock(VSOCK_PORT)
}

async fn serve_vsock(port: u32, bpf: Option<Arc<Vec<u8>>>) {
    let listener = match VsockListener::bind(VsockAddr::new(libc::VMADDR_CID_ANY, port)) {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("agent: failed to bind vsock port {port}: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!("ziee-sandbox-agent: listening on vsock port {port}");
    loop {
        match listener.accept().await {
            Ok((stream, peer)) => spawn_conn(stream, &bpf, format!("{peer:?}")),
            Err(e) => {
                tracing::warn!("agent: vsock accept failed: {e}");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

async fn serve_tcp(addr: &str, bpf: Option<Arc<Vec<u8>>>) {
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("agent: failed to bind tcp {addr}: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!("ziee-sandbox-agent: listening on tcp {addr}");
    loop {
        match listener.accept().await {
            Ok((stream, peer)) => spawn_conn(stream, &bpf, format!("{peer}")),
            Err(e) => {
                tracing::warn!("agent: tcp accept failed: {e}");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

fn spawn_conn<S>(stream: S, bpf: &Option<Arc<Vec<u8>>>, peer: String)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    tracing::info!("agent: connection from {peer}");
    let bpf = bpf.clone();
    tokio::spawn(async move {
        if let Err(e) = handle_conn(stream, bpf).await {
            tracing::warn!("agent: connection handler error: {e}");
        }
    });
}

/// Best-effort mounts. Failures are logged, not fatal: an `execute_command`
/// against a missing rootfs returns a clear non-zero exit, which is more useful
/// than the agent refusing to start.
fn init_mounts() {
    mount_fs("proc", "/proc", "proc", 0, None);
    let _ = std::fs::create_dir_all(ROOTFS_MOUNT);
    // Read-only squashfs.
    mount_fs(ROOTFS_DEVICE, ROOTFS_MOUNT, "squashfs", libc::MS_RDONLY, None);
    let _ = std::fs::create_dir_all(WORKSPACE_MOUNT);
    // virtio-fs workspace share (read-write; bwrap re-binds the per-conversation
    // subdir into the sandbox).
    mount_fs(WORKSPACE_TAG, WORKSPACE_MOUNT, "virtiofs", 0, None);
}

fn mount_fs(src: &str, target: &str, fstype: &str, flags: libc::c_ulong, data: Option<&str>) {
    use std::ffi::CString;
    let c_src = CString::new(src).unwrap();
    let c_tgt = CString::new(target).unwrap();
    let c_fs = CString::new(fstype).unwrap();
    let c_data = data.map(|d| CString::new(d).unwrap());
    let data_ptr = c_data
        .as_ref()
        .map(|c| c.as_ptr() as *const libc::c_void)
        .unwrap_or(std::ptr::null());
    // SAFETY: all pointers are valid CStrings living for the call.
    let rc = unsafe { libc::mount(c_src.as_ptr(), c_tgt.as_ptr(), c_fs.as_ptr(), flags, data_ptr) };
    if rc != 0 {
        tracing::warn!(
            "agent: mount {src} -> {target} ({fstype}) failed: {}",
            std::io::Error::last_os_error()
        );
    } else {
        tracing::info!("agent: mounted {src} -> {target} ({fstype})");
    }
}

/// Handle one control connection: read a single `Exec` frame, run bwrap, stream
/// output, send `Exit`.
async fn handle_conn<S>(stream: S, bpf: Option<Arc<Vec<u8>>>) -> std::io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut rd, wr) = tokio::io::split(stream);

    // Read frames until we get the Exec request — or a Shutdown.
    let mut decoder = Decoder::new();
    let mut buf = [0u8; READ_CHUNK];
    let req = loop {
        match decoder.next_frame() {
            Ok(Some(Frame::Exec(req))) => break req,
            Ok(Some(Frame::Shutdown)) => {
                // Clean stop requested by the host (WSL2 backend on
                // distro-evict; macOS could use it too). Exiting this
                // process triggers bwrap's `--die-with-parent` for any
                // in-flight children that other connections were running.
                // Status 0 is the explicit "clean shutdown" signal.
                tracing::info!("agent: shutdown requested by host; exiting");
                std::process::exit(0);
            }
            Ok(Some(other)) => {
                tracing::warn!("agent: ignoring unexpected pre-exec frame: {other:?}");
                continue;
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!("agent: protocol error: {e}");
                return Ok(());
            }
        }
        let n = rd.read(&mut buf).await?;
        if n == 0 {
            return Ok(()); // peer closed before sending a request
        }
        decoder.feed(&buf[..n]);
    };

    // Reject mismatched protocol versions loudly — defends against operators
    // running a stale agent binary against a fresh server (or vice versa).
    // `#[serde(default)]` on `protocol_version` means peers that predate the
    // field send `0`, which never matches the current PROTOCOL_VERSION.
    if req.protocol_version != PROTOCOL_VERSION {
        tracing::error!(
            request_id = req.request_id,
            peer_version = req.protocol_version,
            agent_version = PROTOCOL_VERSION,
            "agent: protocol version mismatch; rejecting"
        );
        return Ok(());
    }

    tracing::info!(
        request_id = req.request_id,
        argv_len = req.argv.len(),
        "agent: running bwrap"
    );

    // Single writer task owns the write half; stdout/stderr readers + the exit
    // funnel frames through this channel so concurrent writes are serialized.
    let (tx, mut frame_rx) = mpsc::unbounded_channel::<Frame>();
    let writer = tokio::spawn(async move {
        let mut wr = wr;
        while let Some(frame) = frame_rx.recv().await {
            if wr.write_all(&encode(&frame)).await.is_err() {
                break;
            }
        }
        let _ = wr.flush().await;
    });

    // Seccomp: if the host put `--seccomp <fd>` in the argv, pipe the shared
    // BPF to that fd in the bwrap child (mirrors the Linux host's SeccompPipe).
    if req.seccomp_fd.is_some() && bpf.is_none() {
        let _ = tx.send(Frame::Stderr(
            b"agent: seccomp requested but the guest filter failed to build\n".to_vec(),
        ));
        let _ = tx.send(Frame::Exit(ExitStatus { code: -1, timed_out: false }));
        drop(tx);
        let _ = writer.await;
        return Ok(());
    }

    let mut cmd = Command::new(&req.bwrap_path);
    cmd.args(&req.argv)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    // Install the seccomp pipe (read end dup2'd to the host-specified fd in the
    // bwrap child). Keep the read fd to close after spawn.
    let seccomp_read_fd = match (req.seccomp_fd, bpf.as_ref()) {
        (Some(fd), Some(bytes)) => match install_seccomp(&mut cmd, fd, bytes.clone()) {
            Ok(rfd) => Some(rfd),
            Err(e) => {
                let _ = tx.send(Frame::Stderr(format!("agent: seccomp setup failed: {e}\n").into_bytes()));
                let _ = tx.send(Frame::Exit(ExitStatus { code: -1, timed_out: false }));
                drop(tx);
                let _ = writer.await;
                return Ok(());
            }
        },
        _ => None,
    };

    let spawned = cmd.spawn();
    // The child holds its own dup of the read fd; close ours so the writer's
    // EOF is observed once it finishes.
    if let Some(rfd) = seccomp_read_fd {
        unsafe { libc::close(rfd) };
    }
    let mut child = match spawned {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(Frame::Stderr(format!("agent: failed to spawn bwrap: {e}\n").into_bytes()));
            let _ = tx.send(Frame::Exit(ExitStatus { code: -1, timed_out: false }));
            drop(tx);
            let _ = writer.await;
            return Ok(());
        }
    };

    // In-guest cgroup v2 (defense-in-depth; the prlimit wrapper in the argv is
    // the always-on backstop). Held until the end of the fn so Drop removes the
    // cgroup after the child exits.
    let _cgroup = match (&req.cgroup, child.id()) {
        (Some(limits), Some(pid)) => match GuestCgroup::create(req.request_id, limits) {
            Ok(cg) => {
                cg.attach(pid);
                Some(cg)
            }
            Err(e) => {
                tracing::warn!("agent: cgroup create failed: {e}; prlimit still applies");
                None
            }
        },
        _ => None,
    };

    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");
    let out_tx = tx.clone();
    let err_tx = tx.clone();
    let out_task = tokio::spawn(pump(stdout, out_tx, true));
    let err_task = tokio::spawn(pump(stderr, err_tx, false));

    // Wait for the command, racing three things: normal exit, the wall-clock
    // budget (→ SIGKILL; bwrap's --die-with-parent collapses the tree), and the
    // host disconnecting (B5 — turn aborted). On host disconnect we kill the
    // workload instead of letting it run to the full timeout.
    let budget = Duration::from_millis(req.timeout_ms.max(1));
    let (code, timed_out) = tokio::select! {
        res = child.wait() => match res {
            Ok(status) => (status.code().unwrap_or(-1), false),
            Err(_) => (-1, false),
        },
        _ = tokio::time::sleep(budget) => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            (-1, true)
        }
        _ = wait_host_eof(&mut rd) => {
            tracing::info!(request_id = req.request_id, "agent: host disconnected; killing command");
            let _ = child.start_kill();
            let _ = child.wait().await;
            (-1, false)
        }
    };

    // Make sure all output is flushed before Exit.
    let _ = out_task.await;
    let _ = err_task.await;
    let _ = tx.send(Frame::Exit(ExitStatus { code, timed_out }));
    drop(tx);
    let _ = writer.await;
    Ok(())
}

/// One-time cgroup v2 setup: ensure cgroup2 is mounted and enable the
/// controllers child cgroups will use. Best-effort — if it fails, per-exec
/// cgroup limits are simply unavailable and prlimit (in the bwrap argv) does
/// the enforcement, exactly like the Linux host's `CgroupMode::None` path.
fn cgroup_init() {
    if !std::path::Path::new("/sys/fs/cgroup/cgroup.controllers").exists() {
        let _ = std::fs::create_dir_all("/sys/fs/cgroup");
        mount_fs("cgroup2", "/sys/fs/cgroup", "cgroup2", 0, None);
    }
    // The agent (in the root cgroup) is exempt from the no-internal-processes
    // rule, so enabling subtree_control on the root is allowed.
    if let Err(e) = std::fs::write("/sys/fs/cgroup/cgroup.subtree_control", "+memory +pids +cpu") {
        tracing::warn!(
            "agent: enabling cgroup controllers failed: {e}; per-exec cgroup \
             limits unavailable (the prlimit backstop still applies)"
        );
    }
}

/// Per-exec cgroup v2 scope under the guest root, mirroring the host's
/// `CgroupScope`. Drop removes it (empty cgroups rmdir cleanly).
struct GuestCgroup {
    path: std::path::PathBuf,
}

impl GuestCgroup {
    fn create(request_id: u64, limits: &CgroupLimits) -> std::io::Result<Self> {
        let path = std::path::PathBuf::from(format!("/sys/fs/cgroup/sb-{request_id}"));
        std::fs::create_dir(&path)?;
        // Per-controller writes are best-effort: a controller the guest kernel
        // didn't build still leaves the others enforcing.
        let _ = std::fs::write(path.join("memory.max"), limits.memory_max_bytes.to_string());
        let _ = std::fs::write(path.join("memory.swap.max"), limits.memory_swap_max_bytes.to_string());
        let _ = std::fs::write(path.join("pids.max"), limits.pids_max.to_string());
        let _ = std::fs::write(path.join("cpu.max"), &limits.cpu_max);
        Ok(Self { path })
    }

    fn attach(&self, pid: u32) {
        if let Err(e) = std::fs::write(self.path.join("cgroup.procs"), pid.to_string()) {
            tracing::warn!("agent: cgroup attach pid {pid} failed: {e}");
        }
    }
}

impl Drop for GuestCgroup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir(&self.path);
    }
}

/// Pipe the seccomp BPF to a fd in the bwrap child, mirroring the Linux host's
/// `SeccompPipe`: create a pipe, write the bytes from a task, and `dup2` the
/// read end to `target_fd` (clearing `FD_CLOEXEC` so it survives execve into
/// bwrap, which reads it via `--seccomp <target_fd>`). Returns the parent's
/// read fd so the caller can close it after spawn.
fn install_seccomp(cmd: &mut Command, target_fd: i32, bpf: Arc<Vec<u8>>) -> std::io::Result<i32> {
    let mut fds: [libc::c_int; 2] = [0; 2];
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let read_fd = fds[0];
    let write_fd = fds[1];

    // Write the BPF from a task (it may exceed the pipe buffer). Must complete
    // in full or bwrap rejects a truncated filter.
    tokio::spawn(async move {
        let bytes: &[u8] = bpf.as_ref();
        let mut off = 0;
        while off < bytes.len() {
            let n = unsafe {
                libc::write(
                    write_fd,
                    bytes[off..].as_ptr() as *const libc::c_void,
                    bytes.len() - off,
                )
            };
            if n > 0 {
                off += n as usize;
                continue;
            }
            let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
            if n < 0 && (errno == libc::EINTR || errno == libc::EAGAIN) {
                continue;
            }
            break;
        }
        unsafe { libc::close(write_fd) };
        if off < bytes.len() {
            tracing::error!(written = off, expected = bytes.len(), "agent: seccomp BPF write truncated");
        }
    });

    // SAFETY: dup2/fcntl are async-signal-safe; `read_fd` is valid through spawn.
    unsafe {
        cmd.pre_exec(move || {
            if libc::dup2(read_fd, target_fd) < 0 {
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
    Ok(read_fd)
}

/// Resolve when the host closes the control connection (read half hits EOF) or
/// errors — i.e. the turn was abandoned. The host sends only the `Exec` frame,
/// so any further read is EOF on disconnect.
async fn wait_host_eof<R: AsyncReadExt + Unpin>(rd: &mut R) {
    let mut b = [0u8; 256];
    loop {
        match rd.read(&mut b).await {
            Ok(0) | Err(_) => return,
            Ok(_) => {} // unexpected extra data from the host; ignore
        }
    }
}

/// Stream a child pipe into protocol frames.
async fn pump<R: AsyncReadExt + Unpin>(mut reader: R, tx: mpsc::UnboundedSender<Frame>, is_stdout: bool) {
    let mut buf = [0u8; READ_CHUNK];
    loop {
        match reader.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                let chunk = buf[..n].to_vec();
                let frame = if is_stdout { Frame::Stdout(chunk) } else { Frame::Stderr(chunk) };
                if tx.send(frame).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}
