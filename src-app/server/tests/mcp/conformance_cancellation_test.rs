//! Plan-3 Phase-2 (C3) — client request cancellation conformance.
//!
//! `McpClient::cancel(request_id, reason)` MUST send a fire-and-forget
//! `notifications/cancelled` (MCP spec § utilities/cancellation) carrying the
//! abandoned request id + reason, so the server can stop the work.

use super::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};
use ziee::{HttpMcpClient, McpClient, McpServer, TransportType, UsageMode};

fn server_config(url: String) -> McpServer {
    McpServer {
        id: uuid::Uuid::new_v4(),
        user_id: None,
        name: "mock-mcp-cancel".to_string(),
        display_name: "Mock MCP (cancellation fixture)".to_string(),
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

#[tokio::test]
async fn cancel_sends_notifications_cancelled_with_request_id_and_reason() {
    let mock = MockMcpServer::start().await;
    mock.on_method("notifications/cancelled", MockResponse::Accepted);

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    client
        .cancel(42, "user navigated away")
        .await
        .expect("cancel notification should be delivered");

    let received = mock.received();
    let note = received
        .iter()
        .find(|r| r.method == "notifications/cancelled")
        .expect("server must receive a notifications/cancelled");
    // It's a notification — no id at the JSON-RPC envelope level.
    assert!(note.id.is_none(), "notifications/cancelled must not carry a JSON-RPC id");
    let params = &note.body["params"];
    assert_eq!(params["requestId"], 42, "must reference the abandoned request id");
    assert_eq!(params["reason"], "user navigated away", "must carry the reason");

    client.disconnect().await.ok();
}
