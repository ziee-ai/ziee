//! Core (not lit_search-specific) — the `tool_result_mcp` built-in: exact
//! recall of a prior tool_result block via `get_tool_result`, the
//! `structured_content` round-trip (persisted + surfaced on recall, never
//! stripped), char paging, and conversation-ownership scoping. These guard the
//! recall path that every built-in MCP tool (lit_search, web_search, …) relies
//! on once `clear_old_tool_results` trims a sent result.

use serde_json::{json, Value};
use uuid::Uuid;

use crate::common::test_helpers::create_user_with_permissions;
use crate::common::TestServer;

/// JSON-RPC to the tool_result_mcp endpoint, scoped to a conversation.
fn jsonrpc(
    server: &TestServer,
    token: &str,
    conversation_id: Option<&str>,
    method: &str,
    params: Value,
) -> reqwest::RequestBuilder {
    let mut req = reqwest::Client::new()
        .post(server.api_url("/tool-result/mcp"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }));
    if let Some(c) = conversation_id {
        req = req.header("x-conversation-id", c);
    }
    req
}

/// Seed a conversation owned by `user_id` carrying one persisted `tool_result`
/// content block (`tool_use_id`, `content` text, optional `structured_content`).
/// Returns (conversation_id, message_id).
async fn seed_tool_result(
    server: &TestServer,
    user_id: &str,
    tool_use_id: &str,
    content: &str,
    structured: Option<Value>,
) -> (Uuid, Uuid) {
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let uid = Uuid::parse_str(user_id).unwrap();
    let conv_id = Uuid::new_v4();
    let branch_id = Uuid::new_v4();
    let msg_id = Uuid::new_v4();

    sqlx::query(
        r#"INSERT INTO conversations (id, user_id, title, active_branch_id, created_at, updated_at)
           VALUES ($1, $2, 'tr', NULL, NOW(), NOW())"#,
    )
    .bind(conv_id)
    .bind(uid)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO branches (id, conversation_id, parent_branch_id, created_from_message_id, created_at)
           VALUES ($1, $2, NULL, NULL, NOW())"#,
    )
    .bind(branch_id)
    .bind(conv_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE conversations SET active_branch_id = $1 WHERE id = $2")
        .bind(branch_id)
        .bind(conv_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        r#"INSERT INTO messages (id, role, originated_from_id, created_at)
           VALUES ($1, 'assistant', $1, NOW())"#,
    )
    .bind(msg_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO branch_messages (branch_id, message_id, created_at)
           VALUES ($1, $2, NOW())"#,
    )
    .bind(branch_id)
    .bind(msg_id)
    .execute(&pool)
    .await
    .unwrap();

    let mut block = json!({
        "type": "tool_result",
        "tool_use_id": tool_use_id,
        "name": "literature_search",
        "content": content,
        "is_error": false,
    });
    if let Some(sc) = structured {
        block["structured_content"] = sc;
    }
    sqlx::query(
        r#"INSERT INTO message_contents (id, message_id, content_type, content, sequence_order, created_at, updated_at)
           VALUES (gen_random_uuid(), $1, 'tool_result', $2, 0, NOW(), NOW())"#,
    )
    .bind(msg_id)
    .bind(&block)
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;
    (conv_id, msg_id)
}

