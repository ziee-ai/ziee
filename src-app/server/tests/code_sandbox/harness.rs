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

/// The published rootfs repo + the release tag the tests pin to.
/// Override the tag with `ZIEE_SANDBOX_TEST_TAG` to test a different
/// release. Tests fetch the real GitHub-published, cosign-signed rootfs
/// — there is no locally-built fixture anymore.
pub const TEST_ROOTFS_REPO: &str = "ziee-ai/sandbox-rootfs";
pub const TEST_ROOTFS_TAG: &str = "v0.0.3-alpha";

pub fn test_rootfs_tag() -> String {
    std::env::var("ZIEE_SANDBOX_TEST_TAG").unwrap_or_else(|_| TEST_ROOTFS_TAG.to_string())
}

/// Our arch token as it appears in the published asset names.
pub fn test_arch_token() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        other => panic!("unsupported test arch {other:?} (need x86_64 or aarch64)"),
    }
}

/// Shared, persistent cache dir under the repo root. Both the
/// bwrap-direct (Tier 4) download and the server-fetched (Tier 6) cache
/// live here so a single download serves the whole suite.
pub fn test_cache_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root")
        .join(".ziee-cache")
        .join("sandbox-rootfs")
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

/// Blocking HTTP GET, run on a fresh thread so `reqwest::blocking`
/// never sees the ambient `#[tokio::test]` runtime (which would panic
/// with "Cannot start a runtime from within a runtime").
fn http_get_blocking(url: &str) -> Result<Vec<u8>, String> {
    let url = url.to_string();
    std::thread::spawn(move || -> Result<Vec<u8>, String> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("ziee-sandbox-tests")
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| format!("build client: {e}"))?;
        let resp = client.get(&url).send().map_err(|e| format!("GET {url}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("GET {url}: HTTP {}", resp.status()));
        }
        resp.bytes()
            .map(|b| b.to_vec())
            .map_err(|e| format!("read body {url}: {e}"))
    })
    .join()
    .map_err(|_| "download thread panicked".to_string())?
}

/// Download (once, cached) a published rootfs asset from the
/// `ziee-ai/sandbox-rootfs` GitHub release for the pinned tag, verifying
/// it against the published `.sha256` sidecar. Returns the cached file
/// path. The cached file is reused on subsequent runs (sha-checked), so
/// the 74 MB minimal squashfs is fetched at most once per machine/tag.
///
/// Panics with actionable guidance on network / verification failure —
/// tests must not silently skip just because GitHub was unreachable.
pub fn ensure_github_asset(flavor: &str, ext: &str) -> PathBuf {
    let tag = test_rootfs_tag();
    let arch = test_arch_token();
    let asset = format!("ziee-sandbox-rootfs-{arch}-{flavor}.{ext}");
    let cache = test_cache_dir();
    std::fs::create_dir_all(&cache).expect("create test cache dir");
    // Tag-prefixed so two pinned tags don't clobber each other.
    let dest = cache.join(format!("{tag}-{asset}"));

    let base = format!("https://github.com/{TEST_ROOTFS_REPO}/releases/download/{tag}");

    // The `.sha256` sidecar is `sha256sum` format: `<hex>␠␠<path>`.
    let sidecar = http_get_blocking(&format!("{base}/{asset}.sha256")).unwrap_or_else(|e| {
        panic!(
            "could not fetch sha256 sidecar for {asset} from {TEST_ROOTFS_REPO}@{tag}: {e}\n  \
             (the integration tests download the real published rootfs; ensure network access \
             and that the tag exists, or set ZIEE_SANDBOX_TEST_TAG / ZIEE_SANDBOX_TEST_SQUASHFS)"
        )
    });
    let expected_sha = String::from_utf8_lossy(&sidecar)
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_lowercase();
    assert_eq!(expected_sha.len(), 64, "malformed sha256 sidecar for {asset}");

    // Cache hit?
    if dest.is_file() {
        if let Ok(bytes) = std::fs::read(&dest) {
            if sha256_hex(&bytes) == expected_sha {
                return dest;
            }
        }
    }

    // Download + verify + atomic-rename into place.
    let bytes = http_get_blocking(&format!("{base}/{asset}"))
        .unwrap_or_else(|e| panic!("download {asset} from {TEST_ROOTFS_REPO}@{tag}: {e}"));
    let actual = sha256_hex(&bytes);
    assert_eq!(
        actual, expected_sha,
        "sha256 mismatch for {asset} (expected {expected_sha}, got {actual})"
    );
    let tmp = dest.with_extension("download.tmp");
    std::fs::write(&tmp, &bytes).expect("write downloaded asset");
    std::fs::rename(&tmp, &dest).expect("rename downloaded asset into cache");
    dest
}

