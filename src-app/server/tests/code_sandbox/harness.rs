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
        iss: "ziee".to_string(),
        aud: "ziee-api".to_string(),
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
///
/// Linux only: the path points at a mounted FUSE directory (where
/// `./usr/` etc. are readable from the host). On Mac/Windows the
/// rootfs is a squashfs file that the libkrun/WSL2 backend mounts
/// inside the guest — use `rootfs_squashfs_path()` instead.
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

/// Returns the test rootfs squashfs FILE path (not a mount dir).
/// Used by `run_in_sandbox()` on Mac/Windows where the backend
/// passes the squashfs to libkrun/WSL2 as a virtio-blk disk; on
/// Linux the backend ignores it (host bwrap reads the mount).
///
/// Built by `scripts/build-test-rootfs.sh` (`just test-prereqs`).
/// Panics with a clear message if missing — tests must not silently
/// skip; the runner ensures the prereq is present.
pub fn rootfs_squashfs_path() -> PathBuf {
    if let Ok(p) = std::env::var("ZIEE_SANDBOX_TEST_SQUASHFS") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return pb;
        }
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidate = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root")
        .join(".ziee-cache")
        .join("sandbox-rootfs")
        .join("test-minimal.squashfs");
    if !candidate.is_file() {
        panic!(
            "test rootfs squashfs missing at {}.\n  \
             Build it with: just test-prereqs\n  \
             Or directly: scripts/build-test-rootfs.sh",
            candidate.display()
        );
    }
    candidate
}

/// `true` when bwrap is on PATH and runnable. Linux-only check;
/// Mac/Windows don't have host bwrap (it runs in the libkrun/WSL2
/// guest), so callers should use `run_in_sandbox()` instead.
pub fn bwrap_available() -> bool {
    Command::new("bwrap").arg("--version").output().is_ok()
}

/// Execute a raw bwrap argv inside the active platform's sandbox
/// dispatch path. The implementation detail of how bwrap actually
/// runs (host process on Linux; libkrun VM on Mac; WSL2 distro on
/// Windows) is owned by the SandboxBackend impl.
///
/// **Argv path conventions** — to be platform-portable, paths in
/// the argv must reference the SANDBOX'S view of the filesystem,
/// not the host's:
///   - `/sandbox-rootfs/usr` (the agent mounts the squashfs there)
///   - `/workspace/<file>` (the agent mounts the virtio-fs share there)
///   - `/proc`, `/dev`, `/tmp` (Linux primitives; exist inside the VM)
///
/// On Linux the active backend translates `/sandbox-rootfs` to the
/// mounted FUSE path. On Mac/Windows the in-VM agent has the rootfs
/// at `/sandbox-rootfs` already.
pub async fn run_in_sandbox(
    argv: Vec<String>,
    timeout: std::time::Duration,
) -> Result<ziee::RawExecResult, ziee::AppError> {
    let rootfs = rootfs_squashfs_path();
    ziee::sandbox_backend()
        .exec_raw_argv(argv, &rootfs, timeout)
        .await
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
                 the systemd Slice / docker cgroup_parent must be configured for this test"
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

// NOTE: the harness-uses-production-argv regression test lives in
// `src/modules/code_sandbox/sandbox.rs` as a `#[cfg(test)]` unit test
// (`argv_includes_security_critical_flags`). It can't live here
// because integration tests linking the build_bwrap_argv via
// `pub use` causes linkme to detect duplicate MODULE_ENTRIES slice
// registrations at runtime (sequence of `pub use` re-exports from
// `lib.rs` somehow forces the section to be emitted twice).

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
        // Match the TestServer's JWT config (which uses prod issuer/
        // audience values for MCP loopback compatibility — see
        // tests/common/mod.rs).
        iss: "ziee".to_string(),
        aud: "ziee-api".to_string(),
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

/// Stage the test squashfs as a "minimal" flavor in a fresh cache dir
/// + write a matching `known_revisions.dev.toml`. Returns the cache
/// TempDir (held by caller so it outlives the TestServer) and the env
/// vars to pass to TestServer. Mac/Windows path — bypasses the
/// `runtime_fetch` network path via cache-hit + sha256 short-circuit.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn stage_test_rootfs_for_e2e() -> Option<(tempfile::TempDir, Vec<(String, String)>)> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    fn sha256_file(p: &std::path::Path) -> Option<String> {
        let mut f = std::fs::File::open(p).ok()?;
        let mut h = Sha256::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = f.read(&mut buf).ok()?;
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
        }
        Some(format!("{:x}", h.finalize()))
    }

    let source_sqfs = rootfs_squashfs_path();
    let arch = std::env::consts::ARCH;
    let cache = tempfile::tempdir().ok()?;
    let sqfs_asset = format!("ziee-sandbox-rootfs-v1.r0-{arch}-minimal.squashfs");
    std::fs::copy(&source_sqfs, cache.path().join(&sqfs_asset)).ok()?;
    let sha256_sqfs = sha256_file(&source_sqfs)?;

    // Windows: also stage the `.tar.zst` (the production WSL2 backend
    // imports tar formats via `wsl --import` and asks runtime_fetch for
    // `RootfsFormat::TarZst`, which requires `sha256_tar_zst` in the
    // revision row). build-test-rootfs.sh publishes both formats side
    // by side; `resolve_tarball_for_rootfs` (in wsl2.rs) finds the
    // sibling for the exec_raw_argv path, and we stage it here for the
    // production-handler path.
    let tar_zst_line = {
        #[cfg(target_os = "windows")]
        {
            let source_tar_zst = source_sqfs.with_extension("tar.zst");
            if source_tar_zst.is_file() {
                let tar_asset = format!("ziee-sandbox-rootfs-v1.r0-{arch}-minimal.tar.zst");
                std::fs::copy(&source_tar_zst, cache.path().join(&tar_asset)).ok()?;
                let sha = sha256_file(&source_tar_zst)?;
                format!("sha256_tar_zst = \"{sha}\"\n")
            } else {
                // Tarball not built — Tier 6 production-handler tests will
                // surface a clear "no sha256_tar_zst" error pointing the
                // operator at `scripts/build-test-rootfs.sh`. exec_raw_argv
                // (Tier 4) doesn't depend on it.
                String::new()
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            String::new()
        }
    };

    let dev_toml = format!(
        "\n\
[[revision]]\n\
schema = 1\n\
revision = \"r0\"\n\
arch = \"{arch}\"\n\
flavor = \"minimal\"\n\
sha256 = \"{sha256_sqfs}\"\n\
{tar_zst_line}\
signed = false\n\
yanked = false\n"
    );
    let dev_toml_path = cache.path().join("known_revisions.dev.toml");
    std::fs::write(&dev_toml_path, dev_toml).ok()?;
    let env = vec![(
        "CODE_SANDBOX_KNOWN_REVISIONS_OVERRIDE".to_string(),
        dev_toml_path.to_string_lossy().into_owned(),
    )];
    Some((cache, env))
}

