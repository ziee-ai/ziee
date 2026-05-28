//! Tier 4 — Linux-only, rootfs-gated. End-to-end MCP-in-sandbox stdio
//! round-trip from `StdioMcpClient::connect()` through `bwrap` and back.
//!
//! Uses an inline `python3 -c '<MCP echo>'` fixture so no external
//! script needs to live in the rootfs. The fixture handles `initialize`
//! + `tools/list` + a single `tools/call` named `echo`.
//!
//! Mirrors the Tier-4 pattern from `tier4_sandbox_smoke.rs`: gate on
//! `ZIEE_SANDBOX_ROOTFS` (or the standard rootfs cache paths), skip
//! gracefully when unavailable.
//!
//! Runs are `#[ignore]`'d so `cargo test` doesn't try them by default.
//! Run via `just check-sandbox` or:
//!
//! ```bash
//! ZIEE_SANDBOX_ROOTFS=.ziee-cache/sandbox-rootfs/current \
//!     cargo test --test integration_tests -- --test-threads=1 \
//!     --ignored code_sandbox::tier4_mcp_stdio_linux
//! ```

#![cfg(target_os = "linux")]

use uuid::Uuid;

use ziee::modules::mcp::client::stdio::StdioMcpClient;
use ziee::modules::mcp::client::traits::McpClient;
use ziee::modules::mcp::models::{McpServer, TransportType, UsageMode};

use super::harness;

/// A tiny MCP "server" implemented as one python script string. Reads
/// JSON-RPC lines on stdin, responds on stdout. Speaks only the slice
/// of MCP the test exercises (initialize, tools/list, tools/call/echo,
/// notifications/initialized).
const MCP_ECHO_PY: &str = r#"
import json, sys
def respond(req_id, result):
    sys.stdout.write(json.dumps({"jsonrpc":"2.0","id":req_id,"result":result})+"\n")
    sys.stdout.flush()
for raw in sys.stdin:
    raw = raw.strip()
    if not raw: continue
    req = json.loads(raw)
    method = req.get("method")
    rid = req.get("id")
    if method == "initialize":
        respond(rid, {"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"tier4-echo","version":"0"}})
    elif method == "notifications/initialized":
        pass
    elif method == "tools/list":
        respond(rid, {"tools":[{"name":"echo","description":"echo back its arg","inputSchema":{"type":"object","properties":{"msg":{"type":"string"}},"required":["msg"]}}]})
    elif method == "tools/call":
        params = req.get("params", {})
        if params.get("name") == "echo":
            msg = params.get("arguments", {}).get("msg", "")
            respond(rid, {"content":[{"type":"text","text": msg}], "isError": False})
        else:
            respond(rid, {"content":[{"type":"text","text":"unknown tool"}], "isError": True})
    else:
        respond(rid, {})
"#;

fn echo_server_config() -> McpServer {
    McpServer {
        id: Uuid::new_v4(),
        user_id: None,
        name: "echo-tier4".into(),
        display_name: "Echo (Tier 4)".into(),
        description: None,
        enabled: true,
        is_system: true,
        is_built_in: false,
        transport_type: TransportType::Stdio,
        command: Some("python3".into()),
        args: serde_json::json!(["-c", MCP_ECHO_PY]),
        environment_variables: serde_json::json!({}),
        url: None,
        headers: serde_json::json!({}),
        timeout_seconds: 30,
        supports_sampling: false,
        usage_mode: UsageMode::Auto,
        max_concurrent_sessions: None,
        run_in_sandbox: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

/// Boot a TestServer with sandbox enabled — populates the
/// `code_sandbox::config::get_state()` global so `should_sandbox()`
/// returns true.
async fn boot_or_skip() -> bool {
    if harness::enabled_test_server().await.is_some() {
        true
    } else {
        eprintln!("tier4_mcp test skipped: sandbox not bootable on this host");
        false
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn mcp_stdio_initialize_and_list_tools_inside_bwrap() {
    if !boot_or_skip().await { return; }

    let mut client = StdioMcpClient::new(echo_server_config())
        .expect("client construction");
    client.connect().await.expect("MCP connect inside bwrap");

    let tools = client.list_tools().await.expect("list_tools");
    assert!(
        tools.iter().any(|t| t.name == "echo"),
        "expected echo tool, got {:?}",
        tools.iter().map(|t| &t.name).collect::<Vec<_>>()
    );

    let res = client
        .call_tool(
            "echo",
            serde_json::json!({"msg": "from-sandbox"}),
            None,
            None,
            None,
        )
        .await
        .expect("tools/call echo");
    let blob = serde_json::to_string(&res.content).unwrap_or_default();
    assert!(
        blob.contains("from-sandbox"),
        "echoed payload missing in response: {blob}"
    );

    client.disconnect().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn mcp_stdio_disconnect_kills_bwrap() {
    if !boot_or_skip().await { return; }

    fn bwrap_count() -> usize {
        std::process::Command::new("pgrep")
            .arg("-c")
            .arg("bwrap")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0)
    }
    let baseline = bwrap_count();

    let mut client = StdioMcpClient::new(echo_server_config()).expect("client");
    client.connect().await.expect("connect");
    // Confirm a fresh bwrap appeared.
    let with_client = bwrap_count();
    assert!(
        with_client > baseline,
        "no new bwrap process appeared (baseline={baseline}, with_client={with_client})"
    );

    drop(client);
    // bwrap teardown is async — poll briefly until we're back at baseline.
    for _ in 0..100 {
        if bwrap_count() <= baseline { return; }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!(
        "bwrap did not drain after client drop (baseline={baseline}, still={})",
        bwrap_count()
    );
}
