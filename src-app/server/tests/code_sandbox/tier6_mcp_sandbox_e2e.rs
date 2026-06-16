//! Tier 6 — FULL HTTP-driven E2E for the MCP-in-sandbox feature.
//!
//! Exercises the production code path end-to-end:
//!
//!     HTTP create system MCP server (run_in_sandbox=true)
//!       → mcp_servers DB row persisted
//!       → admin GET /mcp/servers/{id}/tools
//!         → handler resolves McpSessionManager (server process)
//!         → manager lazy-connects StdioMcpClient
//!         → should_sandbox() = true
//!         → mcp_spawn::start_mcp_in_sandbox
//!         → Linux: bwrap on host  |  macOS: libkrun → in-guest agent → bwrap
//!         → real python3 MCP child speaks JSON-RPC
//!         → list_tools result → JSON envelope back over HTTP
//!       → admin POST /mcp/servers/{id}/tools/echo/call
//!         → same session, same sandboxed child, real tools/call
//!         → real "echo" response round-trips
//!       → admin DELETE /mcp/servers/{id}/disconnect
//!         → manager drops the session → KillProcess / kill_on_drop
//!
//! Rootfs-gated. `#[ignore]`'d by default. On macOS uses the libkrun
//! path via `harness::enabled_test_server()` which stages the
//! `test-minimal.squashfs` test rootfs. On Linux uses host bwrap.
//!
//! ```bash
//! cargo test --test integration_tests -- --test-threads=1 \
//!     --ignored code_sandbox::tier6_mcp_sandbox_e2e
//! ```

use serde_json::json;

use super::harness;
use crate::common::test_helpers;

/// Inline python3 MCP "server" — speaks the slice of MCP that the
/// test exercises (initialize, notifications/initialized, tools/list,
/// tools/call/echo, tools/call/env_dump). python3 ships in the
/// 'minimal' sandbox rootfs on every platform.
const MCP_ECHO_PY: &str = r#"
import json, os, sys
def respond(rid, result):
    sys.stdout.write(json.dumps({"jsonrpc":"2.0","id":rid,"result":result})+"\n")
    sys.stdout.flush()
