//! Integration tests for the `voice` dictation module (Postgres + TestServer).
//!
//! Maps to TESTS.md TEST-11..21 + TEST-33. The philosophy (from the plan) is
//! REAL end-to-end paths — mock only the external boundary:
//!   - `transcribe_test` auto-starts a REAL `stub-whisper-server` subprocess via
//!     the production deployment path and forwards a fixture WAV to `/inference`;
//!   - `version_download_test` drives the FULL resolve→download→verify→extract→
//!     register pipeline against a loopback `MockReleaseServer` (mirror seams),
//!     with no network and no paid credentials.
//!
//! Per-test `app.data_dir` isolation: the harness points the spawned server's
//! `app.data_dir` at a fresh per-test TempDir exposed via `server.data_dir()`.
//! The voice model cache (`<app_data>/voice-models/`) and whisper binary cache
//! (`<app_data>/whisper-runtime/binaries/`) therefore live under that TempDir,
//! so a test can stage a model file / read a downloaded binary at the same path
//! the server uses, and the whole tree is reaped when the TestServer drops.

use std::path::PathBuf;
use std::sync::OnceLock;

use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use crate::common::TestServer;

pub mod mock_release;

mod capability_test;
mod config_gate_test;
mod lifecycle_test;
mod model_test;
mod permissions_test;
mod real_repo_test;
mod settings_test;
mod transcribe_test;
mod version_download_test;
mod version_update_test;

/// The two admin permissions the voice admin surface gates on. Admins hold these
/// via the `*` wildcard in production; a test grants them explicitly.
pub const VOICE_ADMIN_PERMS: &[&str] = &["voice::admin::read", "voice::admin::manage"];

/// Host platform token (`linux`/`macos`/`windows`) — matches
/// `binary_manager::host_platform()` / `gpu_detect`'s default arms, which is
/// what `runtime_ready()` filters on and what the download path registers.
pub fn host_platform() -> String {
    if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        panic!("unsupported platform for voice tests")
    }
    .to_string()
}

/// Host arch token (`x86_64`/`aarch64`).
pub fn host_arch() -> String {
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        panic!("unsupported arch for voice tests")
    }
    .to_string()
}

/// Locate (building on demand) the `stub-whisper-server` test binary. Resolved
/// relative to the running test binary's own `target/<profile>/` dir so it works
/// regardless of a `CARGO_TARGET_DIR` override. Cached after first resolution.
pub fn stub_whisper_binary() -> PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let exe = if cfg!(windows) {
            "stub-whisper-server.exe"
        } else {
            "stub-whisper-server"
        };
        // The integration-test binary lives at `<target>/<profile>/deps/…`; the
        // built `stub-whisper-server` sits two levels up in `<target>/<profile>/`.
        let target_profile_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf())) // deps/
            .and_then(|d| d.parent().map(|d| d.to_path_buf())) // <profile>/
            .expect("resolve target/<profile> dir from current_exe");
        let candidate = target_profile_dir.join(exe);
        if candidate.exists() {
            return candidate;
        }

        // Build it on demand (cargo inherits CARGO_TARGET_DIR from the env, so the
        // artifact lands in the same target dir the test binary is in). cwd = the
        // `src-app` workspace root (CARGO_MANIFEST_DIR = src-app/server).
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest.parent().expect("src-app dir").to_path_buf();
        eprintln!("stub-whisper-server not built; running `cargo build -p stub-whisper-server`…");
        let status = std::process::Command::new(env!("CARGO"))
            .args(["build", "-p", "stub-whisper-server"])
            .current_dir(&workspace_root)
            .status()
            .expect("spawn cargo build -p stub-whisper-server");
        assert!(status.success(), "cargo build -p stub-whisper-server failed");
        assert!(
            candidate.exists(),
            "stub-whisper-server binary missing at {} after build",
            candidate.display()
        );
        candidate
    })
    .clone()
}

