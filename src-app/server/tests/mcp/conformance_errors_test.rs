//! Error-path conformance tests for the MCP HTTP client.
//!
//! These tests use the programmable [`MockMcpServer`] fixture to drive the
//! client through error cases that the real reference server
//! (`@modelcontextprotocol/server-everything`) won't produce: JSON-RPC error
//! responses, malformed bodies, unexpected HTTP statuses, missing fields,
//! session-recovery (404→reinit), header propagation, and DELETE-on-disconnect.
//!
//! The goal is empirical proof that the client behaves correctly when servers
//! misbehave — the user's "100% confidence the client never breaks against any
//! server" requirement.

use super::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};
use ziee::{HttpMcpClient, McpClient, McpServer, TransportType, UsageMode};

fn server_config(url: String) -> McpServer {
    McpServer {
        id: uuid::Uuid::new_v4(),
        user_id: None,
        name: "mock-mcp".to_string(),
        display_name: "Mock MCP (error fixture)".to_string(),
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
        timeout_seconds: 10,
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

// ─── JSON-RPC error responses ──────────────────────────────────────────────

#[tokio::test]
async fn error_jsonrpc_method_not_found_on_tools_list_propagates() {
    let mock = MockMcpServer::start().await;
    mock.on_method("tools/list", MockResponse::JsonRpcError {
        code: -32601,
        message: "Method not found".to_string(),
    });

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let err = client.list_tools().await.expect_err("should surface -32601");
    let msg = err.to_string();
    assert!(msg.contains("-32601") || msg.to_lowercase().contains("method not found"),
            "expected error to mention -32601 or 'method not found'; got: {}", msg);

    client.disconnect().await.ok();
}

#[tokio::test]
async fn error_jsonrpc_internal_error_on_tool_call_propagates() {
    let mock = MockMcpServer::start().await;
    mock.on_method("tools/call", MockResponse::JsonRpcError {
        code: -32603,
        message: "Internal error: database unavailable".to_string(),
    });

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let err = client.call_tool(
        "anything",
        serde_json::json!({}),
        None, None, None,
    ).await.expect_err("tool call should fail");
    let msg = err.to_string();
    assert!(msg.contains("database unavailable") || msg.contains("-32603"),
            "expected error to include server-reported message; got: {}", msg);

    client.disconnect().await.ok();
}

#[tokio::test]
async fn error_list_prompts_method_not_found_is_treated_as_empty() {
    // Per http.rs comment: servers without `prompts` capability return -32601
    // for prompts/list, and we treat that as an empty list rather than an
    // error. This test pins that behaviour so the leniency isn't lost in a
    // refactor.
    let mock = MockMcpServer::start().await;
    mock.on_method("prompts/list", MockResponse::JsonRpcError {
        code: -32601,
        message: "Method not found".to_string(),
    });

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let prompts = client.list_prompts().await.expect("should treat -32601 as empty");
    assert!(prompts.is_empty());

    client.disconnect().await.ok();
}

// ─── Malformed / unexpected HTTP responses ─────────────────────────────────

#[tokio::test]
async fn error_malformed_json_body_surfaces_error_not_panic() {
    let mock = MockMcpServer::start().await;
    mock.on_method("tools/list", MockResponse::Raw {
        status: 200,
        content_type: "application/json",
        body: r#"{"jsonrpc": "2.0", "id": 1, "result": {{ broken"#.to_string(),
    });

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let err = client.list_tools().await.expect_err("malformed JSON must error, not panic");
    // We don't pin the exact error message — could be a serde parse error or
    // a wrapper from our HTTP layer. The contract is "graceful failure".
    let msg = err.to_string().to_lowercase();
    assert!(msg.contains("parse") || msg.contains("json") || msg.contains("expected")
            || msg.contains("invalid") || msg.contains("eof"),
            "expected JSON parse error wording; got: {}", err);
}

#[tokio::test]
async fn error_http_500_surfaces_as_error_not_panic() {
    let mock = MockMcpServer::start().await;
    mock.on_method("tools/list", MockResponse::Raw {
        status: 500,
        content_type: "application/json",
        body: r#"{"error":"upstream down"}"#.to_string(),
    });

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let err = client.list_tools().await.expect_err("HTTP 500 must error");
    let msg = err.to_string();
    assert!(msg.contains("500") || msg.to_lowercase().contains("upstream")
            || msg.to_lowercase().contains("internal"),
            "expected error to reference HTTP 500 / server error; got: {}", msg);
}

#[tokio::test]
async fn error_http_400_surfaces_as_error_not_panic() {
    let mock = MockMcpServer::start().await;
    mock.on_method("tools/list", MockResponse::Raw {
        status: 400,
        content_type: "application/json",
        body: r#"{"error":"bad request"}"#.to_string(),
    });

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let err = client.list_tools().await.expect_err("HTTP 400 must error");
    let msg = err.to_string();
    assert!(!msg.is_empty(), "error must have a meaningful message");
}

#[tokio::test]
async fn error_jsonrpc_response_missing_result_and_error_fields() {
    // A JSON-RPC reply MUST have either `result` or `error`. A reply with
    // neither is malformed. The client should reject it cleanly.
    let mock = MockMcpServer::start().await;
    mock.on_method("tools/list", MockResponse::Raw {
        status: 200,
        content_type: "application/json",
        body: r#"{"jsonrpc":"2.0","id":1}"#.to_string(),
    });

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let res = client.list_tools().await;
    // Either an Err, or an Ok with empty tools list (some deserialization
    // paths return defaulted struct). Both are acceptable — what we're
    // checking is "no panic, no infinite loop".
    match res {
        Err(_) => { /* preferred */ }
        Ok(tools) => assert!(tools.is_empty(), "missing-result reply should not yield tools"),
    }
}

// ─── Session-recovery: 404 → reinitialize ──────────────────────────────────

#[tokio::test]
async fn error_404_on_session_triggers_reinitialize() {
    // Per spec § Session Management: "When a client receives HTTP 404 in
    // response to a request containing an MCP-Session-Id, it MUST start a
    // new session." Our client implements this with one retry.
    let mock = MockMcpServer::start().await;
    // Queue a successful tools/list response — it will be served on the
    // retry attempt after the 404 forces a reinit.
    mock.on_method("tools/list", MockResponse::JsonOk(serde_json::json!({
        "tools": [{
            "name": "test-tool",
            "description": "a test tool",
            "inputSchema": { "type": "object" },
        }]
    })));

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("initial connect");

    // Arm the next request to return 404 — this should be the tools/list
    // call below. The client should catch the 404, drop the session,
    // reinitialize, and retry.
    mock.arm_404_once();

    let tools = client.list_tools().await
        .expect("client should recover from 404 by reinitializing and retrying");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "test-tool");

    // Verify the mock saw the recovery sequence: a second `initialize` was
    // issued, and the count of initializes is now 2.
    let init_count = mock.count_for("initialize");
    assert_eq!(init_count, 2,
               "expected client to reinitialize after 404 (saw {} initializes)", init_count);

    client.disconnect().await.ok();
}

// ─── Header / lifecycle invariants ─────────────────────────────────────────

#[tokio::test]
async fn invariant_accept_header_includes_both_json_and_sse() {
    // MCP spec § Transports — Streamable HTTP: "The client MUST include an
    // Accept header listing both application/json and text/event-stream as
    // supported content types."
    let mock = MockMcpServer::start().await;
    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    // Trigger a regular request so we capture the headers.
    let _ = client.list_tools().await;

    let received = mock.received();
    // Find at least one non-initialize POST and check its accept header.
    let req = received.iter().find(|r| r.method == "tools/list")
        .expect("should have observed tools/list");
    let accept = req.headers.get("accept").map(|s| s.as_str()).unwrap_or("");
    assert!(accept.contains("application/json") && accept.contains("text/event-stream"),
            "Accept header must include both content types; got: {:?}", accept);

    client.disconnect().await.ok();
}

#[tokio::test]
async fn invariant_protocol_version_header_sent_after_initialize() {
    // MCP spec § Transports: After initialize, every subsequent request MUST
    // carry `MCP-Protocol-Version: <negotiated-version>`.
    let mock = MockMcpServer::start().await;
    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let _ = client.list_tools().await;

    let received = mock.received();
    let req = received.iter().find(|r| r.method == "tools/list")
        .expect("should have observed tools/list");
    let ver = req.headers.get("mcp-protocol-version").cloned().unwrap_or_default();
    assert!(!ver.is_empty(),
            "MCP-Protocol-Version must be sent on post-initialize requests; got: {:?}",
            req.headers);

    client.disconnect().await.ok();
}

#[tokio::test]
async fn invariant_session_id_sent_in_subsequent_requests() {
    let mock = MockMcpServer::start().await;
    mock.set_session_id(Some("test-session-xyz"));

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");
    let _ = client.list_tools().await;

    let received = mock.received();
    let req = received.iter().find(|r| r.method == "tools/list")
        .expect("should have observed tools/list");
    let sid = req.headers.get("mcp-session-id").cloned().unwrap_or_default();
    assert_eq!(sid, "test-session-xyz",
               "MCP-Session-Id from initialize must be propagated to later requests");

    client.disconnect().await.ok();
}

#[tokio::test]
async fn invariant_initialized_notification_sent_after_initialize() {
    // MCP spec MUST: client sends `notifications/initialized` after
    // receiving the initialize response, before any other request.
    let mock = MockMcpServer::start().await;
    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let received = mock.received();
    let init_idx = received.iter().position(|r| r.method == "initialize")
        .expect("must observe initialize");
    let notif_idx = received.iter()
        .position(|r| r.method == "notifications/initialized")
        .expect("MUST send notifications/initialized after initialize");
    assert!(notif_idx > init_idx,
            "notifications/initialized must come AFTER initialize");

    client.disconnect().await.ok();
}

#[tokio::test]
async fn invariant_unique_request_ids_across_calls() {
    let mock = MockMcpServer::start().await;
    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    // Queue 3 successful list_tools responses
    for _ in 0..3 {
        mock.on_method("tools/list", MockResponse::JsonOk(serde_json::json!({"tools": []})));
    }

    for _ in 0..3 {
        let _ = client.list_tools().await;
    }

    let received = mock.received();
    let ids: Vec<i64> = received.iter()
        .filter(|r| r.method == "tools/list")
        .filter_map(|r| r.id)
        .collect();
    assert_eq!(ids.len(), 3, "expected 3 tools/list calls");

    // Each id must be unique within the session
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(),
               "request ids must be unique within a session; saw {:?}", ids);

    client.disconnect().await.ok();
}

