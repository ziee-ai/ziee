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
    seed_tool_result_ex(server, user_id, tool_use_id, content, structured, false).await
}

/// Like `seed_tool_result` but lets the caller mark the block as an ERROR
/// result (`is_error: true`) — the failure path a failing MCP tool persists.
async fn seed_tool_result_ex(
    server: &TestServer,
    user_id: &str,
    tool_use_id: &str,
    content: &str,
    structured: Option<Value>,
    is_error: bool,
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
        "is_error": is_error,
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
async fn get_tool_result_multi_page_recall_cycle() {
    // A model recalling a large tool result walks it page-by-page: it reads
    // `has_more` + the echoed `offset`, then re-calls with the next offset
    // until `has_more` is false, reassembling the full content. This exercises
    // the FULL offset-based pagination cycle (the existing test only fetched a
    // single window).
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tr_cycle", &[]).await;
    let total = 500usize;
    let big = "A".repeat(total);
    let (conv, _msg) =
        seed_tool_result(&server, &user.user_id, "toolu_cycle", &big, None).await;

    let page_size = 120u64;
    let mut offset = 0u64;
    let mut accumulated = String::new();
    let mut pages = 0;
    loop {
        let res = jsonrpc(
            &server,
            &user.token,
            Some(&conv.to_string()),
            "tools/call",
            json!({ "name": "get_tool_result",
                    "arguments": { "tool_use_id": "toolu_cycle",
                                   "offset": offset, "max_chars": page_size } }),
        )
        .send()
        .await
        .unwrap();
        assert_eq!(res.status(), 200);
        let body: Value = res.json().await.unwrap();
        let sc = &body["result"]["structuredContent"];

        // The page echoes the requested offset + the canonical total.
        assert_eq!(sc["offset"].as_u64().unwrap(), offset, "offset echoed: {body}");
        assert_eq!(sc["total_chars"].as_u64().unwrap(), total as u64);

        let returned = sc["returned_chars"].as_u64().unwrap();
        assert!(returned > 0 && returned <= page_size, "page bounded: {body}");

        // Accumulate the CONTENT (the window before the continuation marker).
        let text = body["result"]["content"][0]["text"].as_str().unwrap();
        let content = text.split("\n[…").next().unwrap_or(text);
        accumulated.push_str(content);

        pages += 1;
        assert!(pages <= 10, "must terminate, not loop forever");

        if sc["has_more"].as_bool().unwrap() {
            // Advance to exactly where this page ended.
            offset += returned;
        } else {
            break;
        }
    }

    // The walk reassembled the entire result and took the expected number of
    // pages (ceil(500/120) = 5).
    assert_eq!(pages, 5, "500 chars / 120 per page = 5 pages");
    assert_eq!(accumulated.chars().count(), total, "full content reassembled");
    assert!(accumulated.chars().all(|c| c == 'A'), "content intact across pages");
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

/// `max_chars` is clamped to the inclusive range [1, 100_000] before paging.
/// This pins BOTH boundaries against a >100k payload: a sub-range request
/// (0, which Serde accepts but the handler clamps up to 1) returns exactly one
/// char, and an over-range request (200_000, clamped down to 100_000) returns
/// exactly 100_000 — neither under- nor over-shoots the clamp.
#[tokio::test]
async fn get_tool_result_clamps_max_chars_to_bounds() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tr_clamp", &[]).await;
    // Larger than the upper clamp so the 100_000 ceiling is observable.
    let big = "A".repeat(100_010);
    let (conv, _msg) =
        seed_tool_result(&server, &user.user_id, "toolu_clamp", &big, None).await;
    let conv = conv.to_string();

    // Lower bound: max_chars=0 clamps UP to 1 → exactly one char, more to come.
    let lo: Value = jsonrpc(
        &server,
        &user.token,
        Some(&conv),
        "tools/call",
        json!({ "name": "get_tool_result",
                "arguments": { "tool_use_id": "toolu_clamp", "offset": 0, "max_chars": 0 } }),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    let lo_sc = &lo["result"]["structuredContent"];
    assert_eq!(lo_sc["returned_chars"], 1, "max_chars=0 clamps up to 1: {lo}");
    assert_eq!(lo_sc["has_more"], true);
    assert_eq!(lo_sc["total_chars"], 100_010);

    // Upper bound: max_chars=200_000 clamps DOWN to 100_000.
    let hi: Value = jsonrpc(
        &server,
        &user.token,
        Some(&conv),
        "tools/call",
        json!({ "name": "get_tool_result",
                "arguments": { "tool_use_id": "toolu_clamp", "offset": 0, "max_chars": 200_000 } }),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    let hi_sc = &hi["result"]["structuredContent"];
    assert_eq!(hi_sc["returned_chars"], 100_000, "max_chars clamps down to 100_000: {hi}");
    assert_eq!(hi_sc["has_more"], true, "100_010 > 100_000 ⇒ more remains");
}

/// An ERROR tool_result block (`is_error: true`) — what a failing MCP tool
/// persists — must STILL be recallable via `get_tool_result`: the handler keys
/// only on `tool_use_id` + `content_type='tool_result'` and must NOT filter out
/// error blocks (otherwise the model can never inspect why a tool failed). This
/// pins recall of the error-result path that `clear_old_tool_results` trims.
#[tokio::test]
async fn get_tool_result_recalls_error_blocks() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tr_error", &[]).await;
    let (conv, _msg) = seed_tool_result_ex(
        &server,
        &user.user_id,
        "toolu_err",
        "Tool execution failed: upstream returned 503",
        Some(json!({ "error": "upstream_unavailable", "status": 503 })),
        true,
    )
    .await;

    let body: Value = jsonrpc(
        &server,
        &user.token,
        Some(&conv.to_string()),
        "tools/call",
        json!({ "name": "get_tool_result", "arguments": { "tool_use_id": "toolu_err" } }),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();

    // The error block is found + recalled (not filtered away as a non-result).
    assert!(body["error"].is_null(), "recall must succeed for an error block: {body}");
    let text = body["result"]["content"][0]["text"].as_str().unwrap_or("");
    assert!(
        text.contains("Tool execution failed: upstream returned 503"),
        "error content must be recalled verbatim: {text}"
    );
    // Its structuredContent (the typed error payload) round-trips too.
    assert!(
        text.contains("upstream_unavailable"),
        "error structuredContent must be recalled: {text}"
    );
}
