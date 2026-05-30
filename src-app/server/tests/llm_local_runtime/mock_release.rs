//! Mock release server for engine-binary downloads.
//!
//! Test-side equivalent of a GitHub Releases page for the `ziee-ai/*`
//! engine forks. It packages the `stub-engine` test binary as the real
//! release artifact (`{server}-{platform}-{arch}-cpu.tar.gz`, with the
//! binary entry named `llama-server` / `mistralrs-server`), serves it +
//! a dummy `.sig` + a `releases/latest` JSON from a loopback axum server,
//! and starts a `TestServer` with the two debug-only mirror env vars
//! (`LLM_RUNTIME_RELEASE_MIRROR`, `LLM_RUNTIME_API_MIRROR`) so
//! `binary_download` downloads from THIS process instead of github.com.
//!
//! This lets the integration suite exercise the FULL engine-download
//! pipeline — resolve → download → extract → cache → register — without
//! a published fork release, and then spawn the cached stub for the
//! lifecycle / proxy tests.
//!
//! Mirrors `crate::code_sandbox::mirror_fixture` (the rootfs equivalent).

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

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

use crate::common::{TestServer, TestServerOptions};

/// Version tag every fixture artifact is published under.
pub const TEST_VERSION: &str = "v0.0.0-test";

/// A synthetic release tag that exists upstream but ships NO binary asset —
/// exercises the "release created, build pending" path (`binary_ready=false`).
pub const PENDING_VERSION: &str = "v9.9.9-pending";

/// A running mock release server + a TestServer wired to fetch from it.
pub struct MockReleaseServer {
    pub server: TestServer,
    pub mirror_url: String,
    pub version: String,
    pub platform: String,
    pub arch: String,
    /// sha256 of the staged llamacpp archive (informational; the runtime
    /// download path does not itself verify sha256).
    pub llamacpp_sha256: String,
    _serve_dir: TempDir,
    _server_handle: JoinHandle<()>,
}

impl Drop for MockReleaseServer {
    fn drop(&mut self) {
        self._server_handle.abort();
    }
}

/// Stand up the mock release server (staging both engine flavors) and a
/// TestServer pointed at it. Builds the stub-engine on demand if it is
/// not already in the workspace target dir.
pub async fn setup() -> MockReleaseServer {
    setup_with_env(Vec::new()).await
}

/// Like [`setup`] but injects additional env vars into the spawned
/// server (on top of the mirror vars) — e.g. `LLM_RUNTIME_REAPER_TICK_MS`
/// for the idle-eviction / drain tests.
pub async fn setup_with_env(mut extra_env: Vec<(String, String)>) -> MockReleaseServer {
    let stub = stub_engine_binary();
    let platform = host_platform();
    let arch = host_arch();

    let serve_dir = TempDir::new().expect("serve TempDir");
    let serve_root = serve_dir.path().to_path_buf();

    // Stage both engine archives so tests for either engine resolve.
    let llamacpp_sha256 = stage_engine(
        &serve_root,
        "ziee-ai/llama.cpp",
        "llama-server",
        &platform,
        &arch,
        &stub,
    );
    stage_engine(
        &serve_root,
        "ziee-ai/mistral.rs",
        "mistralrs-server",
        &platform,
        &arch,
        &stub,
    );

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

    // Start a TestServer that resolves engine downloads against the mock.
    let mut env = vec![
        ("LLM_RUNTIME_RELEASE_MIRROR".to_string(), mirror_url.clone()),
        ("LLM_RUNTIME_API_MIRROR".to_string(), mirror_url.clone()),
    ];
    env.append(&mut extra_env);
    let opts = TestServerOptions {
        extra_env: env,
        ..Default::default()
    };
    let server = TestServer::start_with_options(opts).await;

    MockReleaseServer {
        server,
        mirror_url,
        version: TEST_VERSION.to_string(),
        platform,
        arch,
        llamacpp_sha256,
        _serve_dir: serve_dir,
        _server_handle: server_handle,
    }
}

