//! Plan-3 Phase-1 MCP client conformance tests (against the programmable mock):
//!
//! * **C1** — protocol-version negotiation: the client accepts a supported
//!   older version and **rejects** an unsupported one (disconnecting).
//! * **I3** — JSON-RPC response `id` matched structurally (string OR number),
//!   so a server echoing our numeric id as a string still correlates.
//! * **I4** — `tools/list` follows `nextCursor` pagination across pages.

use super::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};
use ziee::{HttpMcpClient, McpClient, McpServer, TransportType, UsageMode};

fn server_config(url: String) -> McpServer {
    McpServer {
        id: uuid::Uuid::new_v4(),
        user_id: None,
        name: "mock-mcp-phase1".to_string(),
        display_name: "Mock MCP (phase1 fixture)".to_string(),
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
        last_health_check_at: None,
        last_health_check_status: "untested".to_string(),
        last_health_check_reason: None,
    }
}

fn init_result(protocol_version: &str) -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": protocol_version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "mock", "version": "0.0.1" },
    })
}

// ─── C1: protocol-version negotiation ────────────────────────────────────────

#[tokio::test]
async fn connect_rejects_unsupported_protocol_version() {
    let mock = MockMcpServer::start().await;
    mock.on_method("initialize", MockResponse::JsonOk(init_result("1999-01-01")));

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    let res = client.connect().await;
    assert!(
        res.is_err(),
        "client MUST refuse to connect when the server negotiates an unsupported protocol version"
    );
    let msg = res.unwrap_err().to_string();
    assert!(
        msg.contains("unsupported protocol version"),
        "error should explain the version mismatch, got: {msg}"
    );
}

#[tokio::test]
async fn connect_accepts_supported_older_protocol_version() {
    let mock = MockMcpServer::start().await;
    // 2024-11-05 is older than our latest but still in SUPPORTED_PROTOCOL_VERSIONS.
    mock.on_method("initialize", MockResponse::JsonOk(init_result("2024-11-05")));

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client
        .connect()
        .await
        .expect("client must accept a supported older protocol version");
    client.disconnect().await.ok();
}

// ─── I3: structural response-id matching ─────────────────────────────────────

#[tokio::test]
async fn response_with_string_id_is_matched() {
    let mock = MockMcpServer::start().await;
    // The mock substitutes __ID__ with the request's numeric id; QUOTING it
    // makes the response carry a *string* id (e.g. "5") — a legal JSON-RPC id
    // shape. The client must still correlate it to its numeric request id.
    mock.on_method(
        "tools/list",
        MockResponse::SseStream(vec![
            r#"{"jsonrpc":"2.0","id":"__ID__","result":{"tools":[{"name":"strid","description":"d","inputSchema":{"type":"object"}}]}}"#
                .to_string(),
        ]),
    );

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let tools = tokio::time::timeout(std::time::Duration::from_secs(5), client.list_tools())
        .await
        .expect("client must not hang on a string-id response")
        .expect("client must match a string-form response id to its numeric request id");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "strid");

    client.disconnect().await.ok();
}

// ─── I4: nextCursor pagination ────────────────────────────────────────────────

#[tokio::test]
async fn list_tools_follows_pagination_cursor() {
    let mock = MockMcpServer::start().await;
    // First page returns a cursor; second page completes the list. The client
    // must concatenate both rather than truncating at page one.
    mock.on_method(
        "tools/list",
        MockResponse::JsonOk(serde_json::json!({
            "tools": [{ "name": "page1", "description": "d", "inputSchema": { "type": "object" } }],
            "nextCursor": "cursor-2"
        })),
    );
    mock.on_method(
        "tools/list",
        MockResponse::JsonOk(serde_json::json!({
            "tools": [{ "name": "page2", "description": "d", "inputSchema": { "type": "object" } }]
        })),
    );

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let tools = client.list_tools().await.expect("paginated list_tools");
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["page1", "page2"], "both pages must be returned");
    // The second tools/list must have carried the cursor.
    assert_eq!(mock.count_for("tools/list"), 2, "client must fetch both pages");

    client.disconnect().await.ok();
}
