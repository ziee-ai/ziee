//! Tier 4 — sandbox smoke tests.
//!
//! Dispatched through `harness::run_in_sandbox()` so the same test code
//! runs on Linux (host bwrap), macOS (libkrun VM + bwrap-in-VM), and
//! Windows (WSL2 distro + bwrap-in-distro). No platform-specific paths
//! appear in the argv — `/sandbox-rootfs`, `/proc`, `/dev`, `/tmp`,
//! `/workspace` are the canonical in-sandbox paths.
//!
//! The rootfs squashfs is fetched from the `ziee-ai/sandbox-rootfs`
//! GitHub release (cached) by `harness::rootfs_squashfs_path()` and
//! squashfuse-mounted by `harness::ensure_test_rootfs_mounted()`.

use crate::code_sandbox::harness::run_in_sandbox;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::test]
#[ignore = "tier4: requires rootfs + Linux bwrap (or working Mac libkrun vsock); opt-in via --ignored, see CLAUDE.md"]
async fn smoke_echo_hello() {
    // Bind the entire rootfs read-only at /, then add the sandbox
    // primitives (/proc, /dev, /tmp). Binding the whole rootfs
    // (rather than carving /usr + symlinking /bin) sidesteps the
    // layout difference between Debian (real /bin/echo) and Alpine
    // (`/bin/echo → ../usr/bin/coreutils`) — both work because their
    // internal symlinks resolve within the bound tree.
    let argv: Vec<String> = [
        "--unshare-user",
        "--uid", "1001",
        "--gid", "1001",
        "--unshare-uts",
        "--unshare-ipc",
        "--share-net",
        "--new-session",
        "--die-with-parent",
        "--ro-bind", "/sandbox-rootfs", "/",
        "--dev-bind", "/proc", "/proc",
        "--dev", "/dev",
        "--tmpfs", "/tmp",
        "--",
        "/bin/echo", "hello-from-sandbox",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let result = run_in_sandbox(argv, TIMEOUT).await.expect("run_in_sandbox");
    assert_eq!(result.exit_code, 0, "stderr: {}", String::from_utf8_lossy(&result.stderr));
    assert_eq!(
        String::from_utf8_lossy(&result.stdout).trim(),
        "hello-from-sandbox"
    );
}

#[tokio::test]
#[ignore = "tier4: requires rootfs + Linux bwrap (or working Mac libkrun vsock); opt-in via --ignored, see CLAUDE.md"]
async fn smoke_whoami_is_sandboxuser() {
    // Verifies bwrap's `--uid 1001` actually applies — the in-sandbox
    // process should see uid=1001 regardless of the launching uid.
    // We assert on `id -u` (always available) rather than `whoami`
    // (which requires /etc/passwd resolution).
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
        "/bin/sh", "-c", "id -u && id -g",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let result = run_in_sandbox(argv, TIMEOUT).await.expect("run_in_sandbox");
    assert_eq!(result.exit_code, 0, "stderr: {}", String::from_utf8_lossy(&result.stderr));
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        stdout.contains("1001"),
        "expected uid/gid 1001 in stdout, got: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}