/// Returns the test rootfs squashfs FILE path (not a mount dir).
/// Used by `run_in_sandbox()` on Mac/Windows where the backend passes
/// the squashfs to libkrun/WSL2 as a virtio-blk disk; on Linux the
/// backend mounts it via squashfuse.
///
/// Fetches the real published `minimal` squashfs from the
/// `ziee-ai/sandbox-rootfs` GitHub release (cached). Override the file
/// with `ZIEE_SANDBOX_TEST_SQUASHFS` or the flavor with
/// `ZIEE_SANDBOX_FLAVOR`.
pub fn rootfs_squashfs_path() -> PathBuf {
    if let Ok(p) = std::env::var("ZIEE_SANDBOX_TEST_SQUASHFS") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return pb;
        }
    }
    let flavor = std::env::var("ZIEE_SANDBOX_FLAVOR").unwrap_or_else(|_| "minimal".to_string());
    ensure_github_asset(&flavor, "squashfs")
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
/// On Linux the harness squashfuse-mounts the downloaded rootfs and
/// rewrites the `/sandbox-rootfs` argv prefix to that mount point
/// (the Linux `exec_raw_argv` runs the argv verbatim). On Mac/Windows
/// the in-VM agent has the rootfs at `/sandbox-rootfs` already, so the
/// argv is passed through unchanged with the squashfs file handed to
/// the VM backend.
pub async fn run_in_sandbox(
    argv: Vec<String>,
    timeout: std::time::Duration,
) -> Result<ziee::RawExecResult, ziee::AppError> {
    #[cfg(target_os = "linux")]
    {
        let mount = ensure_test_rootfs_mounted();
        let mount_str = mount.to_string_lossy().into_owned();
        // The tier-4 argv references the rootfs as `/sandbox-rootfs`
        // (the canonical in-sandbox path). On the host that path
        // doesn't exist, so rewrite any argv element whose prefix is
        // `/sandbox-rootfs` to the actual squashfuse mount dir.
        let argv: Vec<String> = argv
            .into_iter()
            .map(|a| match a.strip_prefix("/sandbox-rootfs") {
                Some(rest) => format!("{mount_str}{rest}"),
                None => a,
            })
            .collect();
        return ziee::sandbox_backend()
            .exec_raw_argv(argv, &mount, timeout)
            .await;
    }
    #[cfg(not(target_os = "linux"))]
    {
        let rootfs = rootfs_squashfs_path();
        ziee::sandbox_backend()
            .exec_raw_argv(argv, &rootfs, timeout)
            .await
    }
}

