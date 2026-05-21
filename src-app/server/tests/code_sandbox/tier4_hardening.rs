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
        // Echo on one line; `ls /` on a separate line so the test
        // can isolate the LISTING and look for the escape dir there
        // (the echo itself contains "escape-attempt" as data — we
        // don't want to false-positive on that).
        .args(["/bin/sh", "-c", &format!("echo received: {evil}; echo SEP; ls / | tr '\\n' ' '")])
        .output()
        .expect("bwrap spawn");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "exit: {}, stderr: {}", out.status, String::from_utf8_lossy(&out.stderr));
    assert!(stdout.contains(&format!("received: {evil}")), "stdout: {stdout}");
    // Split on the SEP marker — only the second half is the `ls /` output.
    let listing = stdout.splitn(2, "SEP").nth(1).unwrap_or("");
    assert!(
        !listing.contains("escape-attempt"),
        "escape-attempt directory leaked into sandbox /: listing={listing}"
    );
}

#[test]
#[ignore]
fn wall_clock_timeout_kills_bwrap() {
    let Some(rootfs) = skip_if_unavailable() else { return };
    let started = std::time::Instant::now();
    let usr = rootfs.join("usr");
    // Use Stdio::null() for stdout/stderr: when timeout SIGKILLs
    // bwrap, the orphan sleep child can inherit the parent's pipe
    // fds and hold them open for the full 30s — `Command::output()`
    // would then wait for fd EOF and the test would falsely report
    // a 30-second runtime. With null, the parent doesn't wait for
    // anyone to read pipe data and `.wait()` returns as soon as the
    // signal lands.
    use std::process::Stdio;
    let mut child = Command::new("timeout")
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
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("timeout/bwrap spawn");
    let exit = child.wait().expect("wait");
    let elapsed = started.elapsed();
    // Mirror the .output() shape so the assertion below keeps working.
    struct Out { status: std::process::ExitStatus }
    let out = Out { status: exit };
    // timeout(1) sends SIGTERM at the budget; with --kill-after it
    // follows with SIGKILL. The signal lands on bwrap (our direct
    // child). Possible outcomes:
    //   - Normal exit 124 (timeout's documented signal-handler exit)
    //   - Normal exit 137 (shell-shape SIGKILL)
    //   - Signal-death status (Rust's ExitStatus carries .signal()
    //     directly when the process died by signal — code() is None)
    use std::os::unix::process::ExitStatusExt;
    assert!(elapsed < Duration::from_secs(5), "elapsed too long: {:?}", elapsed);
    let died_via_signal = out.status.signal().is_some();
    let exited_with_timeout_code = matches!(out.status.code(), Some(124) | Some(137));
    assert!(
        exited_with_timeout_code || died_via_signal,
        "expected timeout exit or signal-death, got {:?}",
        out.status
    );
}
