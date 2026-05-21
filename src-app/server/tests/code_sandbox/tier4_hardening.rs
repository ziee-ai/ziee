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

// ─── Additional Tier-4 coverage (Phase 4) ───────────────────────────
//
// These tests exercise bwrap DIRECTLY (no HTTP layer) so a regression
// in the argv shape or the rootfs surface shows up at this layer even
// if a Tier-6 happy-path is masking it.

/// --clearenv MUST wipe inherited env. Set a sentinel env on the
/// test process, invoke bwrap, assert `env | grep SENTINEL` shows
/// nothing. Without --clearenv (the production bug fixed in
/// commit d28cc88), DATABASE_URL + JWT secrets + every *_API_KEY
/// would leak into the sandboxed shell.
#[test]
#[ignore]
fn clearenv_wipes_inherited_env_via_direct_bwrap() {
    let Some(rootfs) = skip_if_unavailable() else { return };
    let usr = rootfs.join("usr");
    unsafe { std::env::set_var("ZIEE_TIER4_SENTINEL", "must-not-leak") };
    let out = Command::new("bwrap")
        .arg("--clearenv")
        .args(["--unshare-user", "--uid", "1001", "--gid", "1001", "--share-net", "--new-session", "--die-with-parent"])
        .args(["--ro-bind", usr.to_str().unwrap(), "/usr"])
        .args(["--symlink", "usr/bin", "/bin"])
        .args(["--symlink", "usr/lib", "/lib"])
        .args(["--symlink", "usr/lib64", "/lib64"])
        .args(["--dev-bind", "/proc", "/proc"])
        .args(["--dev", "/dev"])
        .args(["--tmpfs", "/tmp"])
        .args(["--setenv", "HOME", "/home/sandboxuser"])
        .args(["--setenv", "USER", "sandboxuser"])
        .args(["--setenv", "PATH", "/usr/bin:/bin"])
        .arg("--")
        .args(["/bin/sh", "-c", "env"])
        .output()
        .expect("bwrap spawn");
    let stdout = String::from_utf8_lossy(&out.stdout);
    unsafe { std::env::remove_var("ZIEE_TIER4_SENTINEL") };
    assert!(
        !stdout.contains("ZIEE_TIER4_SENTINEL") && !stdout.contains("must-not-leak"),
        "SECURITY: env leaked through --clearenv. stdout:\n{stdout}"
    );
    // Sanity: the explicit --setenv values DID make it through.
    assert!(stdout.contains("USER=sandboxuser"), "USER not set: {stdout}");
}

/// /etc/ssl bind: HTTPS-using tools need CA certs.
#[test]
#[ignore]
fn etc_ssl_is_accessible_via_direct_bwrap() {
    let Some(rootfs) = skip_if_unavailable() else { return };
    let usr = rootfs.join("usr");
    let out = Command::new("bwrap")
        .args(["--unshare-user", "--uid", "1001", "--gid", "1001"])
        .args(["--ro-bind", usr.to_str().unwrap(), "/usr"])
        .args(["--ro-bind", "/etc/ssl", "/etc/ssl"])
        .args(["--symlink", "usr/bin", "/bin"])
        .args(["--symlink", "usr/lib", "/lib"])
        .args(["--symlink", "usr/lib64", "/lib64"])
        .args(["--dev-bind", "/proc", "/proc"])
        .args(["--dev", "/dev"])
        .args(["--tmpfs", "/tmp"])
        .arg("--")
        .args(["/bin/sh", "-c", "test -d /etc/ssl && ls /etc/ssl/certs/ 2>/dev/null | head -1 && echo HAVE_SSL_DIR"])
        .output()
        .expect("bwrap");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("HAVE_SSL_DIR"), "/etc/ssl not visible inside sandbox: {stdout}");
}

