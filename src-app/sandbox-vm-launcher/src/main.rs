//! `ziee-sandbox-vm-launcher` — boots one libkrun microVM and *becomes* it.
//!
//! The server spawns this as a child process (one per warm flavor VM), passing
//! a JSON [`VmLaunchConfig`] file path as argv[1]. On macOS it configures a
//! libkrun context and calls `krun_start_enter`, which **never returns on
//! success** — the calling process turns into the VM and `exit()`s with the
//! guest's exit code. That's exactly why this lives in a separate process
//! rather than a `fork()` inside the server's multithreaded tokio runtime
//! (only async-signal-safe calls are legal post-fork; libkrun is far from it).
//!
//! The guest entrypoint is the `ziee-sandbox-agent` binary (see
//! `sandbox-guest-agent`), which mounts the squashfs + workspace and serves
//! exec requests over vsock. The server reaches the agent through the unix
//! socket libkrun bridges to the guest vsock port.

use serde::{Deserialize, Serialize};

/// Everything the launcher needs to stand up one VM. Written by the server as
/// a temp JSON file; path passed as argv[1].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmLaunchConfig {
    pub num_vcpus: u8,
    pub ram_mib: u32,
    /// Host directory used as the guest root (contains the agent + bwrap + the
    /// minimal userland). Shared via libkrun's root virtio-fs.
    pub root_path: String,
    /// Host path to the sandbox **squashfs** for this flavor; added as a
    /// read-only virtio-blk disk (the agent mounts it at `/sandbox-rootfs`).
    pub sandbox_disk_path: String,
    /// Host workspace root, shared into the guest via virtio-fs tag `workspace`.
    pub workspace_host_path: String,
    /// Unix socket libkrun bridges to the guest vsock port. The server connects
    /// here to talk to the agent.
    pub vsock_socket_path: String,
    /// Guest vsock port the agent listens on.
    pub vsock_port: u32,
    /// Absolute path to the agent binary *inside the guest root*.
    pub agent_exec_path: String,
}

fn main() {
    let cfg_path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: ziee-sandbox-vm-launcher <config.json>");
        std::process::exit(2);
    });
    let cfg: VmLaunchConfig = {
        let bytes = std::fs::read(&cfg_path).unwrap_or_else(|e| {
            eprintln!("launcher: cannot read config {cfg_path}: {e}");
            std::process::exit(2);
        });
        serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            eprintln!("launcher: bad config json: {e}");
            std::process::exit(2);
        })
    };

    run(cfg);
}

