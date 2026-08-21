//! Response-framing regression tests for the MCP HTTP client.
//!
//! These drive the REAL `HttpMcpClient::call_tool` path over a real TCP socket
//! against the programmable `MockMcpServer`; only the external boundary (the
//! MCP server) is mocked. They exist because the client used to decide whether
//! a response was SSE or plain JSON by searching the whole body for the
//! substring `"data: "`, while the payload EXTRACTOR required a LINE beginning
//! with `data: `. Those two tests disagree, so a plain-JSON tool result whose
//! *content* merely contained `data: ` anywhere was routed into the SSE branch,
//! matched no line, and died with "No data found in SSE response" — silently,
//! from the user's point of view: the tool appeared to run and returned nothing.
//!
//! The invariants under test (see `.lifecycle/mcp-response-content-sniffing/DESIGN.md`):
//!   INV-1 — framing is decided by `Content-Type`, never by a body substring.
//!   INV-2 — plain JSON whose CONTENT contains `data: ` parses unchanged.
//!   INV-3 — genuine SSE still parses, incl. no-space `data:` and multi-line data.
//!
//! Both directions are covered on purpose. A "fix" that broke SSE entirely would
//! satisfy INV-2 alone, so the SSE counterparts are what make the JSON tests mean
//! something.

use super::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};
use ziee::{HttpMcpClient, McpClient, McpServer, TransportType, UsageMode};

/// The literal string that reproduced the defect on the live rig: one of the
/// citations returned by `list_citations` is titled "Mobilizing the base of
/// neuroscience data: the case of neuronal morphologies". Note `"data: "`
/// appears MID-LINE (inside the title), and no line of the body begins with it —
/// which is exactly the predicate/extractor disagreement.
const REPRO_TITLE: &str =
    "Mobilizing the base of neuroscience data: the case of neuronal morphologies";

fn server_config(url: String) -> McpServer {
    McpServer {
        id: uuid::Uuid::new_v4(),
        user_id: None,
        name: "mock-mcp-framing".to_string(),
        display_name: "Mock MCP (response-framing fixture)".to_string(),
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
        // No sampling handler → `call_tool` takes the non-streaming path whose
        // Branch 3 carried the defect.
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

fn init_result() -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": "2025-06-18",
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "mock", "version": "0.0.1" },
    })
}

/// The JSON-RPC envelope a server returns for `tools/call`, carrying `text` as
/// the tool's text content. Serialized to ONE line, exactly like the 193,956-byte
/// single-line body the live rig returned.
fn tool_call_envelope(text: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "result": {
            "content": [ { "type": "text", "text": text } ],
            "isError": false
        }
    })
    .to_string()
}

/// Connect a client to the mock and return it, with `initialize` programmed.
async fn connected_client(mock: &MockMcpServer) -> HttpMcpClient {
    mock.on_method("initialize", MockResponse::JsonOk(init_result()));
    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");
    client
}

/// Pull the first text block out of a tool result by round-tripping through
/// JSON. Generic over `Serialize` so the test never names `ToolResult` (which
/// is not re-exported at the crate root) nor the `ToolContent` variant shape.
fn first_text<T: serde::Serialize>(result: &T) -> String {
    let v = serde_json::to_value(result).expect("ToolResult serializes");
    v.get("content")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|b| b.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or_default()
        .to_string()
}

// ─── TEST-6 [acceptance][INV-2] — the literal reproduction ───────────────────

/// A server answers `tools/call` with `Content-Type: application/json` and a
/// result whose TEXT CONTENT contains `data: `. The client must return that
/// content. Pre-fix this fails with "No data found in SSE response".
#[tokio::test]
async fn plain_json_tool_result_containing_data_colon_parses() {
    let mock = MockMcpServer::start().await;
    let mut client = connected_client(&mock).await;

    let body = tool_call_envelope(REPRO_TITLE);
    // Sanity-check the fixture actually reproduces the predicate/extractor
    // disagreement, so this test cannot silently stop testing the defect.
    assert!(
        body.contains("data: "),
        "fixture must contain the literal `data: ` (that is the defect trigger)"
    );
    assert!(
        !body.lines().any(|l| l.starts_with("data:")),
        "fixture must have NO line beginning with `data:` — the body is plain JSON, \
         not SSE; if this ever fails the fixture stopped reproducing the defect"
    );

    mock.on_method(
        "tools/call",
        MockResponse::Raw {
            status: 200,
            content_type: "application/json",
            body,
        },
    );

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.call_tool("list_citations", serde_json::json!({}), None, None, None),
    )
    .await
    .expect("call_tool must not hang")
    .expect("a plain-JSON tool result whose CONTENT contains `data: ` must parse");

    assert_eq!(
        first_text(&result),
        REPRO_TITLE,
        "the tool's content must be returned verbatim"
    );

    client.disconnect().await.ok();
}

// ─── TEST-7 [acceptance][INV-3] — the SSE counterpart ────────────────────────

/// The other direction: a genuine `text/event-stream` response must still parse.
/// Without this, a "fix" that simply broke SSE would look green.
#[tokio::test]
async fn genuine_sse_tool_result_still_parses() {
    let mock = MockMcpServer::start().await;
    let mut client = connected_client(&mock).await;

    // `SseStream` frames each line as `data: <line>` under text/event-stream.
    mock.on_method(
        "tools/call",
        MockResponse::SseStream(vec![
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "__ID__",
                "result": {
                    "content": [ { "type": "text", "text": "sse-payload" } ],
                    "isError": false
                }
            })
            .to_string(),
        ]),
    );

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.call_tool("anything", serde_json::json!({}), None, None, None),
    )
    .await
    .expect("call_tool must not hang on a genuine SSE response")
    .expect("a genuine text/event-stream tool result must parse");

    assert_eq!(first_text(&result), "sse-payload");

    client.disconnect().await.ok();
}