/// /usr is a read-only bind — sandbox user MUST NOT be able to write.
#[test]
#[ignore]
fn usr_bind_is_readonly_via_direct_bwrap() {
    let Some(rootfs) = skip_if_unavailable() else { return };
    let usr = rootfs.join("usr");
    let out = Command::new("bwrap")
        .args(["--unshare-user", "--uid", "1001", "--gid", "1001"])
        .args(["--ro-bind", usr.to_str().unwrap(), "/usr"])
        .args(["--symlink", "usr/bin", "/bin"])
        .args(["--symlink", "usr/lib", "/lib"])
        .args(["--symlink", "usr/lib64", "/lib64"])
        .args(["--dev-bind", "/proc", "/proc"])
        .args(["--dev", "/dev"])
        .args(["--tmpfs", "/tmp"])
        .arg("--")
        .args(["/bin/sh", "-c", "touch /usr/foo 2>&1 || echo 'WRITE_REJECTED'"])
        .output()
        .expect("bwrap");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let combined = format!("{stdout} {stderr}");
    assert!(
        combined.contains("WRITE_REJECTED")
            || combined.contains("Read-only")
            || combined.contains("Permission"),
        "/usr write should fail. stdout={stdout} stderr={stderr}"
    );
}

/// Synthetic passwd: only the sandboxuser line, not the host's
/// /etc/passwd contents.
#[test]
#[ignore]
fn synthetic_passwd_hides_host_users() {
    let Some(rootfs) = skip_if_unavailable() else { return };
    let usr = rootfs.join("usr");
    // Write a synthetic passwd into a temp file to bind-mount.
    let tmp = tempfile::NamedTempFile::new().expect("tempfile");
    std::fs::write(tmp.path(), "sandboxuser:x:1001:1001:Sandbox:/home/sandboxuser:/bin/sh\n")
        .expect("write");
    let out = Command::new("bwrap")
        .args(["--unshare-user", "--uid", "1001", "--gid", "1001"])
        .args(["--ro-bind", usr.to_str().unwrap(), "/usr"])
        .args(["--ro-bind", tmp.path().to_str().unwrap(), "/etc/passwd"])
        .args(["--symlink", "usr/bin", "/bin"])
        .args(["--symlink", "usr/lib", "/lib"])
        .args(["--symlink", "usr/lib64", "/lib64"])
        .args(["--dev-bind", "/proc", "/proc"])
        .args(["--dev", "/dev"])
        .args(["--tmpfs", "/tmp"])
        .arg("--")
        .args(["/bin/sh", "-c", "cat /etc/passwd"])
        .output()
        .expect("bwrap");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("sandboxuser"), "synthetic passwd missing: {stdout}");
    // Generic check: shouldn't see any user containing "root" or
    // common host users. The synthetic file contains exactly 1 line.
    let line_count = stdout.lines().filter(|l| !l.trim().is_empty()).count();
    assert_eq!(line_count, 1, "synthetic passwd should be 1 line, got {line_count}: {stdout}");
}

/// FSIZE rlimit: writing >256 MiB to a file fails partway. We use a
/// smaller cap (1MiB) to keep the test fast, then verify dd reports
/// failure when trying to exceed it.
#[test]
#[ignore]
fn fsize_rlimit_enforced_via_prlimit() {
    let Some(rootfs) = skip_if_unavailable() else { return };
    let usr = rootfs.join("usr");
    let out = Command::new("bwrap")
        .args(["--unshare-user", "--uid", "1001", "--gid", "1001"])
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
            "--fsize=1048576", // 1 MiB cap (vs prod's 256 MiB)
            "--",
            "/bin/sh",
            "-c",
            // try to write 5 MiB — must hit the 1 MiB cap.
            // dd reports failure to stderr; exit code is nonzero.
            "dd if=/dev/zero of=/tmp/big bs=1M count=5 2>&1; ls -la /tmp/big 2>&1",
        ])
        .output()
        .expect("bwrap");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    // dd writes "File size limit exceeded" or similar; the result
    // file should be capped at ~1 MiB (1048576 bytes).
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("File size limit")
            || combined.contains("size limit")
            || combined.contains("1048576"),
        "FSIZE rlimit not enforced: stdout={stdout} stderr={stderr}"
    );
}