/// Build a minimal 16 kHz mono 16-bit PCM WAV of `secs` seconds of silence
/// (a real RIFF/WAVE container — passes the transcribe handler's magic-byte
/// sniff + duration parser). Mirrors the `transcribe.rs` unit-test fixture.
pub fn make_wav(secs: f64) -> Vec<u8> {
    let sample_rate = 16_000u32;
    let channels = 1u16;
    let bits = 16u16;
    let byte_rate = sample_rate * channels as u32 * (bits / 8) as u32;
    let data_len = (byte_rate as f64 * secs) as u32;
    let mut w = Vec::new();
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data_len).to_le_bytes());
    w.extend_from_slice(b"WAVE");
    w.extend_from_slice(b"fmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&channels.to_le_bytes());
    w.extend_from_slice(&sample_rate.to_le_bytes());
    w.extend_from_slice(&byte_rate.to_le_bytes());
    w.extend_from_slice(&(channels * bits / 8).to_le_bytes());
    w.extend_from_slice(&bits.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data_len.to_le_bytes());
    w.extend(std::iter::repeat(0u8).take(data_len as usize));
    w
}

/// Stage a whisper ggml model file (non-empty bytes) at the exact on-disk path
/// the server resolves (`<app_data>/voice-models/ggml-<name>.bin`). This is the
/// air-gap pre-stage path: `model::ensure_model` short-circuits on a present
/// file, so no download (and therefore no sha256 pin check) runs.
pub fn stage_model(server: &TestServer, name: &str) -> PathBuf {
    let dir = server.data_dir().join("voice-models");
    std::fs::create_dir_all(&dir).expect("create voice-models dir");
    let path = dir.join(format!("ggml-{name}.bin"));
    // Any non-empty bytes: `model_present` only checks the file exists + len > 0.
    std::fs::write(&path, b"stub ggml model bytes").expect("write staged model");
    path
}

/// Insert a `voice_runtime_versions` row pointing at `binary_path`, returning the
/// row id. `system_default=true` makes `select_version()` pick it (what the
/// transcribe auto-start path resolves).
pub async fn insert_version_row(
    server: &TestServer,
    version: &str,
    backend: &str,
    binary_path: &str,
    system_default: bool,
) -> Uuid {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&server.database_url)
        .await
        .expect("connect test db to insert voice version");
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO voice_runtime_versions
           (version, platform, arch, backend, binary_path, is_system_default)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id",
    )
    .bind(version)
    .bind(host_platform())
    .bind(host_arch())
    .bind(backend)
    .bind(binary_path)
    .bind(system_default)
    .fetch_one(&pool)
    .await
    .expect("insert voice_runtime_versions row");
    pool.close().await;
    id
}

/// Drive a voice download-events SSE stream to a terminal frame. Returns
/// `Ok(())` on `event: complete`, `Err(msg)` on `event: failed`. Panics on
/// timeout / stream close without a terminal frame.
///
/// The download-events SSE uses camelCase event names from `sse_event_enum!`:
/// `connected` / `progress` / `complete` / `failed`.
pub async fn drive_download_to_terminal(
    server: &TestServer,
    admin_token: &str,
    key: &str,
    timeout: std::time::Duration,
) -> Result<(), String> {
    use tokio_stream::StreamExt;

    let url = server.api_url(&format!("/voice/versions/downloads/{key}/events"));
    let resp = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {admin_token}"))
        .send()
        .await
        .expect("subscribe to download events");
    assert_eq!(resp.status(), 200, "download-events SSE should return 200");

    let deadline = tokio::time::Instant::now() + timeout;
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut saw_progress = false;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            panic!("timed out waiting for a terminal download SSE frame (saw_progress={saw_progress})");
        }
        let chunk = match tokio::time::timeout(remaining, stream.next()).await {
            Ok(Some(Ok(c))) => c,
            Ok(Some(Err(e))) => panic!("SSE stream error: {e}"),
            Ok(None) => panic!("SSE stream closed before a terminal frame"),
            Err(_) => panic!("timed out reading the download SSE stream"),
        };
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(pos) = buf.find("\n\n") {
            let frame: String = buf.drain(..pos + 2).collect();
            let event = frame
                .lines()
                .find_map(|l| l.strip_prefix("event:").map(|r| r.trim().to_string()));
            match event.as_deref() {
                Some("progress") => saw_progress = true,
                Some("complete") => return Ok(()),
                Some("failed") => {
                    let data = frame
                        .lines()
                        .find_map(|l| l.strip_prefix("data:").map(|r| r.trim().to_string()))
                        .unwrap_or_default();
                    return Err(data);
                }
                _ => {}
            }
        }
    }
}