/// Stage one engine's release artifacts under `serve_root`, returning the
/// archive's sha256. Lays out the exact paths `binary_download` requests:
///   `{repo}/releases/download/{TEST_VERSION}/{server}-{platform}-{arch}-cpu.tar.gz`
///   plus a sibling `.sig` and a `repos/{repo}/releases/latest` JSON.
fn stage_engine(
    serve_root: &Path,
    repo: &str,
    server_bin: &str,
    platform: &str,
    arch: &str,
    stub: &Path,
) -> String {
    let archive_name = format!("{server_bin}-{platform}-{arch}-cpu.tar.gz");
    let dl_dir = serve_root
        .join(repo)
        .join("releases")
        .join("download")
        .join(TEST_VERSION);
    std::fs::create_dir_all(&dl_dir).expect("create download dir");

    // tar.gz with a single entry named `server_bin` (what the extractor
    // matches on), holding the stub-engine binary bytes.
    let archive_path = dl_dir.join(&archive_name);
    build_tar_gz(stub, server_bin, &archive_path);

    // Dummy sibling signature (HEAD must 200 so the runtime's best-effort
    // .sig fetch succeeds; real cosign verify is policy-gated server-side).
    std::fs::write(dl_dir.join(format!("{archive_name}.sig")), b"dummy-sig")
        .expect("write .sig");

    // releases/latest JSON for the `version = "latest"` resolution path.
    let latest_dir = serve_root.join("repos").join(repo).join("releases");
    std::fs::create_dir_all(&latest_dir).expect("create latest dir");
    std::fs::write(
        latest_dir.join("latest"),
        format!(r#"{{"tag_name":"{TEST_VERSION}"}}"#),
    )
    .expect("write latest json");

    // `GET /repos/{repo}/releases` (the list `check_for_updates` reads).
    // `releases` is a directory on disk (holds `latest`), so the list lives
    // in a sibling `releases.json` that `serve` falls back to. Two entries:
    // TEST_VERSION (asset + sig present) and PENDING_VERSION (no asset, so
    // `binary_ready=false` for every host).
    let releases_json = format!(
        r#"[
            {{"tag_name":"{PENDING_VERSION}","draft":false,"prerelease":true,"published_at":"2026-05-29T00:00:00Z","assets":[]}},
            {{"tag_name":"{TEST_VERSION}","draft":false,"prerelease":false,"published_at":"2026-05-01T00:00:00Z","assets":[{{"name":"{archive_name}"}},{{"name":"{archive_name}.sig"}}]}}
        ]"#
    );
    std::fs::write(
        serve_root.join("repos").join(repo).join("releases.json"),
        releases_json,
    )
    .expect("write releases.json");

    sha256_file(&archive_path)
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

/// Locate (building on demand) the `stub-engine` binary in the workspace
/// target dir. Cached after the first resolution.
fn stub_engine_binary() -> PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let exe = if cfg!(windows) {
            "stub-engine.exe"
        } else {
            "stub-engine"
        };
        // CARGO_MANIFEST_DIR = <repo>/src-app/server; workspace target is
        // one level up at <repo>/src-app/target.
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest.parent().expect("src-app dir").to_path_buf();
        let candidate = workspace_root.join("target/debug").join(exe);

        if !candidate.exists() {
            eprintln!("stub-engine not built; running `cargo build -p stub-engine`…");
            let status = std::process::Command::new(env!("CARGO"))
                .args(["build", "-p", "stub-engine"])
                .current_dir(&workspace_root)
                .status()
                .expect("spawn cargo build -p stub-engine");
            assert!(status.success(), "cargo build -p stub-engine failed");
        }
        assert!(
            candidate.exists(),
            "stub-engine binary missing at {}",
            candidate.display()
        );
        candidate
    })
    .clone()
}

fn host_platform() -> String {
    if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        panic!("unsupported platform for mock release fixture")
    }
    .to_string()
}

fn host_arch() -> String {
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        panic!("unsupported arch for mock release fixture")
    }
    .to_string()
}

// ── loopback file server (GET + HEAD; HEAD gets correct Content-Length
//    because axum runs the handler and strips the body) ──────────────────

#[derive(Clone)]
struct AppState {
    serve_root: PathBuf,
}

async fn serve(
    State(state): State<AppState>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Path-traversal guard (no canonicalization — we serve real files, no
    // symlinks here, but reject `..` defensively).
    for seg in path.split('/') {
        if seg == ".." || seg.starts_with('/') {
            return Err((StatusCode::FORBIDDEN, "invalid path".to_string()));
        }
    }
    let target = state.serve_root.join(&path);
    // `repos/{repo}/releases` is a directory on disk (it holds `latest`), so
    // the release-list request falls back to a sibling `releases.json`.
    let bytes = match std::fs::read(&target) {
        Ok(b) => b,
        Err(_) => {
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
