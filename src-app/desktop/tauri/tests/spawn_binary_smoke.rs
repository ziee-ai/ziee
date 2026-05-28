//! Spawn-binary smoke test (Layer 2).
//!
//! Boots the production-built `ziee` binary in a subprocess against
//! an isolated tempdir, waits for the embedded backend to bind a
//! port, hits `/api/health`, then SIGTERMs and waits for clean exit.
//!
//! Heavy: real embedded Postgres bringup (~10-15s on a warm box,
//! 60-90s cold). `#[ignore]` so it stays out of the default `cargo
//! test` pass; run via `just check-desktop-spawn` (or directly with
//! `cargo test --test spawn_binary_smoke -- --ignored`).
//!
//! Preconditions:
//!   - `target/release/ziee` exists. Build with `cargo build
//!     --release --bin ziee-desktop` (the bin target is named
//!     `ziee-desktop` per Cargo.toml; the artifact filename matches
//!     `mainBinaryName: "ziee"` in `tauri.conf.json`).
//!   - macOS: the test launches a Tauri window briefly — needs a
//!     real display session. Skip in headless CI.
//!
//! Test deliberately probes only the backend HTTP layer (the
//! production code path everyone exercises). Window / Tauri-IPC are
//! covered by Layer 3 (WebDriver E2E).

use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Wall-clock budget for backend bringup (embedded PG + migrations +
/// admin bootstrap). Cold start can be slow; keep generous.
const BRINGUP_TIMEOUT: Duration = Duration::from_secs(120);

/// Where the production-built binary lives.
fn binary_path() -> PathBuf {
    // Walk up from this test file: tests/ -> tauri/ -> desktop/ -> src-app/
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src_app = manifest.parent().unwrap().parent().unwrap();
    src_app.join("target/release/ziee")
}

/// Find a free TCP port by binding ephemerally and immediately
/// dropping. There's a tiny race window before the spawned process
/// re-binds, but the 8080-8180 range Tauri prod-picks is unlikely
/// to collide on a dev box.
fn pick_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    listener.local_addr().expect("local_addr").port()
}

/// Render a minimal config.yaml pointing at an isolated data_dir so
/// the spawned binary doesn't share the user's real embedded PG
/// instance.
fn write_isolated_config(data_dir: &std::path::Path, port: u16) -> PathBuf {
    let yaml = format!(
        r#"app:
  data_dir: '{data}'
server:
  host: '127.0.0.1'
  port: {port}
  cors: null
postgresql:
  embedded:
    enabled: true
auth:
  jwt_secret: '{secret}'
"#,
        data = data_dir.display(),
        port = port,
        secret = "test-only-jwt-secret-not-for-production-use-32+bytes",
    );
    let path = data_dir.join("config.yaml");
    std::fs::write(&path, yaml).expect("write config.yaml");
    path
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "heavy: spawns the production binary + embedded PG; run via `just check-desktop-spawn`"]
async fn binary_starts_backend_and_serves_health() {
    let bin = binary_path();
    assert!(
        bin.exists(),
        "binary not built at {bin:?} — run `cargo build --release --bin ziee-desktop` first",
    );

    let data_dir = tempfile::tempdir().expect("tempdir");
    let port = pick_free_port();
    let config_path = write_isolated_config(data_dir.path(), port);

    eprintln!("[spawn-smoke] data_dir = {:?}", data_dir.path());
    eprintln!("[spawn-smoke] port     = {port}");

    let mut child = Command::new(&bin)
        .arg("--config-file")
        .arg(&config_path)
        // Quiet Tauri's own stdout chatter; we want stderr for log lines.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // No CONFIG_FILE inheritance — the CLI flag wins.
        .env_remove("CONFIG_FILE")
        // Constrain log noise so the line we grep for is reliably present.
        .env("RUST_LOG", "info")
        .spawn()
        .expect("spawn ziee binary");

    // Drain stderr on a thread; signal when the bringup marker
    // appears. Hold onto the joinhandle so it can keep draining
    // until child exit (otherwise pipe fills and child blocks).
    let (tx, rx) = mpsc::channel::<()>();
    let stderr = child.stderr.take().expect("stderr piped");
    let drain = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        let mut signaled = false;
        for line in reader.lines().map_while(Result::ok) {
            eprintln!("[ziee] {line}");
            if !signaled && line.contains("Backend server started successfully") {
                let _ = tx.send(());
                signaled = true;
            }
        }
    });

    // Wait for the bringup marker on a blocking helper thread so we
    // don't park the tokio runtime.
    let started =
        tokio::task::spawn_blocking(move || rx.recv_timeout(BRINGUP_TIMEOUT).is_ok())
            .await
            .unwrap_or(false);

    let outcome = if started {
        // Hit /api/health. Backend's CORS is permissive in desktop
        // mode (set by mod.rs::init -> cors = None), so plain GET.
        let url = format!("http://127.0.0.1:{port}/api/health");
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("reqwest client");
        match client.get(&url).send().await {
            Ok(resp) => {
                eprintln!("[spawn-smoke] GET {url} -> {}", resp.status());
                resp.status().is_success()
            }
            Err(e) => {
                eprintln!("[spawn-smoke] health probe error: {e}");
                false
            }
        }
    } else {
        eprintln!("[spawn-smoke] timed out waiting for backend bringup");
        false
    };

    // SIGTERM the child. Tauri's `cleanup_server` runs on window-close,
    // not SIGTERM, so embedded-PG state may not unwind cleanly — but
    // the tempdir is dropped either way. On macOS/Linux `kill()` sends
    // SIGKILL; close enough for a smoke test.
    let _ = child.kill();
    let exit = child.wait().expect("wait child");
    drop(drain.join());

    assert!(
        outcome,
        "backend did not come up healthy within {:?} (child exited: {exit:?})",
        BRINGUP_TIMEOUT,
    );
}
