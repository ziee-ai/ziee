// Integration tests for the elicitation_mcp built-in MCP server.
//
//   - JSON-RPC surface: initialize / tools/list / tools/call fallback /
//     unknown-method error (the loopback endpoint at /api/elicitation/mcp).
//   - Built-in row upsert idempotency (re-registration keeps ONE row and
//     refreshes the loopback url).

use serde_json::{json, Value};

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;

fn jsonrpc(
    server: &TestServer,
    token: &str,
    method: &str,
    params: Value,
) -> reqwest::RequestBuilder {
    reqwest::Client::new()
        .post(server.api_url("/elicitation/mcp"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
}

#[tokio::test]
async fn test_elicitation_initialize_and_tools_list() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "elic_init", &["mcp_servers::read"]).await;

    let res = jsonrpc(&server, &user.token, "initialize", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["result"]["serverInfo"]["name"], "elicitation");

    let res = jsonrpc(&server, &user.token, "tools/list", json!({}))
        .send()
        .await
        .unwrap();
    let body: Value = res.json().await.unwrap();
    let names: Vec<&str> = body["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"ask_user"), "ask_user must be listed: {names:?}");
}

#[tokio::test]
async fn test_elicitation_tools_call_outside_chat_returns_iserror() {
    // Reaching the loopback tools/call path means ask_user was invoked outside
    // an interactive chat turn (the chat loop intercepts it first). The handler
    // must answer with an isError tool result, NOT a JSON-RPC error.
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "elic_call", &["mcp_servers::read"]).await;

    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "ask_user", "arguments": { "message": "hi" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["result"]["isError"], true, "fallback must be a tool error: {body}");
    assert!(
        body["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .contains("interactive chat turn"),
        "explains why: {body}"
    );
}

#[tokio::test]
async fn test_elicitation_unknown_method_is_method_not_found() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "elic_mnf", &["mcp_servers::read"]).await;
    let res = jsonrpc(&server, &user.token, "does/not/exist", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["error"]["code"], -32601, "method_not_found; got {body}");
}

#[tokio::test]
async fn test_elicitation_requires_mcp_servers_read_permission() {
    let server = TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(&server, "elic_noperm").await;
    let res = jsonrpc(&server, &user.token, "tools/list", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403, "must be permission-gated");
}

#[tokio::test]
async fn test_elicitation_builtin_upsert_is_idempotent() {
    // upsert_builtin_server uses ON CONFLICT (id) DO UPDATE — re-registering the
    // built-in (e.g. on every boot, with a new loopback port) must keep exactly
    // ONE row and refresh its url, never duplicate.
    let server = TestServer::start().await;
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let repo = ziee::elicitation_mcp::ElicitationMcpRepository::new(pool.clone());
    let server_id = ziee::elicitation_mcp::elicitation_mcp_server_id();

    repo.upsert_builtin_server(server_id, "http://127.0.0.1:11111/api/elicitation/mcp")
        .await
        .unwrap();
    repo.upsert_builtin_server(server_id, "http://127.0.0.1:22222/api/elicitation/mcp")
        .await
        .unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mcp_servers WHERE id = $1")
        .bind(server_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "re-registration must not duplicate the built-in row");

    let url: String = sqlx::query_scalar("SELECT url FROM mcp_servers WHERE id = $1")
        .bind(server_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(url, "http://127.0.0.1:22222/api/elicitation/mcp", "url refreshed to latest");
    pool.close().await;
}
