//! Tier 4 — cgroup delegation behavior.
//!
//! Memory enforcement has two layers:
//!   - rlimits (always-on, via prlimit in the bwrap argv) — covered by
//!     `tier4_hardening::memory_bomb_killed_by_as_rlimit`.
//!   - cgroup v2 memory.max (defense-in-depth; Linux backend delegates
//!     a parent cgroup, Mac backend's libkrun agent sets up cgroup
//!     inside the guest VM, Windows backend's WSL2 agent does the same).
//!
//! The cgroup setup is a per-backend implementation detail; the safety
//! property it provides — "memory hog is bounded" — is exercised by
//! tier4_hardening + tier6_hardening. This file documents the
//! delegation mode the active backend is using (helpful for debugging
//! a regression on a specific platform).

use crate::code_sandbox::harness::run_in_sandbox;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(15);

#[tokio::test]
async fn cgroup_v2_is_mounted_inside_sandbox() {
    // Inside the active sandbox dispatch, /sys/fs/cgroup should be a
    // cgroup2 filesystem (either delegated from the host on Linux, or
    // set up by the agent inside the libkrun/WSL2 VM on Mac/Windows).
    // If it isn't present, the in-VM cgroup defense-in-depth isn't
    // active and we're relying purely on rlimits.
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
        "/bin/sh", "-c",
        "if [ -f /sys/fs/cgroup/cgroup.controllers ]; then \
           echo CGROUP2_MOUNTED; \
           cat /sys/fs/cgroup/cgroup.controllers 2>/dev/null; \
         else \
           echo NO_CGROUP2 \
             '(rlimits-only path — defense-in-depth via prlimit still applies)'; \
         fi",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let out = run_in_sandbox(argv, TIMEOUT).await.expect("run_in_sandbox");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.exit_code, 0, "stderr: {}", String::from_utf8_lossy(&out.stderr));
    // Both modes are accepted — the assertion is that we got a clean
    // answer (the probe ran), not that one specific mode is active.
    assert!(
        stdout.contains("CGROUP2_MOUNTED") || stdout.contains("NO_CGROUP2"),
        "expected probe output, got: {stdout}"
    );
    eprintln!("cgroup state inside sandbox:\n{stdout}");
}

#[tokio::test]
async fn rlimits_apply_when_cgroup_unavailable() {
    // The fallback contract: even when cgroup delegation is off, the
    // prlimit wrapper in the bwrap argv MUST clamp memory. This is the
    // same property tier4_hardening::memory_bomb_killed_by_as_rlimit
    // tests; this entry exists so the cgroup-fallback story has its
    // own discoverable test.
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
        "/usr/bin/prlimit",
        "--as=16777216", // 16 MiB virtual memory
        "--",
        "/bin/sh", "-c",
        // Allocate 64 MiB in shell (string repeat). With AS=16MiB,
        // bash's malloc should fail and we should get a non-zero exit
        // OR an explicit error message. We check via the dd path:
        // dd's bs allocation hits the limit.
        "dd if=/dev/zero of=/dev/null bs=64M count=1 2>&1 | head -3; echo RC=$?",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let out = run_in_sandbox(argv, TIMEOUT).await.expect("run_in_sandbox");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let combined = format!("{stdout}{stderr}");
    // dd reports either "Cannot allocate memory" or a clamped byte count.
    assert!(
        combined.contains("Cannot allocate memory")
            || combined.contains("memory exhausted")
            || combined.contains("RC=1"),
        "expected memory limit hit. stdout={stdout} stderr={stderr}"
    );
}
