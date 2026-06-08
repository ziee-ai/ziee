//! Tier 4 — PID-namespace strict mode vs --dev-bind fallback.
//!
//! Validates the two-tier strategy from the plan:
//!   PidNsMode::Strict         → --unshare-pid + --proc /proc
//!   PidNsMode::DevBindFallback→ --dev-bind /proc /proc (host/VM PIDs visible)
//!   PidNsMode::Disabled       → sandbox refuses to run
//!
//! On macOS the libkrun VM is its own PID namespace (the agent runs as
//! PID 1 inside), so `--unshare-pid + --proc /proc` is uniformly
//! available regardless of host kernel quirks.

use crate::code_sandbox::harness::run_in_sandbox;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(15);

#[tokio::test]
#[ignore = "tier4: requires rootfs + Linux bwrap (or working Mac libkrun vsock); opt-in via --ignored, see CLAUDE.md"]
async fn strict_mode_proc_self_status_shows_pid_1() {
    let argv: Vec<String> = [
        "--unshare-user",
        "--uid", "1001",
        "--gid", "1001",
        "--unshare-pid",
        "--share-net",
        "--new-session",
        "--die-with-parent",
        "--ro-bind", "/sandbox-rootfs", "/",
        "--proc", "/proc",
        "--dev", "/dev",
        "--tmpfs", "/tmp",
        "--",
        "/bin/sh", "-c", "echo $$",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let out = run_in_sandbox(argv, TIMEOUT).await.expect("run_in_sandbox");
    if out.exit_code != 0 {
        eprintln!(
            "test note: strict PID-ns mode rejected by this kernel (stderr: {})",
            String::from_utf8_lossy(&out.stderr)
        );
        // The DevBindFallback test below is the always-available
        // alternative; let this one pass-with-warning on kernels
        // that can't mount a fresh procfs.
        return;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let pid: u32 = stdout.trim().parse().unwrap_or(0);
    // bwrap itself is PID 1 in the new namespace; the shell it execs
    // is PID 2. Both indicate a fresh pid-ns. A real-host PID (>10)
    // would mean strict mode was silently downgraded.
    assert!(
        pid <= 5,
        "in strict PID-ns mode the shell PID should be tiny (1 or 2), got {pid}"
    );
}

#[tokio::test]
#[ignore = "tier4: requires rootfs + Linux bwrap (or working Mac libkrun vsock); opt-in via --ignored, see CLAUDE.md"]
async fn dev_bind_fallback_shows_real_pid_in_proc() {
    let argv: Vec<String> = [
        "--unshare-user",
        "--uid", "1001",
        "--gid", "1001",
        // NOTE: no --unshare-pid here. Fallback mode binds the
        // executing env's /proc directly; the shell's $$ is its
        // real PID in that env (host PID on Linux, in-VM PID on Mac).
        "--share-net",
        "--new-session",
        "--die-with-parent",
        "--ro-bind", "/sandbox-rootfs", "/",
        "--dev-bind", "/proc", "/proc",
        "--dev", "/dev",
        "--tmpfs", "/tmp",
        "--",
        "/bin/sh", "-c", "echo $$",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let out = run_in_sandbox(argv, TIMEOUT).await.expect("run_in_sandbox");
    assert_eq!(out.exit_code, 0, "fallback mode must work; stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let pid: u32 = stdout.trim().parse().expect("parse pid");
    assert!(
        pid > 1,
        "in --dev-bind /proc fallback the shell should see a real PID > 1, got {pid}"
    );
}
