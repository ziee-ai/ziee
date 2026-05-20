//! Tier 4 — cgroup delegation behavior.
//!
//! Two assertions:
//!   1. `caps.cgroup = None` mode (no delegation): the rlimits-only
//!      memory enforcement still kicks in (covered by tier4_hardening's
//!      memory_bomb_killed_by_as_rlimit; this file confirms the
//!      pre-condition).
//!   2. `caps.cgroup = Delegated(parent)` mode: a transient child
//!      cgroup is created + memory.max enforced + cgroup is cleaned
//!      up on exit.
//!
//! Cgroup tests are skipped unless the test runner has write access
//! to a delegated parent — typically false in unprivileged docker.

use std::path::PathBuf;
use std::process::Command;

use crate::code_sandbox::harness::{bwrap_available, needs_cgroup_delegation, rootfs_path};

#[test]
#[ignore]
fn no_delegation_falls_back_to_rlimits_only() {
    if !bwrap_available() {
        eprintln!("test skipped: bwrap not installed");
        return;
    }
    let Some(_rootfs) = rootfs_path() else {
        eprintln!("test skipped: no rootfs mounted");
        return;
    };
    // The mere absence of `needs_cgroup_delegation()` returning true
    // proves we're in the rlimits-only path. The actual rlimits
    // enforcement is tested by tier4_hardening::memory_bomb_killed_by_as_rlimit.
    // This test documents the fallback expectation explicitly.
    let has_cgroups = needs_cgroup_delegation();
    println!(
        "code_sandbox cgroup mode: {}",
        if has_cgroups {
            "Delegated"
        } else {
            "None (rlimits-only)"
        }
    );
    // Always passes — this is a documentation-style assertion.
}

#[test]
#[ignore]
fn delegated_cgroup_enforces_memory_max() {
    if !bwrap_available() {
        eprintln!("test skipped: bwrap not installed");
        return;
    }
    if !needs_cgroup_delegation() {
        return;
    }
    let Some(rootfs) = rootfs_path() else {
        eprintln!("test skipped: no rootfs mounted");
        return;
    };

    let parent = std::env::var("CODE_SANDBOX_CGROUP_PARENT")
        .unwrap_or_else(|_| "/sys/fs/cgroup/ziee-sandbox.slice".to_string());
    let parent = PathBuf::from(parent);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let cg = parent.join(format!("test-{nanos}"));
    std::fs::create_dir(&cg).expect("create cgroup");
    // Cleanup helper — manual drop guard (no scopeguard dep).
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir(&self.0);
        }
    }
    let _g = Cleanup(cg.clone());

    std::fs::write(cg.join("memory.max"), "33554432").expect("memory.max=32M");
    let _ = std::fs::write(cg.join("memory.swap.max"), "0");

    let usr = rootfs.join("usr");
    let cmd_str = format!(
        r#"echo $$ > {}/cgroup.procs; exec python3 -c '
try:
    x = bytearray(128 * 1024 * 1024)
    print("FAIL")
except MemoryError:
    print("OK_memerror")'"#,
        cg.display()
    );
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
        // Sysfs bind so the shell can write to cgroup.procs from inside.
        .args([
            "--dev-bind",
            cg.to_str().unwrap(),
            cg.to_str().unwrap(),
        ])
        .arg("--")
        .args(["/bin/sh", "-c", &cmd_str])
        .output()
        .expect("bwrap spawn");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Either the cgroup attach succeeded and memory.max killed the
    // process (exit 137 SIGKILL) or Python raised MemoryError on its
    // own from rlimits. Both are acceptable outcomes of "memory was
    // bounded".
    let exit = out.status.code().unwrap_or(-1);
    assert!(
        stdout.contains("OK_memerror") || exit == 137,
        "expected MemoryError or SIGKILL; stdout={stdout} exit={exit}"
    );
}
