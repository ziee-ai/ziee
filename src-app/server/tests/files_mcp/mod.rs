// ============================================================================
// files_mcp built-in MCP server tests.
//
// Tests the JSON-RPC handler at /api/files/mcp (Track A):
//   - initialize / tools/list return the 3 read-only tools.
//   - tools/call requires the x-conversation-id header (conversation-scoped).
//   - the handler is gated on `files::read` (granted to all users by default).
//
// The full read_file/list_files/grep_files round-trips over a real conversation
// with project + attached files are exercised by the chat-extension integration
// tests + E2E (they need the stub chat provider, see the test strategy).
// ============================================================================

use serde_json::{Value, json};
use uuid::Uuid;

fn jsonrpc_call(
    server: &crate::common::TestServer,
    token: &str,
    conversation_id: Option<Uuid>,
    method: &str,
    params: Value,
) -> reqwest::RequestBuilder {
    let mut req = reqwest::Client::new()
        .post(server.api_url("/files/mcp"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        }));
    if let Some(cid) = conversation_id {
        req = req.header("x-conversation-id", cid.to_string());
    }
    req
}

#[tokio::test]
async fn test_initialize_returns_server_info() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "files_mcp_init",
        &["files::read"],
    )
    .await;
    let res = jsonrpc_call(&server, &user.token, None, "initialize", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(body["result"]["serverInfo"]["name"], "files");
}

#[tokio::test]
async fn test_tools_list_returns_three_read_tools() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "files_mcp_list",
        &["files::read"],
    )
    .await;
    let res = jsonrpc_call(&server, &user.token, None, "tools/list", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    let tools = body["result"]["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert_eq!(names.len(), 3, "exactly 3 read-only tools");
    assert!(names.contains(&"list_files"));
    assert!(names.contains(&"read_file"));
    assert!(names.contains(&"grep_files"));
}

#[tokio::test]
async fn test_tools_call_requires_conversation_id() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "files_mcp_noconv",
        &["files::read"],
    )
    .await;
    // No x-conversation-id header → tools/call must error (these tools are
    // conversation-scoped), not silently operate on nothing.
    let res = jsonrpc_call(
        &server,
        &user.token,
        None,
        "tools/call",
        json!({ "name": "list_files", "arguments": {} }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert!(
        body["error"].is_object(),
        "tools/call without x-conversation-id should return a JSON-RPC error, got: {body}"
    );
}
