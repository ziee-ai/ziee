//! Test fixture that stands up a local file-server "mirror" of a
//! single rootfs squashfs + a matching `known_revisions.dev.toml`,
//! then injects the two debug-only env vars
//! (`CODE_SANDBOX_KNOWN_REVISIONS_OVERRIDE` +
//! `CODE_SANDBOX_ROOTFS_MIRROR`) into a TestServer so the
//! `runtime_fetch` path downloads from THIS process instead of GitHub
//! Releases.
//!
//! This is the test-side equivalent of `scripts/dev-release.sh` —
//! same effective env shape, but the http server is an in-process
//! axum router so test cleanup is automatic via Drop + tokio
//! abort-on-drop semantics on the JoinHandle.
//!
//! Lets Tier 3 tests exercise the FULL auto-fetch path (download →
//! sha256-verify → atomic-install) without needing a published
//! GitHub release.
//!
//! Skips cleanly (returns None) when:
//!  - bwrap is missing (needed for TestServer's sandbox init), OR
//!  - no built squashfs is available in the dev cache.

#![allow(dead_code)]

use std::path::PathBuf;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use crate::code_sandbox::harness;
use crate::common::{TestServer, TestServerOptions};

/// Active fixture. Holds a server-side handle that aborts on drop so
/// nothing leaks if a test panics. Also keeps the `TempDir` alive.
pub struct MirrorFixture {
    pub server: TestServer,
    pub mirror_url: String,
    pub squashfs_sha256: String,
    pub flavor: String,
    /// kept alive so the cache_dir + dev toml outlive the test.
    _cache_dir: TempDir,
    _serve_dir: TempDir,
    _server_handle: JoinHandle<()>,
}

impl Drop for MirrorFixture {
    fn drop(&mut self) {
        self._server_handle.abort();
    }
}

