//! Tier 4 — bwrap-required smoke tests.
//!
//! All tests `#[ignore]` so server-only CI skips them; locally and in
//! the rootfs-PR / nightly workflows they run with:
//!   cargo test --test integration_tests -- --ignored code_sandbox::tier4_
//!
//! Requires:
//!   - bwrap installed (`apt install bubblewrap`)
//!   - Rootfs mounted at ZIEE_SANDBOX_ROOTFS env var (default
//!     /tmp/ziee-sandbox-rootfs or .ziee-cache/sandbox-rootfs/current)
//!
//! The test driver is intentionally thin — it shells out to bwrap
//! directly with the SAME flag set the production sandbox.rs uses.
//! When sandbox.rs gains a flag, mirror the change here.

use std::path::PathBuf;
use std::process::Command;

fn rootfs_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("ZIEE_SANDBOX_ROOTFS") {
        let pb = PathBuf::from(p);
        if pb.join("usr").exists() {
            return Some(pb);
        }
    }
    for candidate in [
        ".ziee-cache/sandbox-rootfs/current",
        "/var/lib/ziee/sandbox-rootfs/current",
        "/opt/ziee-sandbox-rootfs/current",
    ] {
        let pb = PathBuf::from(candidate);
        if pb.join("usr").exists() {
            return Some(pb);
        }
    }
    None
}

fn skip_if_no_rootfs() -> Option<PathBuf> {
    match rootfs_path() {
        Some(p) => Some(p),
        None => {
            eprintln!(
                "test skipped: no rootfs found. Set ZIEE_SANDBOX_ROOTFS or run \
                 `just sandbox-build && just sandbox-mount`."
            );
            None
        }
    }
}

fn bwrap_available() -> bool {
    Command::new("bwrap").arg("--version").output().is_ok()
}

#[test]
#[ignore]
fn smoke_echo_hello() {
    if !bwrap_available() {
        eprintln!("test skipped: bwrap not installed");
        return;
    }
    let Some(rootfs) = skip_if_no_rootfs() else { return };
    let usr = rootfs.join("usr");

    let out = Command::new("bwrap")
        .args([
            "--unshare-user",
            "--uid",
            "1001",
            "--gid",
            "1001",
            "--unshare-uts",
            "--unshare-ipc",
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
        .args(["/bin/echo", "hello-from-sandbox"])
        .output()
        .expect("bwrap spawn");
    assert!(out.status.success(), "exit: {}", out.status);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "hello-from-sandbox"
    );
}

#[test]
#[ignore]
fn smoke_whoami_is_sandboxuser() {
    if !bwrap_available() {
        eprintln!("test skipped: bwrap not installed");
        return;
    }
    let Some(rootfs) = skip_if_no_rootfs() else { return };
    let usr = rootfs.join("usr");

    // Synthetic passwd file.
    let ws = tempfile::tempdir().unwrap();
    let passwd = ws.path().join("passwd");
    std::fs::write(
        &passwd,
        "sandboxuser:x:1001:1001::/home/sandboxuser:/bin/bash\n",
    )
    .unwrap();
    let group = ws.path().join("group");
    std::fs::write(&group, "sandboxuser:x:1001:\n").unwrap();

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
        .args(["--ro-bind", passwd.to_str().unwrap(), "/etc/passwd"])
        .args(["--ro-bind", group.to_str().unwrap(), "/etc/group"])
        .args(["--symlink", "usr/bin", "/bin"])
        .args(["--symlink", "usr/lib", "/lib"])
        .args(["--symlink", "usr/lib64", "/lib64"])
        .args(["--dev-bind", "/proc", "/proc"])
        .args(["--dev", "/dev"])
        .args(["--tmpfs", "/tmp"])
        .arg("--")
        .args(["/bin/sh", "-c", "whoami && id -u"])
        .output()
        .expect("bwrap spawn");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "exit: {}, stderr: {}", out.status, String::from_utf8_lossy(&out.stderr));
    assert!(stdout.contains("sandboxuser"), "got: {stdout}");
    assert!(stdout.contains("1001"), "got: {stdout}");
}
