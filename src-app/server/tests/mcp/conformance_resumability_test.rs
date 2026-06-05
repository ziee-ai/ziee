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
use ziee::{
    HttpMcpClient, McpClient, McpServer, OAuthClientConfig, TransportType, UsageMode,
};

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
    // With Plan-3 Phase-3 (I2) also opening a standalone GET on connect, the
    // mock sees BOTH; pick the resume one by `Last-Event-Id` header presence.
    let received = mock.received();
    let resume_get = received
        .iter()
        .find(|r| r.method == "__get_sse" && r.headers.contains_key("last-event-id"))
        .expect("client must issue a resume GET carrying Last-Event-Id");
    let leid = resume_get
        .headers
        .get("last-event-id")
        .expect("resume GET must carry a Last-Event-Id header");
    assert_eq!(leid, "s1_0", "resume must reference the last priming event id");

    client.disconnect().await.ok();
}

// ─── Phase-3 audit follow-ups (H3 / H4 / H5 / M3 / M4 / M5) ──────────────────

/// **H3 / M3**: the standalone GET-SSE reconnects after the server closes,
/// carrying the last `id:` it observed in the `Last-Event-Id` header. This
/// proves the per-iteration last-event-id tracking + the reconnect loop +
/// the spec's "resume from where you left off" semantics.
///
/// Mock orchestration: queue a SHORT-RETRY event on the standalone GET (sets
/// the backoff initial to 100ms so the test finishes fast), and queue the
/// reconnect's payload on the resume queue (the second GET will carry a
/// `Last-Event-Id` header, so per the mock's queue split it routes there).
#[tokio::test]
async fn standalone_get_sse_reconnects_with_last_event_id_on_close() {
    let mock = MockMcpServer::start().await;
    // First open: deliver one event with id=g_0 and `retry: 100` (cuts the
    // 1000ms default reconnect delay to 100ms), then close.
    mock.on_get_standalone(MockResponse::SseRaw(
        "retry: 100\nevent: message\nid: g_0\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/message\",\"params\":{\"level\":\"info\",\"data\":\"first\"}}\n\n".to_string(),
    ));
    // Reconnect: the second GET carries `Last-Event-Id: g_0` → pulls from the
    // resume queue.
    mock.on_get_resume(MockResponse::SseRaw(
        "event: message\nid: g_1\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/message\",\"params\":{\"level\":\"info\",\"data\":\"second\"}}\n\n".to_string(),
    ));

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    // Wait up to ~3s for the second GET (the reconnect) to land at the mock.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let resume_get_arrived = loop {
        let received = mock.received();
        let resume_get = received
            .iter()
            .find(|r| r.method == "__get_sse" && r.headers.contains_key("last-event-id"));
        if resume_get.is_some() {
            break resume_get.cloned();
        }
        if std::time::Instant::now() > deadline {
            panic!(
                "reconnect GET with Last-Event-Id never arrived; received: {:?}",
                received
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    };
    let resume_get = resume_get_arrived.expect("resume GET found");
    assert_eq!(
        resume_get.headers.get("last-event-id").map(String::as_str),
        Some("g_0"),
        "reconnect must carry the last delivered event id"
    );

    client.disconnect().await.ok();
}

/// **H4 / M4**: a server `retry:` field overrides the default 1000ms initial
/// backoff. We measure the elapsed time between the first close and the
/// second GET to confirm the reconnect happened well under the SDK default —
/// proves the `retry:` honoring path runs (and rules out a "we just sleep
/// 1000ms" regression).
#[tokio::test]
async fn standalone_get_sse_honors_server_retry_field_for_reconnect_delay() {
    let mock = MockMcpServer::start().await;
    // Event with `retry: 100` — slashes the reconnect delay 10x.
    mock.on_get_standalone(MockResponse::SseRaw(
        "retry: 100\nevent: message\nid: g_0\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/message\",\"params\":{}}\n\n".to_string(),
    ));
    // Second GET — we don't care about its body, just that it arrived in time.
    mock.on_get_resume(MockResponse::SseRaw(
        "event: message\nid: g_1\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/message\",\"params\":{}}\n\n".to_string(),
    ));

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    let started = std::time::Instant::now();
    client.connect().await.expect("connect");

    // Wait for the resume GET; budget < the 1000ms SDK default so a
    // regression that ignored `retry:` and used the default would fail.
    let deadline = started + std::time::Duration::from_millis(700);
    loop {
        if mock
            .received()
            .iter()
            .any(|r| r.method == "__get_sse" && r.headers.contains_key("last-event-id"))
        {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!(
                "reconnect GET did not arrive within 700ms — server `retry:` likely ignored"
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }

    client.disconnect().await.ok();
}

/// **H5 / M5**: when the standalone GET-SSE returns 401, the client runs the
/// OAuth refresh flow and retries with the fresh bearer. Mock orchestration:
///   1. `enable_oauth` so POST init triggers the initial token acquisition.
///   2. `arm_401_on_next_get` to force the FIRST standalone GET to 401 with
///      a `WWW-Authenticate` challenge.
///   3. Queue a normal event for AFTER refresh (the second GET carries the
///      same `Bearer` value — the mock's token endpoint returns the same
///      access token; the assertion is that there WERE two GET attempts +
///      an extra `/token` call between them).
#[tokio::test]
async fn standalone_get_sse_refreshes_oauth_on_401_and_retries() {
    let mock = MockMcpServer::start().await;
    mock.enable_oauth("cid", "sec", "tok-v1");
    mock.arm_401_on_next_get();
    // Successful event on the retry — bearer is now re-acquired and the
    // mock just serves the next standalone queue entry.
    mock.on_get_standalone(MockResponse::SseRaw(
        "event: message\nid: g_0\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/message\",\"params\":{}}\n\n".to_string(),
    ));

    let cfg = OAuthClientConfig {
        client_id: "cid".to_string(),
        client_secret: "sec".to_string(),
        scopes: None,
        resource: None,
    };
    let mut client =
        HttpMcpClient::new_with_oauth(server_config(mock.base_url()), cfg).unwrap();
    client.connect().await.expect("connect");

    // Wait for the second GET (post-refresh) AND the extra /token round-trip.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        let received = mock.received();
        let get_count = received.iter().filter(|r| r.method == "__get_sse").count();
        let token_count = received.iter().filter(|r| r.method == "__token").count();
        // First /token was the initial acquisition during connect's POST 401
        // flow; the second is the refresh triggered by the GET 401.
        if get_count >= 2 && token_count >= 2 {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!(
                "expected ≥2 GET-SSE attempts + ≥2 /token calls (initial + refresh); \
                 got GET={get_count}, token={token_count}"
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    client.disconnect().await.ok();
}

// ─── I2: standalone GET-SSE opens after `initialized` and tolerates 405 ─────

/// `connect()` MUST open a standalone GET-SSE per MCP spec § Transports and
/// MUST tolerate `405 Method Not Allowed` silently — that's how a server
/// signals "no standalone stream". The mock's default GET-handler returns
/// 405 when the standalone queue is empty.
#[tokio::test]
async fn standalone_get_sse_opens_after_initialize_and_405_is_silent() {
    let mock = MockMcpServer::start().await;
    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();

    client
        .connect()
        .await
        .expect("connect should succeed even though the server 405s the GET");

    // The standalone GET happens on a spawned task; poll until it lands at the
    // mock (or fail the test on timeout). Bounded short loop — the open IS
    // synchronous from connect's POV but the network round-trip is async.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let standalone_get = loop {
        let received = mock.received();
        if let Some(r) = received
            .iter()
            .find(|r| r.method == "__get_sse" && !r.headers.contains_key("last-event-id"))
            .cloned()
        {
            break r;
        }
        if std::time::Instant::now() > deadline {
            panic!("standalone GET-SSE never arrived at the mock");
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    };

    // Spec headers on the GET: Accept, mcp-session-id, MCP-Protocol-Version.
    assert!(
        standalone_get
            .headers
            .get("accept")
            .map(|v| v.contains("text/event-stream"))
            .unwrap_or(false),
        "standalone GET must advertise Accept: text/event-stream; got {:?}",
        standalone_get.headers.get("accept")
    );
    assert!(
        standalone_get.headers.get("mcp-session-id").is_some(),
        "standalone GET must carry the negotiated session id"
    );
    assert!(
        standalone_get.headers.get("mcp-protocol-version").is_some(),
        "standalone GET must carry the negotiated protocol version"
    );
    assert!(
        standalone_get
            .headers
            .get("last-event-id")
            .map(|v| v.is_empty())
            .unwrap_or(true),
        "standalone GET (no resume) must NOT carry a Last-Event-Id"
    );

    client.disconnect().await.ok();
}

/// When the server returns a real event stream on the standalone GET (rather
/// than 405), the client task MUST consume it without erroring. Today the
/// router logs received events; tomorrow it will dispatch them. Either way,
/// connect/disconnect MUST stay clean.
#[tokio::test]
async fn standalone_get_sse_consumes_a_programmed_event_cleanly() {
    let mock = MockMcpServer::start().await;
    // A spec-shaped notifications/message event on the standalone GET.
    mock.on_get_standalone(MockResponse::SseRaw(
        "event: message\nid: g_0\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/message\",\"params\":{\"level\":\"info\",\"data\":\"hello\"}}\n\n".to_string(),
    ));

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    // Wait for the GET to land at the mock (mirror the 405 test).
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        if mock
            .received()
            .iter()
            .any(|r| r.method == "__get_sse" && !r.headers.contains_key("last-event-id"))
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(
        mock.count_for("__get_sse") >= 1,
        "standalone GET must have been issued"
    );

    // Give the event consumer a moment to drain — there's no observable side
    // effect today (router logs), so we just assert disconnect is clean.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    client.disconnect().await.expect("clean disconnect");
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

    // No RESUME GET should have been attempted (no priming event id was
    // seen). I2 also opens a standalone GET on connect, so the test counts
    // only GETs that carry `Last-Event-Id` — those are the resume ones.
    let resume_gets = mock
        .received()
        .iter()
        .filter(|r| r.method == "__get_sse" && r.headers.contains_key("last-event-id"))
        .count();
    assert_eq!(
        resume_gets, 0,
        "client must not attempt resume without a Last-Event-Id"
    );

    client.disconnect().await.ok();
}
