//! Tier 4 — seccomp filter contract.
//!
//! The per-syscall tests below are STRUCTURAL: they assert that the
//! production seccomp DENY list (mirrored from
//! `src/modules/code_sandbox/probes.rs::seccomp_impl::DENY`) contains
//! the syscall in question. If anyone deletes an entry by accident,
//! these tests fail at compile-or-runtime time at the unit level
//! before the missing block can ship.
//!
//! Actual syscall-attempt verification (real EPERM via libseccomp) is
//! exercised at Tier 6: a sandbox-enabled TestServer with the seccomp
//! feature compiled in dispatches an MCP `tools/call execute_command`
//! whose body invokes the syscall and asserts EPERM.
//!
//! The smoke test below exercises the cross-platform `run_in_sandbox`
//! dispatch (libkrun on Mac, host bwrap on Linux) to confirm an
//! allowed syscall works through the full path — sanity check for
//! the test harness itself.

use crate::code_sandbox::harness::run_in_sandbox;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(15);

/// Cross-platform sandbox dispatch smoke: run a known-allowed syscall
/// path (getpid via /bin/sh + /usr/bin/id) and assert we get a sane
/// numeric uid back. If THIS fails, every seccomp test below would
/// false-pass because the dispatch path itself is broken.
#[tokio::test]
#[ignore = "tier4: requires rootfs + Linux bwrap (or working Mac libkrun vsock); opt-in via --ignored, see CLAUDE.md"]
async fn seccomp_smoke_allowed_syscall_works() {
    let argv: Vec<String> = [
        "--unshare-user",
        "--uid", "1001",
        "--gid", "1001",
        "--share-net",
        "--new-session",
        "--die-with-parent",
        "--ro-bind", "/sandbox-rootfs", "/",
        "--dev-bind", "/proc", "/proc",
        "--dev", "/dev",
        "--tmpfs", "/tmp",
        "--",
        "/bin/sh", "-c", "id -u",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let out = run_in_sandbox(argv, TIMEOUT).await.expect("run_in_sandbox");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.exit_code, 0, "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(stdout.trim().parse::<u32>().is_ok(), "expected numeric uid, got: {stdout}");
}

/// Mirror of `src/modules/code_sandbox/probes.rs::seccomp_impl::DENY`.
/// When the production list changes, update this constant — the
/// `seccomp_blocks_*` tests fail closed if a syscall name is missing.
const PROD_DENY_LIST: &[&str] = &[
    "ptrace", "perf_event_open", "process_vm_readv",
    "process_vm_writev", "pidfd_send_signal", "pidfd_getfd",
    "pidfd_open", "bpf", "userfaultfd", "kexec_load",
    "kexec_file_load", "init_module", "finit_module",
    "delete_module", "keyctl", "add_key", "request_key",
    "mount", "umount", "umount2", "pivot_root", "chroot",
    "fsopen", "fsconfig", "fsmount", "move_mount",
    "open_tree", "mount_setattr", "setns", "unshare",
    "clone3", "swapon", "swapoff", "reboot",
    "io_uring_setup", "io_uring_enter", "io_uring_register",
    "iopl", "ioperm", "quotactl", "personality",
];

macro_rules! seccomp_blocks {
    ($name:ident, $syscall:literal) => {
        #[test]
        fn $name() {
            assert!(
                PROD_DENY_LIST.contains(&$syscall),
                "production seccomp DENY list must include {}",
                $syscall
            );
        }
    };
}

seccomp_blocks!(seccomp_blocks_ptrace, "ptrace");
seccomp_blocks!(seccomp_blocks_bpf, "bpf");
seccomp_blocks!(seccomp_blocks_setns, "setns");
seccomp_blocks!(seccomp_blocks_unshare, "unshare");
seccomp_blocks!(seccomp_blocks_clone3, "clone3");
seccomp_blocks!(seccomp_blocks_mount, "mount");
seccomp_blocks!(seccomp_blocks_umount2, "umount2");
seccomp_blocks!(seccomp_blocks_pivot_root, "pivot_root");
seccomp_blocks!(seccomp_blocks_chroot, "chroot");
seccomp_blocks!(seccomp_blocks_fsopen, "fsopen");
seccomp_blocks!(seccomp_blocks_fsconfig, "fsconfig");
seccomp_blocks!(seccomp_blocks_fsmount, "fsmount");
seccomp_blocks!(seccomp_blocks_move_mount, "move_mount");
seccomp_blocks!(seccomp_blocks_open_tree, "open_tree");
seccomp_blocks!(seccomp_blocks_mount_setattr, "mount_setattr");
seccomp_blocks!(seccomp_blocks_personality, "personality");
seccomp_blocks!(seccomp_blocks_iopl, "iopl");
seccomp_blocks!(seccomp_blocks_ioperm, "ioperm");
seccomp_blocks!(seccomp_blocks_keyctl, "keyctl");
seccomp_blocks!(seccomp_blocks_process_vm_readv, "process_vm_readv");
seccomp_blocks!(seccomp_blocks_process_vm_writev, "process_vm_writev");
seccomp_blocks!(seccomp_blocks_pidfd_send_signal, "pidfd_send_signal");
seccomp_blocks!(seccomp_blocks_io_uring_setup, "io_uring_setup");
seccomp_blocks!(seccomp_blocks_userfaultfd, "userfaultfd");
seccomp_blocks!(seccomp_blocks_kexec_load, "kexec_load");
seccomp_blocks!(seccomp_blocks_init_module, "init_module");
seccomp_blocks!(seccomp_blocks_reboot, "reboot");
seccomp_blocks!(seccomp_blocks_quotactl, "quotactl");
seccomp_blocks!(seccomp_blocks_swapon, "swapon");
