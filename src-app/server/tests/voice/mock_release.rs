//! Mock release server for whisper-server binary downloads.
//!
//! Test-side equivalent of the `ziee-ai/whisper.cpp` fork's GitHub Releases +
//! API. Packages the `stub-whisper-server` test binary as the real release
//! artifact (`whisper-server-{platform}-{arch}-cpu.tar.gz`, with the binary
//! entry named `whisper-server` — what the extractor matches on) alongside its
//! MANDATORY `.sha256` sidecar, and serves them + a `repos/{repo}/releases` list
//! + `releases/latest` from a loopback axum server. A `TestServer` is started
//! with the two debug-only mirror env vars (`WHISPER_RUNTIME_RELEASE_MIRROR`,
//! `WHISPER_RUNTIME_API_MIRROR`) so `WhisperDownloader` resolves against THIS
//! process instead of github.com.
//!
//! This exercises the FULL binary-download pipeline — resolve → download →
//! sha256-verify → extract → cache → register — with no network and no paid
//! credentials. Mirrors `tests/llm_local_runtime/mock_release.rs`.
//!
//! NOTE: builds a `tar.gz` (the non-Windows archive format). The real download
//! path runs against this mock on Linux (the CI parallel target), which is
//! exactly where the hero/download tests must run for real.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use flate2::Compression;
use flate2::write::GzEncoder;
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::{host_arch, host_platform, stub_whisper_binary};
use crate::common::{TestServer, TestServerOptions};

/// The whisper.cpp fork repo slug (matches `engine/download.rs::WHISPER_REPO`).
const WHISPER_REPO: &str = "ziee-ai/whisper.cpp";

/// Version tag every fixture artifact is published under.
pub const TEST_VERSION: &str = "v0.0.0-test";

/// A running mock release server + a TestServer wired to fetch from it.
pub struct MockReleaseServer {
    pub server: TestServer,
    pub mirror_url: String,
    pub version: String,
    pub platform: String,
    pub arch: String,
    /// sha256 of the staged whisper archive (== the served `.sha256` sidecar).
    pub archive_sha256: String,
    _serve_dir: TempDir,
    _server_handle: JoinHandle<()>,
}

impl Drop for MockReleaseServer {
    fn drop(&mut self) {
        self._server_handle.abort();
    }
}

/// Stand up the mock release server + a TestServer pointed at it (default opts).
pub async fn setup() -> MockReleaseServer {
    setup_with_options(TestServerOptions::default()).await
}

/// Like [`setup`] but with caller-provided base options; the mirror env vars are
/// appended to `opts.extra_env` so the caller can layer additional env/config.
pub async fn setup_with_options(mut opts: TestServerOptions) -> MockReleaseServer {
    let stub = stub_whisper_binary();
    let platform = host_platform();
    let arch = host_arch();

    let serve_dir = TempDir::new().expect("serve TempDir");
    let serve_root = serve_dir.path().to_path_buf();

    let archive_sha256 = stage_whisper(&serve_root, &platform, &arch, &stub);

    // Spawn the loopback file server.
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock release server");
    let port = listener.local_addr().expect("local_addr").port();
    let mirror_url = format!("http://127.0.0.1:{port}");

    let app = axum::Router::new()
        .route("/{*path}", get(serve))
        .with_state(AppState {
            serve_root: serve_root.clone(),
        });
    let server_handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });

    // Point the whisper downloader at the mock (release host + API host).
    opts.extra_env.push((
        "WHISPER_RUNTIME_RELEASE_MIRROR".to_string(),
        mirror_url.clone(),
    ));
    opts.extra_env
        .push(("WHISPER_RUNTIME_API_MIRROR".to_string(), mirror_url.clone()));
    let server = TestServer::start_with_options(opts).await;

    MockReleaseServer {
        server,
        mirror_url,
        version: TEST_VERSION.to_string(),
        platform,
        arch,
        archive_sha256,
        _serve_dir: serve_dir,
        _server_handle: server_handle,
    }
}

