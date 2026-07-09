//! Message operation integration tests

use reqwest::StatusCode;
use serde_json::json;

// =====================================================
// Get Conversation History Tests
// =====================================================

#[tokio::test]
async fn test_get_conversation_history_empty() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "messages::read"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{}/messages", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let messages: Vec<serde_json::Value> = response.json().await.unwrap();

    assert_eq!(messages.len(), 0, "New conversation should have no messages");
}

#[tokio::test]
async fn test_get_conversation_history_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["messages::read"],
    )
    .await;

    let fake_id = uuid::Uuid::new_v4();

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{}/messages", fake_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =====================================================
// Get Message Tests
// =====================================================

#[tokio::test]
async fn test_get_message_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["messages::read"],
    )
    .await;

    let fake_id = uuid::Uuid::new_v4();

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/messages/{}", fake_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_message_invalid_uuid() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["messages::read"],
    )
    .await;

    let response = reqwest::Client::new()
        .get(server.api_url("/messages/not-a-uuid"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// =====================================================
// Edit Message Tests
// =====================================================

#[tokio::test]
async fn test_edit_message_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["messages::create"],
    )
    .await;

    let fake_conversation_id = uuid::Uuid::new_v4();
    let fake_message_id = uuid::Uuid::new_v4();

    let payload = json!({
        "content": "Edited content"
    });

    let response = reqwest::Client::new()
        .put(server.api_url(&format!(
            "/conversations/{}/messages/{}",
            fake_conversation_id, fake_message_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_edit_message_empty_content() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "messages::create"],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // Use a fake message ID
    let fake_message_id = uuid::Uuid::new_v4();

    let payload = json!({
        "content": ""
    });

    let response = reqwest::Client::new()
        .put(server.api_url(&format!(
            "/conversations/{}/messages/{}",
            conversation_id, fake_message_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // Should fail validation before trying to find the message
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// =====================================================
// Delete Message Tests
// =====================================================

#[tokio::test]
async fn test_delete_message_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["messages::delete"],
    )
    .await;

    let fake_id = uuid::Uuid::new_v4();

    let response = reqwest::Client::new()
        .delete(server.api_url(&format!("/messages/{}", fake_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =====================================================
// Send Message Validation Tests
// =====================================================

#[tokio::test]
async fn test_send_message_empty_content_accepted_for_tool_only_calls() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let (_stub, model) = super::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        model_id,
        branch_id,
        "",
    )
    .await;

    // Empty content is now accepted by design: tool-only calls (a
    // model that issues only `tool_use` blocks with no preceding text)
    // are valid in modern LLM APIs. The fire-and-forget endpoint
    // returns 200 + `{user_message_id, assistant_message_id}`; the reply
    // itself streams over `GET /api/chat/stream`. Previously this
    // returned 400; the validation was removed when tool-only chats
    // became first-class.
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_send_message_invalid_branch_id() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    let (_stub, model) = super::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    let fake_branch_id = uuid::Uuid::new_v4();

    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        model_id,
        fake_branch_id,
        "Test message",
    )
    .await;

    // Should return 404 for non-existent branch
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_send_message_invalid_model_id() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let fake_model_id = uuid::Uuid::new_v4();

    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        fake_model_id,
        branch_id,
        "Test message",
    )
    .await;

    // Should return 404 for non-existent model
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_send_message_conversation_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["messages::create"],
    )
    .await;

    let fake_conversation_id = uuid::Uuid::new_v4();
    let fake_model_id = uuid::Uuid::new_v4();
    let fake_branch_id = uuid::Uuid::new_v4();

    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        fake_conversation_id,
        fake_model_id,
        fake_branch_id,
        "Test message",
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_send_message_returns_message_ids() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "conversations::create",
            "messages::create",
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    let conversation = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let (_stub, model) = super::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        model_id,
        branch_id,
        "Hello, world!",
    )
    .await;

    // Fire-and-forget: POST returns 200 + JSON `{user_message_id,
    // assistant_message_id}` immediately (NO body stream). The reply
    // itself streams over `GET /api/chat/stream`.
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(
        !body["assistant_message_id"].is_null(),
        "response body must carry a non-null assistant_message_id; got {body}"
    );
}

// =====================================================
// Lazy-load pagination + in-conversation search (feature: lazy-load-conversation-messages)
// =====================================================

use uuid::Uuid;

/// Seed `count` messages into a branch with STRICTLY INCREASING junction
/// `created_at` (so branch order is deterministic, independent of NOW()
/// ties). Returns the message ids in chronological order. Text is
/// `"seeded message {i}"`.
pub(crate) async fn seed_ordered_messages(
    database_url: &str,
    branch_id: Uuid,
    count: usize,
) -> Vec<Uuid> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(database_url)
        .await
        .expect("connect");
    let mut ids = Vec::with_capacity(count);
    for i in 0..count {
        let role = if i % 2 == 0 { "user" } else { "assistant" };
        // created_at strictly increasing by 1s per message.
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO messages (role, originated_from_id, edit_count, created_at)
             VALUES ($1, gen_random_uuid(), 0, NOW() + ($2 || ' seconds')::interval)
             RETURNING id",
        )
        .bind(role)
        .bind(i.to_string())
        .fetch_one(&pool)
        .await
        .expect("insert message");
        sqlx::query("UPDATE messages SET originated_from_id = id WHERE id = $1")
            .bind(id)
            .execute(&pool)
            .await
            .expect("origin");
        let content = json!({ "type": "text", "text": format!("seeded message {i}") });
        sqlx::query(
            "INSERT INTO message_contents (message_id, content_type, content, sequence_order)
             VALUES ($1, 'text', $2, 0)",
        )
        .bind(id)
        .bind(&content)
        .execute(&pool)
        .await
        .expect("content");
        // Junction created_at = the message's created_at → deterministic order.
        sqlx::query(
            "INSERT INTO branch_messages (branch_id, message_id, is_clone, created_at)
             VALUES ($1, $2, false, (SELECT created_at FROM messages WHERE id = $2))",
        )
        .bind(branch_id)
        .bind(id)
        .execute(&pool)
        .await
        .expect("branch_messages");
        ids.push(id);
    }
    pool.close().await;
    ids
}