/// Build a fixture for `flavor` (typically "minimal"). Returns None
/// when prerequisites are missing — caller should `return` to skip
/// cleanly.
///
/// SCHEMA + REVISION + ARCH are hardcoded to match the minimal
/// squashfs the dev environment builds (v1.r0-x86_64). The fixture
/// is intentionally narrow; broaden when a real test needs other
/// (schema, revision) tuples.
pub async fn setup(flavor: &str) -> Option<MirrorFixture> {
    if !harness::bwrap_available() {
        eprintln!("test skipped: bwrap not installed (needed for TestServer sandbox init)");
        return None;
    }

    // 1. Locate an existing .squashfs in the dev cache. (Don't BUILD
    // one here — that takes minutes and is the rootfs maintainer's
    // job; the test just needs SOMETHING with the right filename
    // shape to verify the wire path.)
    let source_sqfs = match find_dev_squashfs(flavor) {
        Some(p) => p,
        None => {
            eprintln!(
                "test skipped: no {flavor} .squashfs in .ziee-cache/sandbox-rootfs/. \
                 Run `just sandbox-build {flavor}` first."
            );
            return None;
        }
    };
    // Canonicalize to an absolute path — `find_dev_squashfs` may
    // return a `../../...`-relative path. The symlink we'll create
    // in the temp serve dir needs an absolute target or the http
    // server's `std::fs::read` will resolve it relative to the
    // symlink's location (which is the temp dir, NOT the test CWD)
    // and 404 on a non-existent file.
    let source_sqfs = source_sqfs
        .canonicalize()
        .expect("canonicalize source squashfs");

    let schema = 1u32;
    let revision = "r0";
    let arch = std::env::consts::ARCH;
    let asset_filename = format!(
        "ziee-sandbox-rootfs-v{schema}.{revision}-{arch}-{flavor}.squashfs"
    );
    let tag = format!("sandbox-rootfs-v{schema}.{revision}-{arch}");

    // 2. SHA256 the source (the fetch path verifies against this).
    let sha256 = sha256_file(&source_sqfs);

    // 3. Lay out the serve dir as the URL expects:
    //    <serve_root>/<tag>/<asset_filename> -> the real squashfs (symlink)
    let serve_dir = TempDir::new().ok()?;
    let tag_dir = serve_dir.path().join(&tag);
    std::fs::create_dir_all(&tag_dir).ok()?;
    // Cross-platform symlink: Windows distinguishes file vs dir links
    // via `symlink_file` / `symlink_dir`, and may require Developer Mode
    // or admin privilege. Fall back to `fs::copy` if the symlink call
    // fails — the fixture only needs the URL to serve the bytes, not
    // share inode state with the source.
    let dest = tag_dir.join(&asset_filename);
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&source_sqfs, &dest).ok()?;
    }
    #[cfg(windows)]
    {
        if std::os::windows::fs::symlink_file(&source_sqfs, &dest).is_err() {
            std::fs::copy(&source_sqfs, &dest).ok()?;
        }
    }

    // 4. Cache dir — where the prefetch path will INSTALL the
    // downloaded squashfs. Must be a fresh tempdir; if it had the
    // file already, the runtime_fetch idempotency path would short-
    // circuit with bytes_downloaded=0 (correct production behavior,
    // but defeats the purpose of THIS test).
    let cache_dir = TempDir::new().ok()?;

    // 5. Write the dev `known_revisions.dev.toml` pointing at the
    // sha256. `signed = false` skips cosign (no Sigstore identity
    // setup in tests).
    let dev_toml_path = cache_dir.path().join("known_revisions.dev.toml");
    let dev_toml = format!(
        r#"
[[revision]]
schema = {schema}
revision = "{revision}"
arch = "{arch}"
flavor = "{flavor}"
sha256 = "{sha256}"
signed = false
yanked = false
"#
    );
    std::fs::write(&dev_toml_path, dev_toml).ok()?;

    // 6. Spawn an axum http server on a random loopback port serving
    // the serve_dir. ServeDir would have been simpler but we want
    // explicit JoinHandle ownership; a 1-route impl is fine.
    let listener = TcpListener::bind("127.0.0.1:0").await.ok()?;
    let port = listener.local_addr().ok()?.port();
    let mirror_url = format!("http://127.0.0.1:{port}");

    let app_state = AppState {
        serve_root: serve_dir.path().to_path_buf(),
    };
    let app = axum::Router::new()
        .route("/{*path}", get(serve))
        .with_state(app_state);
    let server_handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });

    // 7. Start a TestServer with sandbox enabled + rootfs_path
    // pointed at our cache_dir (so prefetch's derive_cache_dir
    // returns OUR dir) + the two debug-only env vars set.
    //
    // The rootfs_path's PARENT is the cache_dir; we use
    // `<cache>/current` as the canonical "mounted rootfs" location
    // — that path doesn't actually need to exist for the prefetch
    // path; only for `execute_command`'s lazy mount, which we don't
    // exercise here.
    let rootfs_path = cache_dir.path().join("current");
    let opts = TestServerOptions {
        sandbox_enabled: true,
        sandbox_rootfs: Some(rootfs_path),
        sandbox_cgroup_parent: String::new(),
        extra_env: vec![
            (
                "CODE_SANDBOX_KNOWN_REVISIONS_OVERRIDE".to_string(),
                dev_toml_path.to_string_lossy().into_owned(),
            ),
            (
                "CODE_SANDBOX_ROOTFS_MIRROR".to_string(),
                mirror_url.clone(),
            ),
        ],
        sandbox_cache_tempdir: None,
                use_desktop_binary: false,
    };
    let server = TestServer::start_with_options(opts).await;

    Some(MirrorFixture {
        server,
        mirror_url,
        squashfs_sha256: sha256,
        flavor: flavor.to_string(),
        _cache_dir: cache_dir,
        _serve_dir: serve_dir,
        _server_handle: server_handle,
    })
}

fn find_dev_squashfs(flavor: &str) -> Option<PathBuf> {
    let suffix = format!("-{flavor}.squashfs");
    for candidate in [
        ".ziee-cache/sandbox-rootfs",
        "../.ziee-cache/sandbox-rootfs",
        "../../.ziee-cache/sandbox-rootfs",
    ] {
        let dir = PathBuf::from(candidate);
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(&suffix))
            {
                return Some(p);
            }
        }
    }
    None
}

fn sha256_file(path: &std::path::Path) -> String {
    use std::io::Read;
    let mut f = std::fs::File::open(path).expect("open source squashfs");
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).expect("read");
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    format!("{:x}", h.finalize())
}

// ─────────────────────────────────────────────────────────────────────
// axum http handler — serves files from `serve_root` by path
// ─────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    serve_root: PathBuf,
}

async fn serve(
    State(state): State<AppState>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Lightweight path-traversal guard: reject any segment that
    // equals ".." or starts with "/". We deliberately do NOT
    // canonicalize — the serve_dir contains symlinks pointing at
    // squashfs files OUTSIDE the dir (so we don't have to copy 57 MB
    // per test), and canonicalize would reject those.
    for seg in path.split('/') {
        if seg == ".." || seg.starts_with('/') {
            return Err((StatusCode::FORBIDDEN, "invalid path".to_string()));
        }
    }
    let target = state.serve_root.join(&path);
    let bytes = std::fs::read(&target)
        .map_err(|e| (StatusCode::NOT_FOUND, format!("not found ({path}): {e}")))?;
    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
        bytes,
    ))
}
