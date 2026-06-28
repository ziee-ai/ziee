//! Spawn the `@modelcontextprotocol/server-everything` reference server as
//! a child process for use by transport conformance tests.
//!
//! The "everything" server is the canonical MCP reference implementation
//! that demonstrates every protocol feature: tools, prompts, resources,
//! sampling, elicitation, and notifications. Testing against it proves our
//! client honours the spec because the reference server rejects
//! non-conforming clients (e.g., it requires `notifications/initialized`
//! before serving `tools/list`).
//!
//! Usage:
//! ```
//! let server = EverythingServer::start().await?;
//! // server.base_url() → "http://127.0.0.1:<port>/mcp"
//! // server is dropped → child process is killed
//! ```

use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};

/// Handle to a running `npx @modelcontextprotocol/server-everything` process.
/// Killed on drop.
pub struct EverythingServer {
    #[allow(dead_code)]
    child: Child,
    port: u16,
}

impl EverythingServer {
    /// Spawn the server on a free port. Returns once the server is bound and
    /// answering HTTP requests. Times out after 60s (initial `npx -y` invocation
    /// can pull packages on first run).
    ///
    /// Returns `Err` if `npx` is not on PATH, the npm registry is unreachable,
    /// or the server fails to bind. Tests that depend on this fixture should
    /// `if let Err(_) = EverythingServer::start() { eprintln!("skipping…"); return; }`
    /// — see `try_start_or_skip()`.
    pub async fn start() -> Result<Self, String> {
        let port = portpicker::pick_unused_port()
            .ok_or_else(|| "no free port available".to_string())?;

        // `streamableHttp` is the spec-current transport. The everything server
        // reads PORT from the env (other args go after `streamableHttp`).
        let mut child = Command::new("npx")
            .args(["-y", "@modelcontextprotocol/server-everything", "streamableHttp"])
            .env("PORT", port.to_string())
            // Capture stdout/stderr so we can drain them — otherwise the child
            // can block on a full pipe.
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("failed to spawn `npx`: {} (is node installed?)", e))?;

        // Drain stdout/stderr in background tasks so the child doesn't block.
        // Server logs are prefixed for grepping when a test fails.
        if let Some(stdout) = child.stdout.take() {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    eprintln!("[everything-server stdout] {}", line);
                }
            });
        }
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    eprintln!("[everything-server stderr] {}", line);
                }
            });
        }

        // Poll the server until it accepts connections, or time out.
        // First boot of `npx -y` can take 30+ seconds (downloads + extracts).
        let base_url = format!("http://127.0.0.1:{}/mcp", port);
        let probe = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();

        let ready = timeout(Duration::from_secs(60), async {
            loop {
                // POST an MCP initialize request — server returns 200 (or 400)
                // when the HTTP server is up. Either status counts as "bound".
                let resp = probe
                    .post(&base_url)
                    .header("Accept", "application/json, text/event-stream")
                    .json(&serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": 0,
                        "method": "initialize",
                        "params": {
                            "protocolVersion": "2025-11-25",
                            "capabilities": {},
                            "clientInfo": { "name": "probe", "version": "0.0.0" },
                        },
                    }))
                    .send()
                    .await;
                if resp.is_ok() {
                    return;
                }
                sleep(Duration::from_millis(500)).await;
            }
        })
        .await;

        if ready.is_err() {
            let _ = child.kill().await;
            return Err(format!("server-everything failed to bind on port {} within 60s", port));
        }

        eprintln!("[everything-server] ready at {}", base_url);
        Ok(Self { child, port })
    }

    /// Best-effort start. Returns `None` (with a printed reason) when the
    /// fixture can't run — caller should `return;` from the test to skip.
    /// Used so CI without node can still pass.
    ///
    /// **Loud mode:** set `MCP_REQUIRE_EVERYTHING=1` to turn an inability to
    /// start the reference server into a hard panic instead of a silent skip.
    /// Because libtest captures a passing test's output, a skip would otherwise
    /// be invisible and the reference-server coverage could be quietly lost —
    /// set this in CI (or locally) to guarantee these tests actually ran.
    pub async fn try_start_or_skip(test_name: &str) -> Option<Self> {
        match Self::start().await {
            Ok(s) => Some(s),
            Err(e) => {
                if std::env::var("MCP_REQUIRE_EVERYTHING").as_deref() == Ok("1") {
                    panic!(
                        "[{}] MCP_REQUIRE_EVERYTHING=1 but could not start \
                         @modelcontextprotocol/server-everything: {}",
                        test_name, e
                    );
                }
                eprintln!(
                    "[{}] SKIPPED — could not start @modelcontextprotocol/server-everything: {} \
                     (set MCP_REQUIRE_EVERYTHING=1 to fail instead of skip)",
                    test_name, e
                );
                None
            }
        }
    }

    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}/mcp", self.port)
    }

    #[allow(dead_code)]
    pub fn port(&self) -> u16 {
        self.port
    }
}

// Drop relies on `kill_on_drop(true)` set when spawning the child — Tokio
// will SIGKILL the process when the `Child` is dropped. No explicit impl
// needed.
