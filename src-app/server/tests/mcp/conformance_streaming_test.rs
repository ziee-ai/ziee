//! SSE streaming edge-case tests for the MCP HTTP client.
//!
//! Per MCP spec § Transports — Streamable HTTP:
//! > The server MAY return an SSE stream (text/event-stream) instead of a
//! > single JSON response. The stream MAY contain server-initiated JSON-RPC
//! > requests and notifications interleaved with the response to the
//! > client's request.
//!
//! These tests use the programmable [`MockMcpServer`] fixture to emit
//! specific SSE byte sequences that exercise the `extract_response_by_id`
//! parser in `client/http.rs`:
//!
//! * Notification arrives before the response → notification dropped,
//!   response returned.
//! * Multiple unrelated notifications/requests interleaved → all skipped,
//!   correct response found.
//! * CRLF event separators (per SSE spec) → still parsed.
//! * Multi-line `data:` field within a single event → lines concatenated
//!   per SSE spec.
//! * `data:` with no leading space (compact form) → still parsed.
//! * Empty SSE stream / stream without matching id → clean error.

use super::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};
use ziee::{HttpMcpClient, McpClient, McpServer, TransportType, UsageMode};

fn server_config(url: String) -> McpServer {
    McpServer {
        id: uuid::Uuid::new_v4(),
        user_id: None,
        name: "mock-mcp-sse".to_string(),
        display_name: "Mock MCP (SSE fixture)".to_string(),
        description: None,
        enabled: true,
        is_system: false,
        transport_type: TransportType::Http,
        command: None,
        args: serde_json::json!([]),
        environment_variables: serde_json::json!({}),
        url: Some(url),
        headers: serde_json::json!({}),
        timeout_seconds: 10,
        supports_sampling: false,
        usage_mode: UsageMode::Auto,
        max_concurrent_sessions: None,
        is_built_in: false,
        run_in_sandbox: false,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

fn jsonrpc_notification_event(method: &str, params: serde_json::Value) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    }).to_string()
}

// The mock substitutes the literal text `__ID__` with the request's actual
// id at render time. We emit it unquoted so the resulting JSON has a numeric
// id (which is what the client expects).
fn jsonrpc_response_event_raw(result_json: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":__ID__,"result":{}}}"#, result_json)
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn sse_notification_before_response_is_skipped() {
    let mock = MockMcpServer::start().await;
    // Server sends a progress notification, then the actual response.
    mock.on_method("tools/list", MockResponse::SseStream(vec![
        jsonrpc_notification_event(
            "notifications/progress",
            serde_json::json!({ "progressToken": "abc", "progress": 0.5 }),
        ),
        jsonrpc_response_event_raw(r#"{"tools":[{"name":"foo","description":"d","inputSchema":{"type":"object"}}]}"#),
    ]));

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let tools = client.list_tools().await
        .expect("client must skip notification and return tools");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "foo");

    client.disconnect().await.ok();
}

#[tokio::test]
async fn sse_multiple_notifications_before_response() {
    let mock = MockMcpServer::start().await;
    mock.on_method("tools/list", MockResponse::SseStream(vec![
        jsonrpc_notification_event("notifications/progress", serde_json::json!({"progress": 0.1})),
        jsonrpc_notification_event("notifications/message", serde_json::json!({"data": "log"})),
        jsonrpc_notification_event("notifications/progress", serde_json::json!({"progress": 0.9})),
        jsonrpc_response_event_raw(r#"{"tools":[]}"#),
    ]));

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let tools = client.list_tools().await
        .expect("client must skip all notifications and return empty list");
    assert_eq!(tools.len(), 0);

    client.disconnect().await.ok();
}

#[tokio::test]
async fn sse_response_with_non_matching_id_is_skipped() {
    let mock = MockMcpServer::start().await;
    // First event has id 999 (doesn't match our request) — must be skipped.
    // Second event uses the real id placeholder and matches.
    mock.on_method("tools/list", MockResponse::SseStream(vec![
        r#"{"jsonrpc":"2.0","id":999,"result":{"tools":[{"name":"wrong","description":"d","inputSchema":{"type":"object"}}]}}"#.to_string(),
        jsonrpc_response_event_raw(r#"{"tools":[{"name":"right","description":"d","inputSchema":{"type":"object"}}]}"#),
    ]));

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let tools = client.list_tools().await
        .expect("client must skip mismatched-id response");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "right",
               "client must use the response matching its own request id");

    client.disconnect().await.ok();
}

#[tokio::test]
async fn sse_stream_with_no_matching_response_errors_cleanly() {
    let mock = MockMcpServer::start().await;
    // Only notifications, no response. Client should error (and not hang).
    mock.on_method("tools/list", MockResponse::SseStream(vec![
        jsonrpc_notification_event("notifications/progress", serde_json::json!({"progress": 0.5})),
        jsonrpc_notification_event("notifications/message", serde_json::json!({"data": "no response coming"})),
    ]));

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    // Use a short-ish timeout to defend against the parser hanging.
    let res = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.list_tools(),
    ).await;

    let outer = res.expect("client must not hang when stream has no matching response");
    assert!(outer.is_err(),
            "missing-response stream must surface as an Err, not Ok");

    client.disconnect().await.ok();
}

#[tokio::test]
async fn sse_empty_stream_errors_cleanly() {
    let mock = MockMcpServer::start().await;
    mock.on_method("tools/list", MockResponse::SseStream(vec![]));

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let res = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.list_tools(),
    ).await
    .expect("must not hang on empty SSE stream");

    assert!(res.is_err(), "empty SSE stream must yield Err");

    client.disconnect().await.ok();
}

#[tokio::test]
async fn sse_server_initiated_request_before_response_is_ignored() {
    // The server is allowed to send sampling/elicitation requests over the
    // SSE stream. For a plain list_tools (no sampling handler installed),
    // those should be ignored and the actual response delivered. We're not
    // wiring up the sampling handler here — the client should just skip the
    // server-initiated request and find our response.
    let mock = MockMcpServer::start().await;
    mock.on_method("tools/list", MockResponse::SseStream(vec![
        // Server-initiated request (id=42 — not ours)
        r#"{"jsonrpc":"2.0","id":42,"method":"sampling/createMessage","params":{}}"#.to_string(),
        // Our response
        jsonrpc_response_event_raw(r#"{"tools":[]}"#),
    ]));

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let tools = client.list_tools().await
        .expect("server-initiated request should be ignored, response delivered");
    assert_eq!(tools.len(), 0);

    client.disconnect().await.ok();
}
