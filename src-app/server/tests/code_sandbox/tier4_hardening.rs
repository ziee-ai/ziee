//! Tier 4 — hardening tests under bwrap (`#[ignore]`d).
//!
//! Each test asserts ONE invariant from the validated table in the
//! plan (Phase 3 "Empirical validation"):
//!   - Fork bomb hits RLIMIT_NPROC and doesn't take down the host.
//!   - Memory bomb hits RLIMIT_AS.
//!   - Wall-clock timeout SIGKILLs bwrap.
//!   - --die-with-parent kills child when parent dies.
//!   - Argument-terminator stops `--ro-bind / /escape` from smuggling.

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

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

fn skip_if_unavailable() -> Option<PathBuf> {
    if Command::new("bwrap").arg("--version").output().is_err() {
        eprintln!("test skipped: bwrap not installed");
        return None;
    }
    rootfs_path().or_else(|| {
        eprintln!("test skipped: no rootfs mounted (set ZIEE_SANDBOX_ROOTFS)");
        None
    })
}

fn bwrap_run(rootfs: &PathBuf, cmd: &str) -> std::process::Output {
    let usr = rootfs.join("usr");
    Command::new("bwrap")
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
        .args([
            "/usr/bin/prlimit",
            "--nproc=64",
            "--as=536870912",
            "--fsize=268435456",
            "--nofile=1024",
            "--core=0",
            "--",
            "/bin/bash",
            "-lc",
            cmd,
        ])
        .output()
        .expect("bwrap spawn")
}

#[test]
#[ignore]
fn fork_bomb_killed_by_nproc_rlimit() {
    let Some(rootfs) = skip_if_unavailable() else { return };
    let started = std::time::Instant::now();
    // Don't actually run a real fork-bomb against the host — set
    // NPROC=64 and try to spawn 200 sleeps; expect EAGAIN.
    let out = bwrap_run(
        &rootfs,
        r#"for i in $(seq 1 200); do sleep 30 & done 2>&1 | head -3; wait"#,
    );
    let elapsed = started.elapsed();
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(elapsed < Duration::from_secs(8), "elapsed: {:?}", elapsed);
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("Resource temporarily unavailable")
            || combined.contains("fork: retry"),
        "expected EAGAIN evidence; got: {combined}"
    );
}

#[test]
#[ignore]
fn memory_bomb_killed_by_as_rlimit() {
    let Some(rootfs) = skip_if_unavailable() else { return };
    // Allocate 4 GiB in Python with RLIMIT_AS=512MiB → MemoryError.
    let out = bwrap_run(
        &rootfs,
        r#"python3 -c 'try:
    x = bytearray(4 * 1024 * 1024 * 1024)
    print("FAIL: alloc succeeded")
except MemoryError:
    print("MemoryError raised")'"#,
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("MemoryError raised"),
        "expected MemoryError; got: {stdout}"
    );
}

#[test]
#[ignore]
fn argument_injection_after_dashdash_is_inert() {
    let Some(rootfs) = skip_if_unavailable() else { return };
    let usr = rootfs.join("usr");
    // Pass a "filename" containing flag-like text AFTER the --. bwrap
    // must treat it as data, not as a `--ro-bind` flag.
    let evil = "--ro-bind / /escape-attempt";
    let out = Command::new("bwrap")
        .args(["--unshare-user", "--uid", "1001", "--gid", "1001", "--share-net", "--new-session", "--die-with-parent"])
        .args(["--ro-bind", usr.to_str().unwrap(), "/usr"])
        .args(["--symlink", "usr/bin", "/bin"])
        .args(["--symlink", "usr/lib", "/lib"])
        .args(["--symlink", "usr/lib64", "/lib64"])
        .args(["--dev-bind", "/proc", "/proc"])
        .args(["--dev", "/dev"])
        .args(["--tmpfs", "/tmp"])
        .arg("--")
        .args(["/bin/sh", "-c", &format!("echo received: {evil}; ls / | tr '\\n' ' '")])
        .output()
        .expect("bwrap spawn");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "exit: {}, stderr: {}", out.status, String::from_utf8_lossy(&out.stderr));
    assert!(stdout.contains(&format!("received: {evil}")), "stdout: {stdout}");
    // The crucial assertion: bwrap did NOT bind / → /escape-attempt.
    assert!(
        !stdout.contains("escape-attempt"),
        "escape-attempt directory leaked into sandbox /: {stdout}"
    );
}

#[test]
#[ignore]
fn wall_clock_timeout_kills_bwrap() {
    let Some(rootfs) = skip_if_unavailable() else { return };
    let started = std::time::Instant::now();
    let usr = rootfs.join("usr");
    let out = Command::new("timeout")
        .args(["--kill-after=1", "2"])
        .arg("bwrap")
        .args(["--unshare-user", "--uid", "1001", "--gid", "1001", "--share-net", "--new-session", "--die-with-parent"])
        .args(["--ro-bind", usr.to_str().unwrap(), "/usr"])
        .args(["--symlink", "usr/bin", "/bin"])
        .args(["--symlink", "usr/lib", "/lib"])
        .args(["--symlink", "usr/lib64", "/lib64"])
        .args(["--dev-bind", "/proc", "/proc"])
        .args(["--dev", "/dev"])
        .args(["--tmpfs", "/tmp"])
        .arg("--")
        .args(["/bin/sleep", "30"])
        .output()
        .expect("timeout/bwrap spawn");
    let elapsed = started.elapsed();
    // timeout returns 124 when it kills.
    assert!(elapsed < Duration::from_secs(5), "elapsed too long: {:?}", elapsed);
    assert!(
        out.status.code() == Some(124) || out.status.code() == Some(137),
        "expected timeout exit, got {:?}",
        out.status
    );
}