pub(crate) async fn get_history(
    server: &crate::common::TestServer,
    token: &str,
    conversation_id: Uuid,
    query: &str,
) -> (StatusCode, serde_json::Value) {
    let url = server.api_url(&format!(
        "/conversations/{}/messages{}",
        conversation_id, query
    ));
    let resp = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let body = if status == StatusCode::OK {
        resp.json().await.unwrap()
    } else {
        serde_json::Value::Null
    };
    (status, body)
}

pub(crate) fn msg_ids(body: &serde_json::Value) -> Vec<Uuid> {
    body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| super::helpers::parse_uuid(&m["id"]))
        .collect()
}

/// TEST-4: tail + before keyset pagination reconstructs the whole branch exactly
/// once, with correct has_more_* flags.
#[tokio::test]
async fn test_history_pagination_tail_and_before() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "messages::read"],
    )
    .await;
    let conv = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conv_id = super::helpers::parse_uuid(&conv["id"]);
    let branch_id = super::helpers::parse_uuid(&conv["active_branch_id"]);

    let all = seed_ordered_messages(&server.database_url, branch_id, 25).await;

    // Tail: newest 10, ASC, older exist, no newer.
    let (st, body) = get_history(&server, &user.token, conv_id, "?limit=10").await;
    assert_eq!(st, StatusCode::OK);
    let tail = msg_ids(&body);
    assert_eq!(tail.len(), 10);
    assert_eq!(tail, all[15..25].to_vec(), "tail is the newest 10 in ASC order");
    assert_eq!(body["has_more_before"], serde_json::json!(true));
    assert_eq!(body["has_more_after"], serde_json::json!(false));

    // before = oldest-in-tail → next 10 older.
    let (_st, body2) = get_history(
        &server,
        &user.token,
        conv_id,
        &format!("?limit=10&before={}", all[15]),
    )
    .await;
    let page2 = msg_ids(&body2);
    assert_eq!(page2, all[5..15].to_vec());
    assert_eq!(body2["has_more_before"], serde_json::json!(true));
    assert_eq!(body2["has_more_after"], serde_json::json!(true));

    // before = oldest-so-far → final 5, no more older.
    let (_st, body3) = get_history(
        &server,
        &user.token,
        conv_id,
        &format!("?limit=10&before={}", all[5]),
    )
    .await;
    let page3 = msg_ids(&body3);
    assert_eq!(page3, all[0..5].to_vec());
    assert_eq!(body3["has_more_before"], serde_json::json!(false));

    // Union of the three pages == all 25, no dup / no skip.
    let mut union: Vec<Uuid> = tail;
    union.extend(page2);
    union.extend(page3);
    union.sort();
    let mut expected = all.clone();
    expected.sort();
    assert_eq!(union, expected, "pages reconstruct the full branch exactly once");
}

