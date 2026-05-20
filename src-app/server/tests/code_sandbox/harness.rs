//! Test harness conveniences for code_sandbox tests.
//!
//! Plan Phase 9 lists this file as the source of `test_sandbox()`,
//! `test_jwt()`, `test_client()`, and the `#[needs_*]` runtime-skip
//! markers. All helpers are pub so any tier 2-4 test file can pull them
//! in with `use crate::code_sandbox::harness::*;`.
//!
//! The `#[needs_*]` markers are runtime-skip functions (not attribute
//! macros — Rust attribute macros require a proc-macro crate and the
//! complexity isn't justified for our use case). Each returns true
//! when the test should run, false when it should skip cleanly.

#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;

use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// JWT claims structure matching `crate::modules::auth::jwt::Claims`.
/// We can't import the private type into integration tests, so we
/// re-declare it here. The shape must stay in sync.
#[derive(Debug, Serialize, Deserialize)]
struct TestClaims {
    sub: String,
    exp: i64,
    iat: i64,
    iss: String,
    aud: String,
    username: String,
    email: String,
    is_admin: bool,
}

/// Sign a short-lived JWT compatible with the dev/test JWT config.
/// Uses the canonical dev secret from src-app/server/config/dev.yaml
/// (override via TEST_JWT_SECRET env var to match a different config).
pub fn test_jwt(user_id: Uuid, _conv_id: Uuid) -> String {
    let secret = std::env::var("TEST_JWT_SECRET")
        .unwrap_or_else(|_| "dev-secret-change-in-production-min-32-chars-long".to_string());
    let now = Utc::now();
    let claims = TestClaims {
        sub: user_id.to_string(),
        exp: (now + Duration::seconds(60)).timestamp(),
        iat: now.timestamp(),
        iss: "ziee-chat".to_string(),
        aud: "ziee-chat-api".to_string(),
        username: String::new(),
        email: String::new(),
        is_admin: false,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .expect("sign test jwt")
}

/// Returns the rootfs mount path probed from the standard locations.
/// `None` when no rootfs is mounted — callers should skip the test
/// with `eprintln!("test skipped: …")`.
pub fn rootfs_path() -> Option<PathBuf> {
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

/// `true` when bwrap is on PATH and runnable.
pub fn bwrap_available() -> bool {
    Command::new("bwrap").arg("--version").output().is_ok()
}

/// Runtime skip helper for tests that need the FULL flavor (numpy /
/// torch / etc). Reads `ZIEE_SANDBOX_FLAVOR`; returns true to proceed
/// when flavor is "full" or unset (assume full unless told otherwise).
pub fn needs_full_rootfs() -> bool {
    match std::env::var("ZIEE_SANDBOX_FLAVOR") {
        Ok(v) if v == "minimal" => {
            eprintln!("test skipped: ZIEE_SANDBOX_FLAVOR=minimal but test needs full");
            false
        }
        _ => true,
    }
}

/// Runtime skip helper for tests that need a delegated cgroup parent
/// the test runner can write to. Detected by mkdir/rmdir under the
/// configured `CODE_SANDBOX_CGROUP_PARENT` (or the default).
pub fn needs_cgroup_delegation() -> bool {
    let parent = std::env::var("CODE_SANDBOX_CGROUP_PARENT")
        .unwrap_or_else(|_| "/sys/fs/cgroup/ziee-sandbox.slice".to_string());
    let probe = PathBuf::from(&parent).join(".harness-probe");
    let _ = std::fs::remove_dir(&probe);
    match std::fs::create_dir(&probe) {
        Ok(()) => {
            let _ = std::fs::remove_dir(&probe);
            true
        }
        Err(_) => {
            eprintln!(
                "test skipped: cgroup parent {parent} not writable by test runner; \
                 see CLAUDE.md for the systemd Slice / docker cgroup_parent setup"
            );
            false
        }
    }
}

/// Runtime skip helper for seccomp-feature tests. The libseccomp
/// crate is behind the `code_sandbox_seccomp` Cargo feature; we can't
/// query feature flags from tests without recompiling the lib, so
/// we instead probe whether the shared lib loads.
pub fn needs_seccomp_feature() -> bool {
    if !PathBuf::from("/usr/lib/x86_64-linux-gnu/libseccomp.so.2").exists()
        && !PathBuf::from("/usr/lib64/libseccomp.so.2").exists()
        && !PathBuf::from("/lib/x86_64-linux-gnu/libseccomp.so.2").exists()
    {
        eprintln!(
            "test skipped: libseccomp.so.2 not found on host; \
             `apt install libseccomp2 libseccomp-dev` to enable"
        );
        return false;
    }
    true
}

/// One-shot bwrap invocation used by tier4 hardening tests.
/// Wraps the user command in `prlimit` with the SAME rlimits the
/// production `sandbox::run_in_sandbox` applies, so test-side
/// assertions reflect the production threat model.
pub fn run_bwrap(rootfs: &PathBuf, user_cmd: &str) -> std::process::Output {
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
            user_cmd,
        ])
        .output()
        .expect("bwrap spawn")
}