#[tokio::test]
async fn jsonrpc_lifecycle_initialize_tools_list_ping() {
    // The MCP protocol handshake the chat client performs against the built-in
    // tool_result server before any tools/call: initialize → tools/list → ping.
    // Guards handlers.rs:57-75 (none of the prior tests exercise the lifecycle).
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tr_lifecycle", &[]).await;

    // initialize — protocolVersion + serverInfo.name come straight from the real
    // handler (no conversation header needed for the lifecycle methods).
    let res = jsonrpc(&server, &user.token, None, "initialize", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(body["id"], 1);
    assert_eq!(body["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(body["result"]["serverInfo"]["name"], "tool_result");
    assert!(
        body["result"]["capabilities"]["tools"].is_object(),
        "initialize must advertise the tools capability: {body}"
    );

    // tools/list — the single get_tool_result tool with its inputSchema.
    let res = jsonrpc(&server, &user.token, None, "tools/list", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    let tools = body["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 1, "exactly one tool: {body}");
    assert_eq!(tools[0]["name"], "get_tool_result");
    assert!(
        tools[0]["inputSchema"].is_object(),
        "get_tool_result must expose an inputSchema: {body}"
    );

    // ping — an empty/ok result, no error.
    let res = jsonrpc(&server, &user.token, None, "ping", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert!(body["result"].is_object(), "ping returns an ok result: {body}");
    assert!(body["error"].is_null(), "ping must not error: {body}");

    // An unknown method is a JSON-RPC method-not-found error (still HTTP 200).
    let res = jsonrpc(&server, &user.token, None, "does/not/exist", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert!(
        body["error"].is_object(),
        "unknown method must return a JSON-RPC error: {body}"
    );
}

#[tokio::test]
async fn get_tool_result_recalls_content_and_structured_content() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tr_recall", &[]).await;
    let (conv, _msg) = seed_tool_result(
        &server,
        &user.user_id,
        "toolu_recall1",
        "Literature search digest: 3 records after dedup.",
        Some(json!({ "after_dedup": 3, "records": [{ "doi": "10.1/x", "title": "Recalled Paper" }] })),
    )
    .await;

    let res = jsonrpc(
        &server,
        &user.token,
        Some(&conv.to_string()),
        "tools/call",
        json!({ "name": "get_tool_result", "arguments": { "tool_use_id": "toolu_recall1" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    let text = body["result"]["content"][0]["text"].as_str().unwrap_or("");
    // The persisted content text comes back verbatim ...
    assert!(text.contains("digest: 3 records after dedup"), "recall text: {text}");
    // ... AND the structured_content is surfaced (the ONLY model-readable path).
    assert!(text.contains("--- structuredContent ---"), "recall must include structuredContent: {text}");
    assert!(text.contains("Recalled Paper"), "structuredContent fields recalled: {text}");

    let sc = &body["result"]["structuredContent"];
    assert!(sc["total_chars"].as_u64().unwrap_or(0) > 0);
    assert_eq!(sc["has_more"], false);
    assert_eq!(sc["tool_use_id"], "toolu_recall1");
}

#[tokio::test]
async fn get_tool_result_recalls_web_search_result() {
    // Cross-subsystem: web_search emits a readable text digest PLUS a typed
    // `structuredContent` ({provider, results:[{url,title,snippet}]}); once
    // `clear_old_tool_results` trims the sent block, the model's ONLY way back to
    // it is `tool_result_mcp::get_tool_result`. This seeds a persisted
    // web_search-shaped result and proves both the digest text and the typed
    // provider/results payload round-trip verbatim through recall — the exact
    // path web_search depends on (the block's `name` is irrelevant to recall,
    // which is keyed by tool_use_id within the conversation).
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tr_websearch", &[]).await;

    // Mirror tests/web_search/mcp_test.rs's real structuredContent shape.
    let structured = json!({
        "provider": "searxng",
        "results": [
            { "url": "https://example.com/a", "title": "Example Result", "snippet": "a snippet about the query" },
            { "url": "https://example.com/b", "title": "Second Hit", "snippet": "more context here" }
        ]
    });
    let (conv, _msg) = seed_tool_result(
        &server,
        &user.user_id,
        "toolu_websearch1",
        "Web search via searxng: 2 results for \"rust async\".",
        Some(structured),
    )
    .await;

    let res = jsonrpc(
        &server,
        &user.token,
        Some(&conv.to_string()),
        "tools/call",
        json!({ "name": "get_tool_result", "arguments": { "tool_use_id": "toolu_websearch1" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();

    let text = body["result"]["content"][0]["text"].as_str().unwrap_or("");
    // The readable digest comes back verbatim ...
    assert!(text.contains("Web search via searxng: 2 results"), "recall text: {text}");
    // ... AND the typed web_search structuredContent is surfaced on recall
    // (provider + per-hit url/title/snippet), never stripped.
    assert!(text.contains("--- structuredContent ---"), "recall must include structuredContent: {text}");
    assert!(text.contains("searxng"), "provider recalled: {text}");
    assert!(text.contains("https://example.com/a"), "result url recalled: {text}");
    assert!(text.contains("Example Result"), "result title recalled: {text}");
    assert!(text.contains("a snippet about the query"), "result snippet recalled: {text}");

    let sc = &body["result"]["structuredContent"];
    assert_eq!(sc["tool_use_id"], "toolu_websearch1");
    assert!(sc["total_chars"].as_u64().unwrap_or(0) > 0);
    assert_eq!(sc["has_more"], false);
}

#[tokio::test]
async fn get_tool_result_pages_large_content() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tr_page", &[]).await;
    let big = "A".repeat(500);
    let (conv, _msg) =
        seed_tool_result(&server, &user.user_id, "toolu_big", &big, None).await;

    let res = jsonrpc(
        &server,
        &user.token,
        Some(&conv.to_string()),
        "tools/call",
        json!({ "name": "get_tool_result",
                "arguments": { "tool_use_id": "toolu_big", "offset": 0, "max_chars": 50 } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    let sc = &body["result"]["structuredContent"];
    assert_eq!(sc["returned_chars"], 50, "must page to max_chars: {body}");
    assert_eq!(sc["has_more"], true);
    assert_eq!(sc["total_chars"], 500);
    let text = body["result"]["content"][0]["text"].as_str().unwrap_or("");
    assert!(text.contains("more available"), "paging marker present: {text}");
}

#[tokio::test]
async fn get_tool_result_rejects_other_users_conversation() {
    // Block lives in user A's conversation; user B passing A's conversation id
    // must get NOT_FOUND (scoping — can't read another user's tool results).
    let server = TestServer::start().await;
    let alice = create_user_with_permissions(&server, "tr_alice", &[]).await;
    let bob = create_user_with_permissions(&server, "tr_bob", &[]).await;
    let (conv, _msg) =
        seed_tool_result(&server, &alice.user_id, "toolu_a", "alice's secret digest", None).await;

    let res = jsonrpc(
        &server,
        &bob.token,
        Some(&conv.to_string()),
        "tools/call",
        json!({ "name": "get_tool_result", "arguments": { "tool_use_id": "toolu_a" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert!(
        body["error"]["message"].as_str().unwrap_or("").to_lowercase().contains("no such")
            || body["error"]["message"].as_str().unwrap_or("").to_lowercase().contains("not"),
        "cross-user recall must be NOT_FOUND: {body}"
    );
    // And the secret content must not leak in any field.
    assert!(!serde_json::to_string(&body).unwrap().contains("secret digest"));
}

#[tokio::test]
async fn get_tool_result_requires_conversation_header() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tr_nohdr", &[]).await;
    let (_conv, _msg) =
        seed_tool_result(&server, &user.user_id, "toolu_h", "content", None).await;

    // No x-conversation-id → NO_CONVERSATION.
    let res = jsonrpc(
        &server,
        &user.token,
        None,
        "tools/call",
        json!({ "name": "get_tool_result", "arguments": { "tool_use_id": "toolu_h" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert!(body["error"].is_object(), "missing conversation header must error: {body}");
}

#[tokio::test]
async fn structured_content_persists_on_block() {
    // Guards the core persistence change: structured_content stored on a
    // tool_result block round-trips through JSONB (read back from the DB). The
    // hidden_content-stripping-on-serialize behavior is enforced structurally by
    // the `#[serde(serialize_with = "strip_hidden_content_serialize")]` attribute
    // on MessageContentData (chat/core/models/content.rs), not re-asserted here.
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tr_persist", &[]).await;
    let (_conv, msg) = seed_tool_result(
        &server,
        &user.user_id,
        "toolu_persist",
        "digest text",
        Some(json!({ "records": [{ "doi": "10.1/persisted" }] })),
    )
    .await;

    let contents = crate::chat::helpers::get_message_contents_from_db(&server, msg).await;
    let block = contents
        .iter()
        .find(|c| c["content_type"] == "tool_result")
        .expect("tool_result block persisted");
    assert_eq!(
        block["content"]["structured_content"]["records"][0]["doi"],
        "10.1/persisted",
        "structured_content must persist on the block: {block}"
    );
}

#[tokio::test]
async fn get_tool_result_is_branch_agnostic() {
    // Recall is conversation-scoped but branch-AGNOSTIC: a tool_result persisted
    // on one branch must still be recallable after the conversation's
    // active_branch_id is switched to a DIFFERENT (empty) branch.
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tr_branch", &[]).await;
    let (conv, _msg) = seed_tool_result(
        &server,
        &user.user_id,
        "toolu_branch1",
        "Result lives on the original branch only.",
        None,
    )
    .await;

    // Add a second, empty branch and make IT the active one.
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let other_branch = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO branches (id, conversation_id, parent_branch_id, created_from_message_id, created_at)
           VALUES ($1, $2, NULL, NULL, NOW())"#,
    )
    .bind(other_branch)
    .bind(conv)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE conversations SET active_branch_id = $1 WHERE id = $2")
        .bind(other_branch)
        .bind(conv)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    // The active branch now has NO tool_result, but recall must still find it.
    let res = jsonrpc(
        &server,
        &user.token,
        Some(&conv.to_string()),
        "tools/call",
        json!({ "name": "get_tool_result", "arguments": { "tool_use_id": "toolu_branch1" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    let text = body["result"]["content"][0]["text"].as_str().unwrap_or("");
    assert!(
        text.contains("Result lives on the original branch only."),
        "recall must be branch-agnostic (found on non-active branch): {text}"
    );
    assert_eq!(body["result"]["structuredContent"]["tool_use_id"], "toolu_branch1");
}

// audit id all-53eae2bd53da — the JSON-RPC lifecycle (initialize / tools/list /
// ping) of the tool_result_mcp endpoint was untested; only tools/call paths
// were. These need no conversation header.
#[tokio::test]
async fn tool_result_mcp_jsonrpc_lifecycle() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tr_lifecycle", &["mcp_servers::read"]).await;

    // initialize
    let init: Value = jsonrpc(&server, &user.token, None, "initialize", json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(init["result"]["serverInfo"]["name"], "tool_result", "initialize: {init}");
    assert!(init["result"]["protocolVersion"].is_string(), "initialize: {init}");

    // tools/list exposes get_tool_result
    let list: Value = jsonrpc(&server, &user.token, None, "tools/list", json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(names.contains(&"get_tool_result"), "tools/list: {names:?}");

    // ping → {}
    let ping: Value = jsonrpc(&server, &user.token, None, "ping", json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(ping["error"].is_null(), "ping must succeed: {ping}");
    assert_eq!(ping["result"], json!({}), "ping returns empty result: {ping}");
}

// audit id all-9961accefc1c — the endpoint is gated by McpServersRead; an
// unauthenticated request must 401 and a user lacking mcp_servers::read must
// 403. Neither was asserted.
#[tokio::test]
async fn tool_result_mcp_requires_auth_and_permission() {
    let server = TestServer::start().await;

    // No Authorization header → 401.
    let unauth = reqwest::Client::new()
        .post(server.api_url("/tool-result/mcp"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} }))
        .send()
        .await
        .unwrap();
    assert_eq!(unauth.status(), 401, "no token must be 401");

    // A user WITHOUT mcp_servers::read (default group removed) → 403.
    let noperm = crate::common::test_helpers::create_user_with_only_permissions(
        &server,
        "tr_noperm",
        &["profile::read"],
    )
    .await;
    let forbidden = jsonrpc(&server, &noperm.token, None, "tools/list", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden.status(), 403, "missing mcp_servers::read must be 403");
}

// audit id all-71ce2a6bda0c — recall of a tool_result that carries attached
// resource_links. A file-producing tool (e.g. code_sandbox get_resource_link,
// citations export) stores resource_links inside the tool_result's
// structuredContent; get_tool_result must surface them so the model can recover
// the file references after the result is cleared/truncated.
#[tokio::test]
async fn get_tool_result_recalls_attached_resource_links() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tr_reslink", &[]).await;
    let (conv, _msg) = seed_tool_result(
        &server,
        &user.user_id,
        "toolu_reslink",
        "Generated 1 chart.",
        Some(json!({
            "resource_links": [
                { "uri": "/api/files/abc123/download", "name": "chart.png", "mimeType": "image/png", "is_saved": true }
            ]
        })),
    )
    .await;

    let body: Value = jsonrpc(
        &server,
        &user.token,
        Some(&conv.to_string()),
        "tools/call",
        json!({ "name": "get_tool_result", "arguments": { "tool_use_id": "toolu_reslink" } }),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    let text = body["result"]["content"][0]["text"].as_str().unwrap_or("");
    assert!(text.contains("--- structuredContent ---"), "recall must include structuredContent: {text}");
    assert!(
        text.contains("/api/files/abc123/download"),
        "the attached resource_link URI must be recalled: {text}"
    );
    assert!(text.contains("chart.png"), "the resource_link name must be recalled: {text}");
#[tokio::test]
async fn endpoint_rejects_missing_jwt_with_401() {
    // The handler is gated by `RequirePermissions<(McpServersRead,)>`, whose
    // extractor 401s before any tool dispatch when no Authorization header is
    // present (MISSING_TOKEN). A POST with a well-formed JSON-RPC body but no
    // bearer token must never reach `get_tool_result`.
    let server = TestServer::start().await;

    let res = reqwest::Client::new()
        .post(server.api_url("/tool-result/mcp"))
        .header("x-conversation-id", Uuid::new_v4().to_string())
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": "get_tool_result", "arguments": { "tool_use_id": "toolu_x" } }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.status(),
        401,
        "no Authorization header must be rejected with 401 UNAUTHORIZED"
    );
}

#[tokio::test]
async fn endpoint_rejects_user_without_mcp_read_with_403() {
    // The route requires `mcp_servers::read`. A freshly-registered user holds it
    // via the default group (that's why every other test in this file passes a
    // `&[]` user and still gets 200). Permissions are resolved LIVE from the DB
    // on each request (extractors.rs → `get_user_groups`), so stripping the
    // user's group memberships after registration leaves them authenticated but
    // unauthorized — the gate must answer 403, not 200/401.
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tr_noperm", &[]).await;

    // Drop every group membership → user now has zero permissions.
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let uid = Uuid::parse_str(&user.user_id).unwrap();
    sqlx::query("DELETE FROM user_groups WHERE user_id = $1")
        .bind(uid)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    // Sanity: the token itself is still valid (a permission-bearing user would
    // get 200 here); only the live permission check should fail.
    let res = jsonrpc(
        &server,
        &user.token,
        Some(&Uuid::new_v4().to_string()),
        "tools/call",
        json!({ "name": "get_tool_result", "arguments": { "tool_use_id": "toolu_x" } }),
    )
    .send()
    .await
    .unwrap();

    assert_eq!(
        res.status(),
        403,
        "an authenticated user lacking mcp_servers::read must be rejected with 403 FORBIDDEN"
    );
}

// ---------------------------------------------------------------------------
// Malformed-request handling (handlers.rs:30-48 + the tools/call param/arg
// validation). The endpoint must answer every malformed shape with a graceful
// JSON-RPC error (the spec codes) and NEVER a 5xx / panic. The lifecycle test
// above covers a well-formed unknown METHOD (-32601); these cover broken
// envelopes and bad params/arguments, which no prior test exercises.
// ---------------------------------------------------------------------------

/// A body that isn't valid JSON at all → HTTP 400 + JSON-RPC parse error
/// (-32700), id null. The auth gate runs first, so a valid token is required to
/// reach the parse branch — this asserts the parse branch itself, not the gate.
#[tokio::test]
async fn malformed_body_invalid_json_returns_parse_error() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tr_badjson", &[]).await;

    let res = reqwest::Client::new()
        .post(server.api_url("/tool-result/mcp"))
        .header("Authorization", format!("Bearer {}", user.token))
        .header("content-type", "application/json")
        .header("x-conversation-id", Uuid::new_v4().to_string())
        .body("{ this is : not json")
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.status(),
        400,
        "non-JSON body must be a 400, never a 5xx/panic"
    );
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["jsonrpc"], "2.0");
    assert!(body["id"].is_null(), "parse error carries a null id: {body}");
    assert_eq!(
        body["error"]["code"], -32700,
        "invalid JSON must be JSON-RPC parse error -32700: {body}"
    );
}

/// Valid JSON that is NOT a JSON-RPC request object (no `method` field) → HTTP
/// 400 + invalid-request (-32600).
#[tokio::test]
async fn valid_json_missing_method_returns_invalid_request() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tr_nomethod", &[]).await;

    // Has jsonrpc + id but no `method` → JsonRpcRequest deserialization fails.
    let res = reqwest::Client::new()
        .post(server.api_url("/tool-result/mcp"))
        .header("Authorization", format!("Bearer {}", user.token))
        .header("x-conversation-id", Uuid::new_v4().to_string())
        .json(&json!({ "jsonrpc": "2.0", "id": 1 }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.status(),
        400,
        "a JSON body missing `method` must be a 400, never a 5xx"
    );
    let body: Value = res.json().await.unwrap();
    assert_eq!(
        body["error"]["code"], -32600,
        "non-JSON-RPC object must be invalid-request -32600: {body}"
    );
}

/// `tools/call` whose params object lacks the required `name` → HTTP 200 +
/// invalid-params (-32602). (tools/call errors are JSON-RPC errors at HTTP 200.)
#[tokio::test]
async fn tools_call_missing_name_returns_invalid_params() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tr_noname", &[]).await;

    // params has `arguments` but no `name` → ToolCallParams::deserialize fails.
    let res = jsonrpc(
        &server,
        &user.token,
        Some(&Uuid::new_v4().to_string()),
        "tools/call",
        json!({ "arguments": { "tool_use_id": "toolu_x" } }),
    )
    .send()
    .await
    .unwrap();

    assert_eq!(res.status(), 200, "tools/call param errors are HTTP 200");
    let body: Value = res.json().await.unwrap();
    assert!(body["result"].is_null(), "no result on a param error: {body}");
    assert_eq!(
        body["error"]["code"], -32602,
        "missing `name` must be invalid-params -32602: {body}"
    );
}

/// `tools/call get_tool_result` with NO `tool_use_id` argument → a graceful
/// JSON-RPC error (never a 5xx/panic). The required-arg validation lives in
/// `get_tool_result` and surfaces through `from_app_error`.
#[tokio::test]
async fn tools_call_missing_required_arg_errors_not_5xx() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tr_noarg", &[]).await;

    let res = jsonrpc(
        &server,
        &user.token,
        Some(&Uuid::new_v4().to_string()),
        "tools/call",
        json!({ "name": "get_tool_result", "arguments": {} }),
    )
    .send()
    .await
    .unwrap();

    assert!(
        !res.status().is_server_error(),
        "a missing required arg must never 5xx: got {}",
        res.status()
    );
    let body: Value = res.json().await.unwrap();
    assert!(
        body["error"].is_object(),
        "missing tool_use_id must return a JSON-RPC error, not a result: {body}"
    );
    assert!(body["result"].is_null(), "no result on an arg error: {body}");
}

/// `tools/call get_tool_result` with a WRONG-TYPED argument (tool_use_id as a
/// number, offset as a string) → graceful JSON-RPC error, never a 5xx/panic.
#[tokio::test]
async fn tools_call_wrong_typed_arg_errors_not_5xx() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tr_badtype", &[]).await;

    let res = jsonrpc(
        &server,
        &user.token,
        Some(&Uuid::new_v4().to_string()),
        "tools/call",
        json!({ "name": "get_tool_result", "arguments": { "tool_use_id": 12345, "offset": "nope" } }),
    )
    .send()
    .await
    .unwrap();

    assert!(
        !res.status().is_server_error(),
        "a wrong-typed arg must never 5xx: got {}",
        res.status()
    );
    let body: Value = res.json().await.unwrap();
    assert!(
        body["error"].is_object(),
        "wrong-typed args must return a JSON-RPC error: {body}"
    );
}

/// `max_chars` clamping boundaries (handlers.rs:232 — `.clamp(1, 100_000)`).
/// The clamp is an inline expression, only observable through the handler, so
/// this seeds a content larger than the upper bound and drives `get_tool_result`
/// with the boundary + out-of-range values, asserting `returned_chars` reflects
/// the clamp (lower → 1, upper → 100_000) and that out-of-range values are
/// clamped rather than erroring.
#[tokio::test]
async fn get_tool_result_clamps_max_chars_to_1_and_100000() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tr_clamp", &[]).await;
    // Bigger than the 100_000 upper clamp so the upper boundary is observable.
    let big = "A".repeat(120_000);
    let (conv, _msg) =
        seed_tool_result(&server, &user.user_id, "toolu_clamp", &big, None).await;

    // Helper: call get_tool_result with a given max_chars and return
    // (returned_chars, has_more).
    async fn returned(server: &TestServer, token: &str, conv: &str, max_chars: i64) -> (i64, bool) {
        let res = jsonrpc(
            server,
            token,
            Some(conv),
            "tools/call",
            json!({ "name": "get_tool_result",
                    "arguments": { "tool_use_id": "toolu_clamp", "offset": 0, "max_chars": max_chars } }),
        )
        .send()
        .await
        .unwrap();
        assert_eq!(res.status(), 200, "clamp call must succeed (max_chars={max_chars})");
        let body: Value = res.json().await.unwrap();
        let sc = &body["result"]["structuredContent"];
        assert_eq!(sc["total_chars"], 120_000, "total preserved: {body}");
        (sc["returned_chars"].as_i64().unwrap(), sc["has_more"].as_bool().unwrap())
    }

    // Lower boundary: max_chars=1 → exactly 1 char returned, more remains.
    assert_eq!(returned(&server, &user.token, &conv.to_string(), 1).await, (1, true));
    // Below the lower bound: max_chars=0 → clamped UP to 1 (not an empty page,
    // not an error).
    assert_eq!(returned(&server, &user.token, &conv.to_string(), 0).await, (1, true));
    // Upper boundary: max_chars=100_000 → exactly the cap, more remains
    // (120_000 total).
    assert_eq!(returned(&server, &user.token, &conv.to_string(), 100_000).await, (100_000, true));
    // Above the upper bound: max_chars=200_000 → clamped DOWN to 100_000 (no
    // error, does not return all 120_000).
    assert_eq!(returned(&server, &user.token, &conv.to_string(), 200_000).await, (100_000, true));
}