// ─── TEST-8 [acceptance][INV-3] — spec-legal no-space `data:` ────────────────

/// The EventSource spec makes the space after `data:` OPTIONAL. The client's
/// in-stream extractor used `starts_with("data: ")`, so a conforming server
/// writing `data:{...}` had its payload skipped entirely (treated as a
/// keep-alive) and the call never produced a result.
#[tokio::test]
async fn sse_tool_result_with_no_space_after_data_parses() {
    let mock = MockMcpServer::start().await;
    let mut client = connected_client(&mock).await;

    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "result": {
            "content": [ { "type": "text", "text": "no-space-payload" } ],
            "isError": false
        }
    })
    .to_string();
    // NOTE: no space after `data:` — legal per the EventSource spec.
    mock.on_method(
        "tools/call",
        MockResponse::SseRaw(format!("data:{payload}\n\n")),
    );

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.call_tool("anything", serde_json::json!({}), None, None, None),
    )
    .await
    .expect("call_tool must not hang on a no-space `data:` event")
    .expect("a spec-legal `data:` event with no space must parse");

    assert_eq!(first_text(&result), "no-space-payload");

    client.disconnect().await.ok();
}

// ─── TEST-9 [acceptance][INV-3] — multi-line `data:` block ───────────────────

/// A single SSE event may carry MULTIPLE `data:` lines, concatenated with `\n`
/// per the EventSource spec. The client's in-stream extractor used `.find(..)`,
/// reading only the FIRST line, so a split payload was truncated to a fragment
/// and failed to parse as JSON.
#[tokio::test]
async fn sse_tool_result_split_across_multiple_data_lines_parses() {
    let mock = MockMcpServer::start().await;
    let mut client = connected_client(&mock).await;

    // Split the JSON across two `data:` lines in ONE event block. The split is
    // at an explicit structural boundary (after a top-level comma) rather than
    // at a byte midpoint, so the fixture is deterministic and neither fragment
    // begins with whitespace that a spec-conformant reader would strip.
    // Per spec the receiver joins the lines with `\n`; JSON tolerates the
    // interior newline, so the concatenation is valid iff BOTH lines were read.
    let head = r#"{"jsonrpc":"2.0","id":2,"#;
    let tail =
        r#""result":{"content":[{"type":"text","text":"multiline-payload"}],"isError":false}}"#;
    // Guard the fixture: neither half is valid JSON alone, so this test cannot
    // pass by reading only one `data:` line.
    assert!(
        serde_json::from_str::<serde_json::Value>(head).is_err()
            && serde_json::from_str::<serde_json::Value>(tail).is_err(),
        "each fragment alone must be invalid JSON, or the test would not prove concatenation"
    );
    assert!(
        serde_json::from_str::<serde_json::Value>(&format!("{head}\n{tail}")).is_ok(),
        "the joined fragments must form valid JSON"
    );
    mock.on_method(
        "tools/call",
        MockResponse::SseRaw(format!("data: {head}\ndata: {tail}\n\n")),
    );

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.call_tool("anything", serde_json::json!({}), None, None, None),
    )
    .await
    .expect("call_tool must not hang on a multi-line `data:` event")
    .expect("an SSE payload split across multiple `data:` lines must be concatenated");

    assert_eq!(first_text(&result), "multiline-payload");

    client.disconnect().await.ok();
}

// ─── TEST-10 [acceptance][INV-1] — Content-Type is what decides ──────────────

/// The sharp form of INV-1: the SAME JSON-RPC envelope, delivered once as a raw
/// body under `application/json` and once SSE-framed under `text/event-stream`,
/// must produce the SAME tool result. The framing follows the declared
/// Content-Type; the bytes of the body do not select the branch.
#[tokio::test]
async fn same_envelope_under_both_content_types_yields_same_result() {
    // The envelope deliberately contains `data: ` in its CONTENT, so that the
    // only thing distinguishing the two runs is the declared Content-Type.
    let text = format!("prefix {REPRO_TITLE} suffix");
    let envelope = tool_call_envelope(&text);

    // --- delivered as plain JSON ---
    let mock_json = MockMcpServer::start().await;
    let mut client_json = connected_client(&mock_json).await;
    mock_json.on_method(
        "tools/call",
        MockResponse::Raw {
            status: 200,
            content_type: "application/json",
            body: envelope.clone(),
        },
    );
    let json_result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client_json.call_tool("t", serde_json::json!({}), None, None, None),
    )
    .await
    .expect("no hang (json)")
    .expect("plain-JSON delivery must parse");
    client_json.disconnect().await.ok();

    // --- the same envelope, SSE-framed ---
    let mock_sse = MockMcpServer::start().await;
    let mut client_sse = connected_client(&mock_sse).await;
    mock_sse.on_method(
        "tools/call",
        MockResponse::SseRaw(format!("data: {envelope}\n\n")),
    );
    let sse_result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client_sse.call_tool("t", serde_json::json!({}), None, None, None),
    )
    .await
    .expect("no hang (sse)")
    .expect("SSE delivery of the same envelope must parse");
    client_sse.disconnect().await.ok();

    assert_eq!(
        first_text(&json_result),
        text,
        "plain-JSON delivery must return the content verbatim"
    );
    assert_eq!(
        first_text(&json_result),
        first_text(&sse_result),
        "the same envelope must yield the same result under either framing — \
         Content-Type selects the parser, the body bytes do not"
    );
}