for raw in sys.stdin:
    raw = raw.strip()
    if not raw: continue
    req = json.loads(raw)
    method = req.get("method")
    rid = req.get("id")
    if method == "initialize":
        respond(rid, {"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"tier6-echo","version":"0"}})
    elif method == "notifications/initialized":
        pass
    elif method == "tools/list":
        respond(rid, {"tools":[
            {"name":"echo","description":"echo arg back","inputSchema":{"type":"object","properties":{"msg":{"type":"string"}},"required":["msg"]}},
            {"name":"env_dump","description":"dump os.environ","inputSchema":{"type":"object","properties":{}}}
        ]})
    elif method == "tools/call":
        params = req.get("params", {})
        name = params.get("name")
        if name == "echo":
            msg = params.get("arguments", {}).get("msg", "")
            respond(rid, {"content":[{"type":"text","text": msg}], "isError": False})
        elif name == "env_dump":
            text = "\n".join(f"{k}={v}" for k,v in sorted(os.environ.items()))
            respond(rid, {"content":[{"type":"text","text": text}], "isError": False})
        else:
            respond(rid, {"content":[{"type":"text","text":"unknown tool"}], "isError": True})
    else:
        respond(rid, {})
"#;

/// Create a system stdio MCP server pointed at the inline MCP_ECHO_PY.
/// `run_in_sandbox` is controlled by the caller.
async fn create_sandboxed_echo_server(
    server: &crate::common::TestServer,
    admin_token: &str,
    run_in_sandbox: bool,
    extra_env: serde_json::Value,
) -> String {
    // The create API takes env vars as the structured `environment_variables_entries`
    // ([{key,value,is_secret}]) — NOT the internal write-only `environment_variables`
    // object (skip_serializing on the model). Convert the caller's {k:v} map.
    let env_entries: Vec<serde_json::Value> = extra_env
        .as_object()
        .map(|o| {
            o.iter()
                .map(|(k, v)| json!({ "key": k, "value": v.as_str().unwrap_or(""), "is_secret": false }))
                .collect()
        })
        .unwrap_or_default();
    let url = server.api_url("/mcp/system-servers");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&json!({
            "name": format!("tier6-echo-{}", uuid::Uuid::new_v4()),
            "display_name": "Tier 6 Echo",
            "enabled": true,
            "transport_type": "stdio",
            "command": "python3",
            "args": ["-c", MCP_ECHO_PY],
            "environment_variables_entries": env_entries,
            "timeout_seconds": 60,
            "run_in_sandbox": run_in_sandbox,
        }))
        .send()
        .await
        .expect("create system server");
    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("json");
    assert_eq!(
        status, 201,
        "create system server (run_in_sandbox={run_in_sandbox}) failed: {body}"
    );
    body["id"].as_str().expect("server id").to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn sandboxed_mcp_lists_tools_and_calls_echo_through_http_api() {
    // Boot a real TestServer with sandbox enabled. On Mac this stages
    // test-minimal.squashfs into a TempDir cache and boots a libkrun VM.
    let Some(server) = harness::enabled_test_server().await else {
        eprintln!("tier6 skipped: sandbox not bootable on this host");
        return;
    };

    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "mcp_servers_admin::create",
            "mcp_servers_admin::edit",
            "mcp_servers_admin::read",
            "mcp_servers::read",
        ],
    )
    .await;

    let server_id =
        create_sandboxed_echo_server(&server, &admin.token, true, json!({})).await;

    // GET tools → forces lazy connect through the sandboxed spawn
    // path on the server side. This is THE end-to-end test of the
    // feature: every layer (handler → manager → StdioMcpClient →
    // should_sandbox → mcp_spawn::start_mcp_in_sandbox → libkrun/bwrap
    // → python MCP child → JSON-RPC initialize+list_tools) fires.
    let tools_url = server.api_url(&format!("/mcp/servers/{server_id}/tools"));
    let tools_resp = reqwest::Client::new()
        .get(&tools_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("list tools http");
    let tools_status = tools_resp.status();
    let tools_body: serde_json::Value = tools_resp.json().await.expect("tools json");
    assert_eq!(
        tools_status, 200,
        "list tools failed (this is where sandboxed connect runs end-to-end): {tools_body}"
    );
    let tools = tools_body["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(
        names.contains(&"echo"),
        "sandboxed MCP did not surface 'echo' tool. names={names:?}"
    );

    // POST tools/echo/call — round-trip a real payload through the
    // sandboxed child.
    let call_url =
        server.api_url(&format!("/mcp/servers/{server_id}/tools/echo/call"));
    let call_resp = reqwest::Client::new()
        .post(&call_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "arguments": { "msg": "tier6-from-host" } }))
        .send()
        .await
        .expect("call tool http");
    let call_status = call_resp.status();
    let call_body: serde_json::Value = call_resp.json().await.expect("call json");
    assert_eq!(
        call_status, 200,
        "tools/call returned non-200: {call_body}"
    );
    assert_eq!(call_body["is_error"], false, "tool reported is_error: {call_body}");
    let serialized = serde_json::to_string(&call_body["content"]).unwrap_or_default();
    assert!(
        serialized.contains("tier6-from-host"),
        "echo payload did not round-trip through the sandbox. body={call_body}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn sandboxed_mcp_env_isolates_host_secrets() {
    // Set a host-side env var BEFORE the TestServer boots. The
    // sandboxed MCP child must NOT see it — bwrap's --clearenv +
    // --setenv only the whitelisted entries from configuration is
    // the contract being verified.
    //
    // SAFETY: tests are single-threaded by definition (`--test-threads=1`
    // in the suite recipe). set_var becoming unsafe in 2024 edition
    // is about parallel-thread races; --test-threads=1 + setting before
    // TestServer boot makes this race-free in this context.
    unsafe {
        std::env::set_var("ZIEE_TIER6_HOST_SECRET", "must-not-leak");
    }

    let Some(server) = harness::enabled_test_server().await else {
        eprintln!("tier6 env-isolation skipped: sandbox not bootable on this host");
        return;
    };

    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "mcp_servers_admin::create",
            "mcp_servers_admin::read",
            "mcp_servers::read",
        ],
    )
    .await;

    let server_id = create_sandboxed_echo_server(
        &server,
        &admin.token,
        true,
        json!({ "MY_CONFIG_VAR": "yes-this-is-set" }),
    )
    .await;

    let call_url =
        server.api_url(&format!("/mcp/servers/{server_id}/tools/env_dump/call"));
    let call_resp = reqwest::Client::new()
        .post(&call_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "arguments": {} }))
        .send()
        .await
        .expect("env_dump http");
    let body: serde_json::Value = call_resp.json().await.expect("json");
    let env_text = serde_json::to_string(&body["content"]).unwrap_or_default();

    assert!(
        !env_text.contains("ZIEE_TIER6_HOST_SECRET"),
        "host-process env leaked into the sandboxed MCP child. \
         The bwrap --clearenv contract is broken. body excerpt: {}",
        &env_text.chars().take(500).collect::<String>()
    );
    assert!(
        env_text.contains("MY_CONFIG_VAR=yes-this-is-set"),
        "configured env var was NOT forwarded into the sandbox. body excerpt: {}",
        &env_text.chars().take(500).collect::<String>()
    );
    assert!(
        env_text.contains("HOME=/home/sandboxuser"),
        "synthetic HOME not present — bwrap --setenv block broken? body excerpt: {}",
        &env_text.chars().take(500).collect::<String>()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn sandboxed_mcp_disconnect_then_reconnect_works() {
    // Spawn → call → disconnect → re-spawn the same server. The
    // second connect must succeed: the manager must have released the
    // sandboxed session cleanly (no stuck handle / wedged VM session).
    let Some(server) = harness::enabled_test_server().await else {
        eprintln!("tier6 reconnect skipped: sandbox not bootable on this host");
        return;
    };

    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "mcp_servers_admin::create",
            "mcp_servers_admin::read",
            "mcp_servers::read",
        ],
    )
    .await;

    let server_id =
        create_sandboxed_echo_server(&server, &admin.token, true, json!({})).await;

    let call = |msg: &'static str| {
        let server_url = server
            .api_url(&format!("/mcp/servers/{server_id}/tools/echo/call"));
        let token = admin.token.clone();
        async move {
            reqwest::Client::new()
                .post(&server_url)
                .header("Authorization", format!("Bearer {token}"))
                .json(&json!({ "arguments": { "msg": msg } }))
                .send()
                .await
                .unwrap()
                .json::<serde_json::Value>()
                .await
                .unwrap()
        }
    };

    let r1 = call("first-lifecycle").await;
    assert!(serde_json::to_string(&r1["content"]).unwrap().contains("first-lifecycle"));

    // Force disconnect via the runtime endpoint.
    let dc_url = server.api_url(&format!("/mcp/servers/{server_id}/disconnect"));
    let dc_resp = reqwest::Client::new()
        .delete(&dc_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("disconnect http");
    assert!(
        dc_resp.status().is_success(),
        "disconnect returned {}",
        dc_resp.status()
    );

    // Brief settle so the prior sandboxed child fully tears down
    // (kill_on_drop → bwrap exit → sandboxed-child reaped → manager
    // drops session). 500ms is generous; in practice tens of ms.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Second lifecycle on the same server — re-connects from scratch.
    let r2 = call("second-lifecycle").await;
    assert!(
        serde_json::to_string(&r2["content"]).unwrap().contains("second-lifecycle"),
        "second call after disconnect did NOT succeed. response: {r2}"
    );
}
