//! Tier 4 — PID-namespace strict mode vs --dev-bind fallback.
//!
//! Validates the two-tier strategy from the plan:
//!   PidNsMode::Strict         → --unshare-pid + --proc /proc
//!   PidNsMode::DevBindFallback→ --dev-bind /proc /proc (host PIDs visible)
//!   PidNsMode::Disabled       → sandbox refuses to run
//!
//! Skipped if bwrap or rootfs unavailable.

use std::process::Command;

use crate::code_sandbox::harness::{bwrap_available, rootfs_path};

#[test]
#[ignore]
fn strict_mode_proc_self_status_shows_pid_1() {
    if !bwrap_available() {
        eprintln!("test skipped: bwrap not installed");
        return;
    }
    let Some(rootfs) = rootfs_path() else {
        eprintln!("test skipped: no rootfs mounted");
        return;
    };
    let usr = rootfs.join("usr");

    // Try strict mode; some environments (eg. docker without
    // CAP_SYS_ADMIN) reject the new procfs mount. In that case the
    // boot probe falls back to DevBindFallback, so this test maps to
    // "Strict works → assert PID 1" / "Strict fails → skip".
    let out = Command::new("bwrap")
        .args([
            "--unshare-user",
            "--uid",
            "1001",
            "--gid",
            "1001",
            "--unshare-pid",
            "--share-net",
            "--new-session",
            "--die-with-parent",
        ])
        .args(["--ro-bind", usr.to_str().unwrap(), "/usr"])
        .args(["--symlink", "usr/bin", "/bin"])
        .args(["--symlink", "usr/lib", "/lib"])
        .args(["--symlink", "usr/lib64", "/lib64"])
        .args(["--proc", "/proc"])
        .args(["--dev", "/dev"])
        .args(["--tmpfs", "/tmp"])
        .arg("--")
        .args(["/bin/sh", "-c", "echo $$"])
        .output()
        .expect("bwrap spawn");

    if !out.status.success() {
        eprintln!(
            "test skipped: strict PID-ns mode not supported in this env (stderr: {})",
            String::from_utf8_lossy(&out.stderr)
        );
        return;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let pid: u32 = stdout.trim().parse().unwrap_or(0);
    assert_eq!(pid, 1, "in strict PID-ns mode the shell should be PID 1");
}

#[test]
#[ignore]
fn dev_bind_fallback_shows_host_pids_in_proc() {
    if !bwrap_available() {
        eprintln!("test skipped: bwrap not installed");
        return;
    }
    let Some(rootfs) = rootfs_path() else {
        eprintln!("test skipped: no rootfs mounted");
        return;
    };
    let usr = rootfs.join("usr");

    // Fallback mode: do NOT --unshare-pid; use --dev-bind /proc /proc.
    // The shell's $$ is its real host PID, demonstrably > 1.
    let out = Command::new("bwrap")
        .args([
            "--unshare-user",
            "--uid",
            "1001",
            "--gid",
            "1001",
            "--share-net",
            "--new-session",
            "--die-with-parent",
        ])
        .args(["--ro-bind", usr.to_str().unwrap(), "/usr"])
        .args(["--symlink", "usr/bin", "/bin"])
        .args(["--symlink", "usr/lib", "/lib"])
        .args(["--symlink", "usr/lib64", "/lib64"])
        .args(["--dev-bind", "/proc", "/proc"])
        .args(["--dev", "/dev"])
        .args(["--tmpfs", "/tmp"])
        .arg("--")
        .args(["/bin/sh", "-c", "echo $$"])
        .output()
        .expect("bwrap spawn");
    assert!(out.status.success(), "fallback mode must work everywhere");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let pid: u32 = stdout.trim().parse().expect("parse pid");
    assert!(
        pid > 1,
        "in --dev-bind /proc fallback the shell should see its real host PID > 1, got {pid}"
    );
}