/// Boot a TestServer with code_sandbox enabled. Cross-platform:
///   - Linux: requires bwrap + a host-mounted rootfs (existing path).
///   - Mac/Windows: stages the test squashfs into a TempDir cache +
///     writes `known_revisions.dev.toml` so `runtime_fetch` hits the
///     cache without going to the network; the active backend
///     (libkrun on Mac, WSL2 on Windows) then dispatches bwrap
///     inside the VM/distro.
/// Returns None only when the test rootfs isn't built — caller
/// should panic with `just test-prereqs` guidance (we no longer skip
/// silently).
///
/// Use at the top of every Tier-6 test:
/// ```ignore
/// let Some(server) = enabled_test_server().await else { return };
/// ```
pub async fn enabled_test_server() -> Option<TestServer> {
    #[cfg(target_os = "linux")]
    {
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
        return Some(
            TestServer::start_with_options(TestServerOptions {
                sandbox_enabled: true,
                sandbox_rootfs: Some(rootfs),
                sandbox_cgroup_parent: String::new(),
                extra_env: Vec::new(),
                sandbox_cache_tempdir: None,
            })
            .await,
        );
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        // Stage the test squashfs as the "minimal" flavor in a fresh
        // cache + write a matching known_revisions.toml. The cache
        // TempDir must outlive TestServer — we pass it in via the
        // sandbox_cache_tempdir field on TestServerOptions, and
        // TestServer holds it through the test lifetime.
        //
        // `runtime_mount::derive_cache_dir` does `.parent()` on
        // `rootfs_path` (it expects `<cache>/current`-shape), so we
        // hand it `<tempdir>/current` and stage the squashfs in
        // `<tempdir>` so the derived cache dir resolves correctly.
        let (cache, env) = stage_test_rootfs_for_e2e()
            .expect("stage test rootfs (run `just test-prereqs`)");
        let rootfs_path = cache.path().join("current");
        return Some(
            TestServer::start_with_options(TestServerOptions {
                sandbox_enabled: true,
                sandbox_rootfs: Some(rootfs_path),
                sandbox_cgroup_parent: String::new(),
                extra_env: env,
                sandbox_cache_tempdir: Some(std::sync::Arc::new(cache)),
            })
            .await,
        );
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        eprintln!("test skipped: unsupported platform");
        None
    }
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
    // Capture content-type BEFORE consuming the body — `execute_command`
    // returns `text/event-stream` (since the Plan 2 download-consent +
    // progress work; see `streaming::execute_command_stream`), while every
    // other tool returns `application/json`. Parse accordingly.
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = resp.text().await.expect("read body");
    assert!(
        status.is_success(),
        "tool/call {tool} returned {status}: {body}"
    );
    if content_type.contains("text/event-stream") {
        parse_jsonrpc_from_sse(tool, &body)
    } else {
        serde_json::from_str(&body)
            .unwrap_or_else(|e| panic!("parse jsonrpc body for {tool}: {e}; body={body}"))
    }
}

/// Walk an SSE response body, find the LAST `data:` payload that decodes as
/// a JSON-RPC envelope with a `result` or `error` field, and return that.
/// Intermediate frames (progress notifications, elicitation requests) are
/// skipped because the Tier-6 callers care only about the terminal result.
fn parse_jsonrpc_from_sse(tool: &str, body: &str) -> serde_json::Value {
    // SSE event blocks are separated by blank lines; per-line `data:` payloads
    // within a block are concatenated. Normalize CRLF→LF before splitting so
    // a stray `\r\n` separator doesn't merge blocks.
    let normalized = body.replace("\r\n", "\n").replace('\r', "\n");
    let mut final_envelope: Option<serde_json::Value> = None;
    for block in normalized.split("\n\n") {
        let data: String = block
            .lines()
            .filter_map(|l| l.strip_prefix("data:").or_else(|| l.strip_prefix("data: ")))
            .map(str::trim_start)
            .collect::<Vec<_>>()
            .join("\n");
        if data.is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(&data) {
            Ok(v) => v,
            Err(_) => continue,
        };
        // A JSON-RPC result/error envelope has an `id` and either `result` or
        // `error`. Progress notifications have a `method` field instead.
        if v.get("id").is_some() && (v.get("result").is_some() || v.get("error").is_some()) {
            final_envelope = Some(v);
        }
    }
    final_envelope.unwrap_or_else(|| {
        panic!(
            "tool/call {tool} returned SSE with no JSON-RPC result/error envelope; body={body}"
        )
    })
}
