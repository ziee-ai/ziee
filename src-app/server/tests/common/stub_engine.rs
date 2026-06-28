//! Test helper: spawn the workspace `stub-engine` binary as a standalone
//! OpenAI-compatible server, so the chat path can run deterministically without
//! a real LLM. A `custom` provider pointing at `http://127.0.0.1:<port>/v1`
//! routes generation here (see `chat::helpers::create_stub_model`).
//!
//! The stub emits the fixed reply `"Hello from stub"` as paced SSE deltas;
//! `chunk_delay_ms` slows them so a turn can be observed / cancelled mid-flight.

use std::path::PathBuf;
use std::process::Child;
use std::sync::OnceLock;
use std::time::Duration;

/// A spawned stub-engine process. Killed on drop.
pub struct StubEngine {
    child: Child,
    pub port: u16,
}

impl Drop for StubEngine {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl StubEngine {
    /// Spawn on a free port; resolves once `/health` returns 200.
    pub async fn start_with_chunk_delay(chunk_delay_ms: u64) -> StubEngine {
        let port = free_port();
        let mut cmd = std::process::Command::new(stub_engine_binary());
        cmd.args(["--port", &port.to_string()]);
        if chunk_delay_ms > 0 {
            cmd.args(["--chunk-delay-ms", &chunk_delay_ms.to_string()]);
        }
        let child = cmd.spawn().expect("spawn stub-engine");

        // Poll /health until ready (the bind + axum serve is near-instant).
        let client = reqwest::Client::new();
        let health = format!("http://127.0.0.1:{port}/health");
        let mut ready = false;
        for _ in 0..100 {
            if let Ok(r) = client.get(&health).send().await {
                if r.status().is_success() {
                    ready = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(ready, "stub-engine did not become healthy on port {port}");

        StubEngine { child, port }
    }

    /// The OpenAI-compatible base URL to hand a `custom` provider.
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}/v1", self.port)
    }
}

/// Grab an ephemeral port by binding :0 and immediately releasing it. There is
/// a small race window before stub-engine rebinds, acceptable for tests.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .unwrap()
        .port()
}

/// Locate (building on demand) the `stub-engine` binary in the workspace target
/// dir. Cached after the first resolution. Mirrors the locator in
/// `tests/llm_local_runtime/mock_release.rs`.
fn stub_engine_binary() -> PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let exe = if cfg!(windows) {
            "stub-engine.exe"
        } else {
            "stub-engine"
        };
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