/// Stage the whisper release artifacts under `serve_root`, returning the
/// archive's sha256. Lays out the exact paths `WhisperDownloader` requests:
///   `{repo}/releases/download/{TEST_VERSION}/whisper-server-{platform}-{arch}-cpu.tar.gz`
///   its `.sha256` sidecar, plus `repos/{repo}/releases[.json]` + `releases/latest`.
fn stage_whisper(serve_root: &Path, platform: &str, arch: &str, stub: &Path) -> String {
    let archive_name = format!("whisper-server-{platform}-{arch}-cpu.tar.gz");
    let dl_dir = serve_root
        .join(WHISPER_REPO)
        .join("releases")
        .join("download")
        .join(TEST_VERSION);
    std::fs::create_dir_all(&dl_dir).expect("create download dir");

    // tar.gz with a single entry named `whisper-server` (the extractor matches
    // on the file name), holding the stub-whisper-server bytes.
    let archive_path = dl_dir.join(&archive_name);
    build_tar_gz(stub, "whisper-server", &archive_path);
    let archive_sha256 = sha256_file(&archive_path);

    // MANDATORY sha256 sidecar (`<64hex>  <filename>` format).
    std::fs::write(
        dl_dir.join(format!("{archive_name}.sha256")),
        format!("{archive_sha256}  {archive_name}\n"),
    )
    .expect("write .sha256 sidecar");

    // API: releases/latest JSON (the `version = "latest"` resolution path).
    let latest_dir = serve_root.join("repos").join(WHISPER_REPO).join("releases");
    std::fs::create_dir_all(&latest_dir).expect("create latest dir");
    std::fs::write(
        latest_dir.join("latest"),
        format!(r#"{{"tag_name":"{TEST_VERSION}"}}"#),
    )
    .expect("write latest json");

    // API: `GET /repos/{repo}/releases` — a sibling `releases.json` (the on-disk
    // `releases` is a directory holding `latest`, so `serve` falls back to the
    // sidecar). One installed-able entry: TEST_VERSION with the cpu asset + sig.
    let archive_size = std::fs::metadata(&archive_path).expect("stat archive").len();
    let sha_size = std::fs::metadata(dl_dir.join(format!("{archive_name}.sha256")))
        .expect("stat sha")
        .len();
    let releases_json = format!(
        r#"[
            {{"tag_name":"{TEST_VERSION}","draft":false,"prerelease":false,"published_at":"2026-05-01T00:00:00Z","assets":[{{"name":"{archive_name}","size":{archive_size}}},{{"name":"{archive_name}.sha256","size":{sha_size}}}]}}
        ]"#
    );
    std::fs::write(
        serve_root.join("repos").join(WHISPER_REPO).join("releases.json"),
        releases_json,
    )
    .expect("write releases.json");

    archive_sha256
}

fn build_tar_gz(source_bin: &Path, entry_name: &str, dest: &Path) {
    let tar_gz = std::fs::File::create(dest).expect("create archive");
    let enc = GzEncoder::new(tar_gz, Compression::fast());
    let mut builder = tar::Builder::new(enc);

    let mut f = std::fs::File::open(source_bin).expect("open stub binary");
    let len = f.metadata().expect("stub metadata").len();

    let mut header = tar::Header::new_gnu();
    header.set_size(len);
    header.set_mode(0o755);
    header.set_cksum();
    builder
        .append_data(&mut header, entry_name, &mut f)
        .expect("append stub to archive");
    let enc = builder.into_inner().expect("finish tar");
    enc.finish().expect("finish gzip");
}

fn sha256_file(path: &Path) -> String {
    use std::io::Read;
    let mut f = std::fs::File::open(path).expect("open archive for sha256");
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).expect("read archive");
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    format!("{:x}", hasher.finalize())
}

// ── loopback file server (GET + HEAD; HEAD gets a correct Content-Length
//    because axum runs the handler and strips the body) ──────────────────

#[derive(Clone)]
struct AppState {
    serve_root: PathBuf,
}

async fn serve(
    State(state): State<AppState>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Path-traversal guard.
    for seg in path.split('/') {
        if seg == ".." || seg.starts_with('/') {
            return Err((StatusCode::FORBIDDEN, "invalid path".to_string()));
        }
    }
    let target = state.serve_root.join(&path);
    let bytes = match std::fs::read(&target) {
        Ok(b) => b,
        Err(_) => {
            // `repos/{repo}/releases` is a directory on disk; the list request
            // falls back to a sibling `releases.json`.
            let sidecar = state.serve_root.join(format!("{path}.json"));
            std::fs::read(&sidecar)
                .map_err(|e| (StatusCode::NOT_FOUND, format!("not found ({path}): {e}")))?
        }
    };
    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
        bytes,
    ))
}
