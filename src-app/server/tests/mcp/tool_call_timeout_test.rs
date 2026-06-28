//! MCP per-call timeout + retry-after-timeout (audit all-17d263a18ec8).
//!
//! Gap: no test proved that an MCP request which exceeds the server's
//! configured `timeout_seconds` is torn down with a timeout error, and — the
//! security/robustness-relevant half — that the SAME client recovers and
//! services a SUBSEQUENT call (a single slow upstream response must not wedge
//! the transport for the rest of the session).
//!
//! What's actually bounded by `timeout_seconds`: requests that go through the
//! regular (overall-timeout'd) reqwest client — `do_initialize`, `tools/list`,
//! `ping` — i.e. everything reached via `HttpMcpClient::request`. (A non-
//! sampling `tools/call` POST rides the *streaming* client, which has no
//! overall timeout; that path is instead bounded by `execute_tool`'s outer
//! `timeout_seconds + 300s` wrapper — 301s minimum, not deterministically
//! testable here. So the per-call transport timeout is exercised through
//! `list_tools`, and recovery is then proven to extend to a real `tools/call`.)
//!
//! Only the upstream server's response *timing* is mocked (a genuine slow
//! response); the client's timeout + recovery logic under test is real.

use super::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};
use std::time::Duration;
use ziee::{HttpMcpClient, McpClient, McpServer, TransportType, UsageMode};

/// Server config with a 1-second per-request timeout so a 2s mock response
/// reliably trips the overall timeout.
fn server_config(url: String) -> McpServer {
    McpServer {
        id: uuid::Uuid::new_v4(),
        user_id: None,
        name: "mock-mcp-timeout".to_string(),
        display_name: "Mock MCP (timeout fixture)".to_string(),
        description: None,
        enabled: true,
        is_system: false,
        transport_type: TransportType::Http,
        command: None,
        args: serde_json::json!([]),
        environment_variables: serde_json::json!({}),
        environment_variables_entries: vec![],
        url: Some(url),
        headers: serde_json::json!({}),
        headers_entries: vec![],
        timeout_seconds: 1,
        supports_sampling: false,
        usage_mode: UsageMode::Auto,
        max_concurrent_sessions: None,
        is_built_in: false,
        run_in_sandbox: false,
        sandbox_flavor: "full".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        last_health_check_at: None,
        last_health_check_status: "untested".to_string(),
        last_health_check_reason: None,
    }
}

#[tokio::test]
async fn request_times_out_then_transport_recovers_for_tool_call() {
    let mock = MockMcpServer::start().await;

    // Program the FIRST tools/list to hang 2s (> the 1s timeout), then a fast
    // tools/list, then a fast tools/call. Per-method queues pop front-first.
    mock.on_method(
        "tools/list",
        MockResponse::DelayedJsonOk {
            delay_ms: 2_000,
            value: serde_json::json!({ "tools": [] }),
        },
    );
    mock.on_method(
        "tools/list",
        MockResponse::JsonOk(serde_json::json!({
            "tools": [{
                "name": "echo",
                "description": "echo",
                "inputSchema": { "type": "object" }
            }]
        })),
    );
    mock.on_method(
        "tools/call",
        MockResponse::JsonOk(serde_json::json!({
            "content": [{ "type": "text", "text": "pong-after-recovery" }],
            "isError": false
        })),
    );

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    // connect() only does initialize (auto-answered fast by the mock).
    client.connect().await.expect("connect");

    // (1) First tools/list trips the 1s overall timeout. Bound the whole call
    // in 5s so a regression that fails to time out hangs the suite loudly
    // rather than silently passing.
    let first = tokio::time::timeout(Duration::from_secs(5), client.list_tools()).await;
    let first = first.expect("list_tools must return within 5s, not hang past the 1s timeout");
    assert!(
        first.is_err(),
        "a 2s upstream response must surface as a timeout error against a 1s timeout, got: {first:?}"
    );

    // (2) The SAME client must recover — the slow response did not wedge the
    // transport. A subsequent tools/list succeeds...
    let recovered = client.list_tools().await.expect("transport must recover after a timeout");
    assert!(
        recovered.iter().any(|t| t.name == "echo"),
        "recovered tools/list should return the programmed tool, got: {:?}",
        recovered.iter().map(|t| &t.name).collect::<Vec<_>>()
    );

    // ...and a real tool call on the recovered session round-trips (retry-
    // after-timeout works end to end, not just the listing path).
    let result = client
        .call_tool("echo", serde_json::json!({ "message": "hi" }), None, None, None)
        .await
        .expect("tool call after timeout-recovery must succeed");
    assert!(!result.is_error, "post-recovery tool call should not be an error");
    let combined: String = result
        .content
        .iter()
        .filter_map(|c| serde_json::to_string(&c.content).ok())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        combined.contains("pong-after-recovery"),
        "expected recovered tool result; got: {combined}"
    );

    client.disconnect().await.ok();
}
