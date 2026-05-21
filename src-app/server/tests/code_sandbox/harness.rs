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

// ─────────────────────────────────────────────────────────────────────
// HTTP-E2E helpers (Tier 6)
// ─────────────────────────────────────────────────────────────────────

use crate::common::{TestServer, TestServerOptions};

/// JWT signed with the TestServer's actual JWT config (NOT the
/// `test_jwt()` dev-secret default). Use this for any test where the
/// server must accept the token — the dev secret is wrong for
/// `TestServer` (which configures its own random secret).
pub fn test_server_jwt(user_id: Uuid) -> String {
    let now = Utc::now();
    let claims = TestClaims {
        sub: user_id.to_string(),
        exp: (now + Duration::seconds(300)).timestamp(),
        iat: now.timestamp(),
        iss: "ziee-chat-test".to_string(),
        aud: "ziee-chat-test-api".to_string(),
        username: String::new(),
        email: String::new(),
        is_admin: false,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(b"test-secret-key-for-jwt-tokens-min-32-chars-long"),
    )
    .expect("sign test-server jwt")
}

/// Boot a TestServer with code_sandbox enabled. Skips the test cleanly
/// (returns None) when bwrap isn't installed or no rootfs is mounted.
///
/// Use at the top of every Tier-6 test:
/// ```ignore
/// let Some(server) = enabled_test_server().await else { return };
/// ```
pub async fn enabled_test_server() -> Option<TestServer> {
    if !bwrap_available() {
        eprintln!("test skipped: bwrap not installed");
        return None;
    }
    let Some(rootfs) = rootfs_path() else {
        eprintln!(
            "test skipped: no rootfs mounted. Run `just sandbox-build && \
             just sandbox-mount` first."
        );
        return None;
    };
    Some(
        TestServer::start_with_options(TestServerOptions {
            sandbox_enabled: true,
            sandbox_rootfs: Some(rootfs),
            // Default to rlimits-only (no cgroup delegation needed).
            // Tests that need cgroup enforcement should boot their
            // own TestServer with the right cgroup_parent + first
            // call needs_cgroup_delegation().
            sandbox_cgroup_parent: String::new(),
            extra_env: Vec::new(),
        })
        .await,
    )
}

/// Conversation owned by a specific user. Inserts the row directly
/// via SQL (faster than building a chat-extension request).
pub async fn create_test_conversation(pool: &sqlx::PgPool, user_id: Uuid) -> Uuid {
    let conv_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO conversations (id, user_id, title, created_at, updated_at)
           VALUES ($1, $2, 'Test conversation', NOW(), NOW())"#,
    )
    .bind(conv_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("insert conversation");
    conv_id
}

/// Insert a file owned by `user_id` directly into `files` table AND
/// store its bytes via the filesystem storage at the server's data dir.
/// Returns the file_id. Use `attach_to_conversation` separately to wire
/// it into a conversation's message contents.
pub async fn create_test_file(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    filename: &str,
    bytes: &[u8],
    server_data_dir: &std::path::Path,
) -> Uuid {
    let file_id = Uuid::new_v4();
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("bin");
    let mime = mime_guess::from_path(filename)
        .first_or_octet_stream()
        .essence_str()
        .to_string();
    sqlx::query(
        r#"INSERT INTO files (id, user_id, filename, mime_type, size_bytes, created_at)
           VALUES ($1, $2, $3, $4, $5, NOW())"#,
    )
    .bind(file_id)
    .bind(user_id)
    .bind(filename)
    .bind(&mime)
    .bind(bytes.len() as i64)
    .execute(pool)
    .await
    .expect("insert file");
    // Write the bytes to the server's filesystem storage at the path
    // the FileStorage trait expects (originals/<user>/<file>.<ext>).
    let dest = server_data_dir
        .join("originals")
        .join(user_id.to_string())
        .join(format!("{file_id}.{ext}"));
    std::fs::create_dir_all(dest.parent().unwrap()).expect("mkdir originals");
    std::fs::write(&dest, bytes).expect("write file bytes");
    file_id
}

/// POST a JSON-RPC envelope to the code_sandbox endpoint. Returns the
/// raw reqwest::Response so callers can inspect status + body.
///
/// Uses a 120-second per-request timeout so a hung sandbox call
/// surfaces as a test failure (panic on `.expect("post jsonrpc")`)
/// rather than hanging the whole test run for the server's 10-min
/// wall-clock timeout. Tests that explicitly need a longer ceiling
/// (e.g. the timeout-flag test) should use the `_with_timeout`
/// variant.
pub async fn post_jsonrpc(
    server: &TestServer,
    jwt: &str,
    conv_id: Option<Uuid>,
    method: &str,
    params: serde_json::Value,
) -> reqwest::Response {
    post_jsonrpc_with_timeout(
        server,
        jwt,
        conv_id,
        method,
        params,
        std::time::Duration::from_secs(120),
    )
    .await
}

pub async fn post_jsonrpc_with_timeout(
    server: &TestServer,
    jwt: &str,
    conv_id: Option<Uuid>,
    method: &str,
    params: serde_json::Value,
    timeout: std::time::Duration,
) -> reqwest::Response {
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .expect("reqwest client");
    let mut req = client
        .post(format!("{}/api/code-sandbox", server.base_url))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        }));
    if let Some(c) = conv_id {
        req = req.header("x-conversation-id", c.to_string());
    }
    req.send().await.expect("post jsonrpc")
}

/// Convenience wrapper: POST `tools/call <name>` with `arguments` and
/// return the parsed JSON-RPC body. Asserts HTTP 200 — use the lower-
/// level `post_jsonrpc` if the test expects non-200.
pub async fn tool_call(
    server: &TestServer,
    jwt: &str,
    conv_id: Uuid,
    tool: &str,
    arguments: serde_json::Value,
) -> serde_json::Value {
    let resp = post_jsonrpc(
        server,
        jwt,
        Some(conv_id),
        "tools/call",
        serde_json::json!({ "name": tool, "arguments": arguments }),
    )
    .await;
    let status = resp.status();
    let body = resp.text().await.expect("read body");
    assert!(
        status.is_success(),
        "tool/call {tool} returned {status}: {body}"
    );
    serde_json::from_str(&body).expect("parse jsonrpc body")
}