/// TEST-5: around centers a window on a message; after pages the newer side.
#[tokio::test]
async fn test_history_pagination_around_and_after() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "messages::read"],
    )
    .await;
    let conv = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conv_id = super::helpers::parse_uuid(&conv["id"]);
    let branch_id = super::helpers::parse_uuid(&conv["active_branch_id"]);
    let all = seed_ordered_messages(&server.database_url, branch_id, 25).await;

    // around a middle message with limit 10 → 5 older + target + 5 newer.
    let (st, body) = get_history(
        &server,
        &user.token,
        conv_id,
        &format!("?limit=10&around={}", all[12]),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let win = msg_ids(&body);
    assert_eq!(win, all[7..18].to_vec(), "centered window all[7..=17]");
    assert!(win.contains(&all[12]), "window contains the target");
    assert_eq!(body["has_more_before"], serde_json::json!(true));
    assert_eq!(body["has_more_after"], serde_json::json!(true));

    // after the newest-in-window → the remaining newer messages, no more after.
    let (_st, body2) = get_history(
        &server,
        &user.token,
        conv_id,
        &format!("?limit=10&after={}", all[17]),
    )
    .await;
    let newer = msg_ids(&body2);
    assert_eq!(newer, all[18..25].to_vec());
    assert_eq!(body2["has_more_after"], serde_json::json!(false));
    assert_eq!(body2["has_more_before"], serde_json::json!(true));
}

/// TEST-6: validation + error paths.
#[tokio::test]
async fn test_history_pagination_validation_and_errors() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "messages::read"],
    )
    .await;
    let conv = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conv_id = super::helpers::parse_uuid(&conv["id"]);

    // Empty conversation → empty window, both flags false.
    let (st, body) = get_history(&server, &user.token, conv_id, "").await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body["messages"].as_array().unwrap().len(), 0);
    assert_eq!(body["has_more_before"], serde_json::json!(false));
    assert_eq!(body["has_more_after"], serde_json::json!(false));

    // Unknown before cursor → 404.
    let fake = Uuid::new_v4();
    let (st, _) = get_history(
        &server,
        &user.token,
        conv_id,
        &format!("?before={}", fake),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND, "unknown cursor → 404");

    // Two cursors set → 400.
    let (st, _) = get_history(
        &server,
        &user.token,
        conv_id,
        &format!("?before={}&around={}", fake, fake),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "two cursors → 400");
}