#[cfg(target_os = "macos")]
fn run(cfg: VmLaunchConfig) -> ! {
    use std::ffi::CString;
    use std::os::raw::c_char;

    // libkrun FFI — signatures verified against containers/libkrun
    // include/libkrun.h. `krun_start_enter` only returns on a pre-boot error;
    // on success it exec/exits with the workload's code.
    #[link(name = "krun")]
    extern "C" {
        fn krun_create_ctx() -> i32;
        fn krun_set_vm_config(ctx_id: u32, num_vcpus: u8, ram_mib: u32) -> i32;
        // virtio-fs root with a read_only flag (gap #2): the guest root (agent +
        // bwrap) is mounted read-only so an escaped guest can't tamper it or
        // persist into the shared host dir for future VMs. Uses KRUN_FS_ROOT_TAG.
        fn krun_add_virtiofs3(
            ctx_id: u32,
            c_tag: *const c_char,
            c_path: *const c_char,
            shm_size: u64,
            read_only: bool,
        ) -> i32;
        fn krun_add_disk(
            ctx_id: u32,
            block_id: *const c_char,
            disk_path: *const c_char,
            read_only: bool,
        ) -> i32;
        fn krun_add_virtiofs(ctx_id: u32, c_tag: *const c_char, c_path: *const c_char) -> i32;
        fn krun_add_vsock_port2(
            ctx_id: u32,
            port: u32,
            c_filepath: *const c_char,
            listen: bool,
        ) -> i32;
        fn krun_set_exec(
            ctx_id: u32,
            exec_path: *const c_char,
            argv: *const *const c_char,
            envp: *const *const c_char,
        ) -> i32;
        fn krun_start_enter(ctx_id: u32) -> i32;
    }

    fn cstr(s: &str) -> CString {
        CString::new(s).expect("no interior NUL")
    }
    fn check(what: &str, rc: i32) {
        if rc < 0 {
            eprintln!("launcher: {what} failed (rc={rc})");
            std::process::exit(1);
        }
    }

    // libkrun's KRUN_FS_ROOT_TAG (the magic virtio-fs tag that becomes `/`).
    const KRUN_FS_ROOT_TAG: &str = "/dev/root";

    // Gap #5: macOS has no PR_SET_PDEATHSIG. Watch the parent (the server);
    // if it dies, exit so the VM is reclaimed instead of orphaned. Started
    // BEFORE krun_start_enter (which takes over this thread + never returns).
    {
        let initial_ppid = unsafe { libc::getppid() };
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let ppid = unsafe { libc::getppid() };
            // Reparented (parent died → adopted by launchd, pid 1).
            if ppid != initial_ppid || ppid == 1 {
                eprintln!("launcher: parent (server) exited; tearing down VM");
                std::process::exit(0);
            }
        });
    }

    unsafe {
        let ctx = krun_create_ctx();
        check("krun_create_ctx", ctx);
        let ctx = ctx as u32;

        check("krun_set_vm_config", krun_set_vm_config(ctx, cfg.num_vcpus, cfg.ram_mib));

        // Read-only guest root via virtio-fs (gap #2). shm_size=0 disables the
        // DAX window (simpler; tune later if needed). The guest root image must
        // pre-create the mount points /proc, /sandbox-rootfs, /workspace since
        // the root is read-only.
        let root = cstr(&cfg.root_path);
        let root_tag = cstr(KRUN_FS_ROOT_TAG);
        check(
            "krun_add_virtiofs3(root, ro)",
            krun_add_virtiofs3(ctx, root_tag.as_ptr(), root.as_ptr(), 0, true),
        );

        // Sandbox squashfs as a read-only virtio-blk disk → /dev/vda in guest.
        let block_id = cstr("sandbox");
        let disk = cstr(&cfg.sandbox_disk_path);
        check("krun_add_disk", krun_add_disk(ctx, block_id.as_ptr(), disk.as_ptr(), true));

        // Workspace via virtio-fs (tag must match the agent's WORKSPACE_TAG).
        let tag = cstr("workspace");
        let ws = cstr(&cfg.workspace_host_path);
        check("krun_add_virtiofs", krun_add_virtiofs(ctx, tag.as_ptr(), ws.as_ptr()));

        // Bridge: libkrun listens on the host unix socket and forwards incoming
        // host connections to the guest vsock port (where the agent listens).
        // NOTE: confirm the `listen` semantics against the installed libkrun
        // version on first Mac run — flip if the server can't connect.
        let sock = cstr(&cfg.vsock_socket_path);
        check(
            "krun_add_vsock_port2",
            krun_add_vsock_port2(ctx, cfg.vsock_port, sock.as_ptr(), true),
        );

        // Run the agent as the guest entrypoint (PID 1).
        let exec = cstr(&cfg.agent_exec_path);
        let argv0 = cstr(&cfg.agent_exec_path);
        let argv: [*const c_char; 2] = [argv0.as_ptr(), std::ptr::null()];
        let envp: [*const c_char; 1] = [std::ptr::null()];
        check(
            "krun_set_exec",
            krun_set_exec(ctx, exec.as_ptr(), argv.as_ptr(), envp.as_ptr()),
        );

        // Becomes the VM. Only returns on a pre-boot error.
        let rc = krun_start_enter(ctx);
        eprintln!("launcher: krun_start_enter returned unexpectedly (rc={rc})");
        std::process::exit(if rc < 0 { 1 } else { 0 });
    }
}

#[cfg(not(target_os = "macos"))]
fn run(_cfg: VmLaunchConfig) -> ! {
    eprintln!("ziee-sandbox-vm-launcher is only supported on macOS (libkrun)");
    std::process::exit(1);
}
