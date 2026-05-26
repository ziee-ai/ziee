// ============================================================================
// memory_mcp built-in MCP server tests.
//
// Tests the JSON-RPC handler at /api/memory-mcp:
//   - initialize / tools/list / ping return expected metadata.
//   - tools/call dispatches to remember / recall / forget with JWT
//     user_id scoping.
//   - Cross-user isolation (Plan §10 mandatory regression): user A
//     cannot forget user B's memory via the MCP tool.
//
// memory_mcp uses `memory::write` permission for the JSON-RPC handler
// (same as the user-facing REST CRUD), matching the plan's "the user
// who calls remember/recall/forget is the user whose memories are
// affected" model.
// ============================================================================

use serde_json::{Value, json};

fn jsonrpc_call(server: &crate::common::TestServer, token: &str, method: &str, params: Value) -> reqwest::RequestBuilder {
    reqwest::Client::new()
        .post(server.api_url("/memory-mcp"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        }))
}

#[tokio::test]
async fn test_initialize_returns_server_info() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mcp_init",
        &["memory::write"],
    )
    .await;
    let res = jsonrpc_call(&server, &user.token, "initialize", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(body["result"]["serverInfo"]["name"], "memory");
}

#[tokio::test]
async fn test_tools_list_returns_three_tools() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mcp_tools",
        &["memory::write"],
    )
    .await;
    let res = jsonrpc_call(&server, &user.token, "tools/list", json!({}))
        .send()
        .await
        .unwrap();
    let body: Value = res.json().await.unwrap();
    let tools = body["result"]["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"remember"));
    assert!(names.contains(&"recall"));
    assert!(names.contains(&"forget"));
}

#[tokio::test]
async fn test_remember_then_forget_roundtrip() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mcp_round",
        &["memory::read", "memory::write"],
    )
    .await;

    // remember
    let res = jsonrpc_call(
        &server,
        &user.token,
        "tools/call",
        json!({
            "name": "remember",
            "arguments": { "content": "User likes hiking on weekends" },
        }),
    )
    .send()
    .await
    .unwrap();
    let body: Value = res.json().await.unwrap();
    let mem_id = body["result"]["structuredContent"]["memory_id"]
        .as_str()
        .expect("memory_id")
        .to_string();

    // forget
    let res = jsonrpc_call(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "forget", "arguments": { "memory_id": mem_id } }),
    )
    .send()
    .await
    .unwrap();
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["result"]["structuredContent"]["deleted"], true);
}

#[tokio::test]
async fn test_cross_user_forget_is_404() {
    let server = crate::common::TestServer::start().await;
    let alice = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mcp_alice",
        &["memory::read", "memory::write"],
    )
    .await;
    let bob = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mcp_bob",
        &["memory::read", "memory::write"],
    )
    .await;

    // Alice remembers.
    let res = jsonrpc_call(
        &server,
        &alice.token,
        "tools/call",
        json!({
            "name": "remember",
            "arguments": { "content": "Alice's MCP memory" },
        }),
    )
    .send()
    .await
    .unwrap();
    let body: Value = res.json().await.unwrap();
    let alice_id = body["result"]["structuredContent"]["memory_id"]
        .as_str()
        .unwrap()
        .to_string();

    // Bob tries to forget it.
    let res = jsonrpc_call(
        &server,
        &bob.token,
        "tools/call",
        json!({ "name": "forget", "arguments": { "memory_id": alice_id } }),
    )
    .send()
    .await
    .unwrap();
    let body: Value = res.json().await.unwrap();
    // Must be a JSON-RPC error response — Bob can't delete Alice's row.
    assert!(body["result"].is_null() || body["error"].is_object());
}

#[tokio::test]
async fn test_recall_requires_memory_enabled() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mcp_recall_off",
        &["memory::read", "memory::write"],
    )
    .await;
    // memory_admin_settings.enabled is FALSE by default — recall must
    // refuse with MEMORY_DISABLED error code.
    let res = jsonrpc_call(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "recall", "arguments": { "query": "hiking" } }),
    )
    .send()
    .await
    .unwrap();
    let body: Value = res.json().await.unwrap();
    assert!(
        body["error"].is_object(),
        "recall must error when memory disabled; got: {body}"
    );
}
