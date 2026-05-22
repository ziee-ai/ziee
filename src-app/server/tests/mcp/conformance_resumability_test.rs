//! Plan-3 Phase-3 (I1) — Streamable-HTTP **resumability** client conformance.
//!
//! Per MCP spec § Transports/Resumability (and the MCP TypeScript SDK
//! `client/streamableHttp.ts`): when a tool-call SSE stream carries event ids
//! (a "priming event") and then drops *before* delivering the JSON-RPC
//! response, the client MUST reconnect via `GET` + `Last-Event-Id` and resume,
//! rather than failing the whole call.
//!
//! The mock simulates the disconnect deterministically: the `tools/call` POST
//! returns only a priming `id:` event (empty data) and closes; the queued GET
//! response then delivers the real result. We assert the client (a) recovers
//! the result and (b) issued a GET carrying the correct `Last-Event-Id`.

use super::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};
use ziee_chat::{HttpMcpClient, McpClient, McpServer, TransportType, UsageMode};

fn server_config(url: String) -> McpServer {
    McpServer {
        id: uuid::Uuid::new_v4(),
        user_id: None,
        name: "mock-mcp-resume".to_string(),
        display_name: "Mock MCP (resumability fixture)".to_string(),
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
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

// ─── I1: resume a dropped tool-call stream via Last-Event-Id ─────────────────

#[tokio::test]
async fn tool_call_resumes_dropped_stream_via_last_event_id() {
    let mock = MockMcpServer::start().await;

    // POST tools/call → priming event id=s1_0 then EOF (no result). This is the
    // "server disconnected after the priming event" case the client must resume.
    mock.on_method(
        "tools/call",
        MockResponse::SseRaw("id: s1_0\ndata: \n\n".to_string()),
    );
    // The resume GET delivers the actual tool result on a fresh stream.
    mock.on_get(MockResponse::SseRaw(
        r#"event: message
id: s1_1
data: {"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"resumed"}],"isError":false}}

"#
        .to_string(),
    ));

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        client.call_tool("do_thing", serde_json::json!({}), None, None, None),
    )
    .await
    .expect("client must not hang — it should resume and complete")
    .expect("client must recover the result via Last-Event-Id resume");

    assert!(!result.is_error, "resumed tool result should not be an error");
    let text = serde_json::to_string(&result.content).unwrap();
    assert!(text.contains("resumed"), "expected resumed result, got: {text}");

    // The client must have issued a GET resume carrying our last event id.
    let received = mock.received();
    let resume_get = received
        .iter()
        .find(|r| r.method == "__get_sse")
        .expect("client must issue a GET to resume the dropped stream");
    let leid = resume_get
        .headers
        .get("last-event-id")
        .expect("resume GET must carry a Last-Event-Id header");
    assert_eq!(leid, "s1_0", "resume must reference the last priming event id");

    client.disconnect().await.ok();
}

// ─── Non-resumable stream (no event ids) fails fast, does NOT GET-loop ────────

#[tokio::test]
async fn tool_call_without_event_ids_does_not_attempt_resume() {
    let mock = MockMcpServer::start().await;

    // A stream that ends with NO event id and no result → not resumable; the
    // client must surface the error rather than spin on GET reconnects.
    mock.on_method(
        "tools/call",
        MockResponse::SseRaw("data: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/message\",\"params\":{}}\n\n".to_string()),
    );

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let res = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.call_tool("do_thing", serde_json::json!({}), None, None, None),
    )
    .await
    .expect("must not hang when the stream is not resumable");
    assert!(res.is_err(), "a non-resumable dropped stream should error");

    // No GET should have been attempted (no priming event id was seen).
    assert_eq!(
        mock.count_for("__get_sse"),
        0,
        "client must not attempt resume without a Last-Event-Id"
    );

    client.disconnect().await.ok();
}