/// TEST-8: each windowed message carries its own content blocks (batch load,
/// no cross-message bleed).
#[tokio::test]
async fn test_history_window_content_association() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "messages::read"],
    )
    .await;
    let conv = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conv_id = super::helpers::parse_uuid(&conv["id"]);
    let branch_id = super::helpers::parse_uuid(&conv["active_branch_id"]);
    let all = seed_ordered_messages(&server.database_url, branch_id, 6).await;

    let (st, body) = get_history(&server, &user.token, conv_id, "?limit=30").await;
    assert_eq!(st, StatusCode::OK);
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 6);
    for (i, m) in messages.iter().enumerate() {
        assert_eq!(super::helpers::parse_uuid(&m["id"]), all[i]);
        let contents = m["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 1, "each message has its one seeded block");
        assert_eq!(
            contents[0]["content"]["text"],
            serde_json::json!(format!("seeded message {i}")),
            "content text belongs to the right message (no bleed)"
        );
    }
}

/// TEST-15: in-conversation search — server-side over the whole branch,
/// paginated, snippet, ordinal continuity, LIKE-escaping, owner scope.
#[tokio::test]
async fn test_in_conversation_search() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "messages::read"],
    )
    .await;
    let conv = super::helpers::create_conversation(&server, &user.token, None, None).await;
    let conv_id = super::helpers::parse_uuid(&conv["id"]);
    let branch_id = super::helpers::parse_uuid(&conv["active_branch_id"]);

    // 5 messages contain "refund", plus one literal "50%" and unrelated noise.
    for i in 0..5 {
        super::helpers::seed_text_message(
            &server.database_url,
            branch_id,
            "user",
            &format!("please issue a refund for order {i}"),
        )
        .await;
        super::helpers::seed_text_message(
            &server.database_url,
            branch_id,
            "assistant",
            "unrelated filler text",
        )
        .await;
    }
    super::helpers::seed_text_message(&server.database_url, branch_id, "user", "a 50% discount")
        .await;

    let search = |q: &str, page: i64, per_page: i64| {
        let token = user.token.clone();
        let url = server.api_url(&format!(
            "/conversations/{}/messages/search?q={}&page={}&per_page={}",
            conv_id,
            urlencoding_encode(q),
            page,
            per_page
        ));
        async move {
            let resp = reqwest::Client::new()
                .get(&url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await
                .unwrap();
            (resp.status(), resp.json::<serde_json::Value>().await.unwrap())
        }
    };

    // page 1 of 2 (per_page 2) → total 5, ordinals 1,2.
    let (st, p1) = search("refund", 1, 2).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(p1["total"], serde_json::json!(5));
    let m1 = p1["matches"].as_array().unwrap();
    assert_eq!(m1.len(), 2);
    assert_eq!(m1[0]["ordinal"], serde_json::json!(1));
    assert_eq!(m1[1]["ordinal"], serde_json::json!(2));
    assert!(
        m1[0]["snippet"].as_str().unwrap().contains("refund"),
        "snippet carries the hit"
    );

    // page 2 → ordinals 3,4 (continuous across the page boundary).
    let (_st, p2) = search("refund", 2, 2).await;
    let m2 = p2["matches"].as_array().unwrap();
    assert_eq!(m2[0]["ordinal"], serde_json::json!(3));
    assert_eq!(p2["total"], serde_json::json!(5));

    // Blank query → empty, no scan.
    let (_st, blank) = search("   ", 1, 25).await;
    assert_eq!(blank["total"], serde_json::json!(0));
    assert_eq!(blank["matches"].as_array().unwrap().len(), 0);

    // LIKE metacharacter escaped: "50%" matches only the literal, not everything.
    let (_st, pct) = search("50%", 1, 25).await;
    assert_eq!(pct["total"], serde_json::json!(1), "`%` is literal, not a wildcard");

    // Cross-user access → 404.
    let other = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["messages::read"],
    )
    .await;
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!(
            "/conversations/{}/messages/search?q=refund",
            conv_id
        )))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "cross-user → 404");
}

/// Minimal percent-encoder for the few query terms the search test sends.
fn urlencoding_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
