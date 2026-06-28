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