#[tokio::test]
async fn invariant_disconnect_sends_http_delete() {
    let mock = MockMcpServer::start().await;
    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");
    client.disconnect().await.expect("disconnect");

    // The mock records DELETE as a synthetic method name
    assert!(mock.count_for("__delete_session") >= 1,
            "disconnect MUST issue an HTTP DELETE to terminate the session");
}

#[tokio::test]
async fn invariant_no_calls_when_not_connected() {
    // The client gates every method behind is_connected(). Calling without
    // connect() must return an error, never panic, and must not perform any
    // network I/O (mock should see zero requests).
    let mock = MockMcpServer::start().await;
    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();

    assert!(client.list_tools().await.is_err());
    assert!(client.list_prompts().await.is_err());
    assert!(client.ping().await.is_err());

    assert_eq!(mock.received().len(), 0,
               "no network calls should be issued when not connected");
}

/// MCP server CRASH then RESTART/RESUME. No test exercised the client's behavior
/// when the upstream MCP server process goes away mid-session: a call against a
/// crashed server must FAIL cleanly (connection refused, bounded — not an
/// infinite hang), and the client subsystem must RECOVER when a fresh server
/// comes back (no permanently-poisoned state). Crash is modeled by dropping the
/// in-process MockMcpServer (its axum task is aborted → the port closes).
#[tokio::test]
async fn client_fails_cleanly_on_crash_and_recovers_after_restart() {
    // Live server → connect + list_tools succeed.
    let mock = MockMcpServer::start().await;
    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect to a live server");
    client.list_tools().await.expect("list_tools on a live server");

    // CRASH — drop the mock; its server task is aborted and the port closes.
    drop(mock);
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;

    // A call to the crashed server must ERROR (connection refused), not hang.
    let after_crash = client.list_tools().await;
    assert!(
        after_crash.is_err(),
        "a tool call against a crashed MCP server must surface an error, not hang"
    );
    client.disconnect().await.ok();

    // RESTART / RESUME — a fresh server + client recovers end-to-end, proving the
    // crash did not leave the client layer permanently broken.
    let mock2 = MockMcpServer::start().await;
    let mut client2 = HttpMcpClient::new(server_config(mock2.base_url())).unwrap();
    client2.connect().await.expect("reconnect to the restarted server");
    client2.list_tools().await.expect("list_tools must succeed after restart");
    client2.disconnect().await.ok();
}