/// Linux: squashfuse-mount the downloaded rootfs squashfs to a shared,
/// persistent dir and return the mount point. Idempotent — reuses an
/// existing mount (detected by a visible `usr/`). `run_in_sandbox`
/// rewrites the tier-4 `/sandbox-rootfs` argv prefix to this path.
#[cfg(target_os = "linux")]
pub fn ensure_test_rootfs_mounted() -> PathBuf {
    let sqfs = rootfs_squashfs_path();
    // Tag-specific mount point so a tag bump doesn't reuse a stale
    // mount of the previous release's squashfs.
    let mount = test_cache_dir().join(format!("test-mount-{}", test_rootfs_tag()));
    std::fs::create_dir_all(&mount).expect("create test mount dir");
    if mount.join("usr").is_dir() {
        return mount; // already mounted
    }
    let status = Command::new("squashfuse")
        .arg(&sqfs)
        .arg(&mount)
        .status()
        .expect("spawn squashfuse (apt install squashfuse fuse3)");
    assert!(
        status.success(),
        "squashfuse mount of {} at {} failed",
        sqfs.display(),
        mount.display()
    );
    assert!(
        mount.join("usr").is_dir(),
        "rootfs mounted at {} but no usr/ inside — wrong squashfs?",
        mount.display()
    );
    mount
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

/// Boot a TestServer with code_sandbox enabled, letting the server
/// fetch the pinned rootfs from the `ziee-ai/sandbox-rootfs` GitHub
/// release on its first `execute_command` — the real production path
/// (GitHub Releases discovery → download → sha256 + cosign verify →
/// squashfuse/VM mount). No local fixture, no fake `known_revisions`
/// staging.
///
/// Speed: all Tier-6 servers share ONE persistent cache dir
/// (`.ziee-cache/sandbox-rootfs/e2e`). The FIRST test in a run does the
/// real download + cosign verify; every later test (fresh DB) finds the
/// cached squashfs and `install_version`'s on-disk adoption mounts it
/// without re-downloading the 74 MB asset.
///
/// Returns `None` (skips cleanly) only when the host genuinely can't
/// run the sandbox — on Linux, when bwrap isn't installed. On
/// macOS/Windows the bwrap runs inside the libkrun/WSL2 guest, so the
/// host bwrap check doesn't apply.
///
/// Use at the top of every Tier-6 test:
/// ```ignore
/// let Some(server) = enabled_test_server().await else { return };
/// ```
pub async fn enabled_test_server() -> Option<TestServer> {
    Some(TestServer::start_with_options(github_fetch_server_options(Vec::new())?).await)
}

/// Build the sandbox-enabled `TestServerOptions` that let the server
/// fetch the pinned rootfs from the GitHub release (shared e2e cache).
/// Callers append their own `extra_env` (API keys, sentinels). Returns
/// `None` to skip when the host can't run the sandbox (Linux w/o bwrap).
///
/// `runtime_mount::derive_cache_dir` takes `.parent()` of `rootfs_path`,
/// so `<e2e>/current` makes the derived cache dir `<e2e>` — where the
/// server downloads (and later adopts via `install_version`)
/// `<version>/ziee-sandbox-rootfs-<arch>-<flavor>.<ext>`.
pub fn github_fetch_server_options(
    mut extra_env: Vec<(String, String)>,
) -> Option<TestServerOptions> {
    #[cfg(target_os = "linux")]
    if !bwrap_available() {
        eprintln!("test skipped: bwrap not installed (apt install bubblewrap)");
        return None;
    }

    // Skip when no rootfs asset is published for this host's arch.
    // `ziee-ai/sandbox-rootfs` publishes only `x86_64-*.squashfs`
    // assets today (every tag through v1.0.0-alpha). On aarch64 hosts
    // (Apple Silicon Macs, ARM Linux) the server's auto-fetch hits
    // 404 on `ziee-sandbox-rootfs-aarch64-<flavor>.squashfs` and the
    // sandbox-spawned MCP server never starts — tools/list returns
    // empty, the LLM sees no tools, the assert fails. Once aarch64
    // rootfs is published, drop this gate.
    if test_arch_token() != "x86_64" {
        eprintln!(
            "test skipped: ziee-ai/sandbox-rootfs publishes no {}-arch assets yet \
             (rootfs system is ready, but the released rootfs is x86_64-only)",
            test_arch_token()
        );
        return None;
    }

    let e2e_cache = test_cache_dir().join("e2e");
    std::fs::create_dir_all(&e2e_cache).ok()?;
    let rootfs_path = e2e_cache.join("current");

    // Pin discovery to the tested tag so the suite doesn't drift onto a
    // newer release the moment one is published.
    extra_env.push((
        "CODE_SANDBOX_PIN_VERSION".to_string(),
        test_rootfs_tag().trim_start_matches('v').to_string(),
    ));

    Some(TestServerOptions {
        sandbox_enabled: true,
        rate_limit: None,
        sandbox_rootfs: Some(rootfs_path),
        sandbox_cgroup_parent: String::new(),
        extra_env,
        sandbox_cache_tempdir: None,
        use_desktop_binary: false,
        sandbox_public_base_url: None,
    })
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
