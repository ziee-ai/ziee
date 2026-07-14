//! DB-level Tier-2 tests for the atomic-at-write-time `sequence_order` that
//! `append_content` provides via `INSERT ... (SELECT MAX(sequence_order)+1 ...)`.
//!
//! Covers plan A4's missing DB pieces: an interleaved append sequence that
//! mirrors the reported transcript (two parallel write_file tool_uses + their
//! results, then a follow-up execute_command + its result) must produce
//! strictly-increasing, gap-free `sequence_order`s on first write AND on
//! reload — i.e. the cache↔DB drift that caused the "tool_use should have
//! tool_result blocks" failure is gone.
//!
//! Drives the real repository (`ChatCoreRepository`) against a per-test
//! database created by `TestServer`. No AI provider is needed: the test only
//! creates a conversation/branch/message and appends raw content rows.

use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;
use ziee::test_internals::{ChatCoreRepository, MessageContentData};

use crate::chat::helpers;
use crate::common::test_helpers;

fn tool_use_block(id: &str, name: &str) -> MessageContentData {
    serde_json::from_value(json!({
        "type": "tool_use",
        "id": id,
        "name": name,
        "server_id": "00000000-0000-0000-0000-000000000000",
        "input": {}
    }))
    .expect("tool_use MessageContentData")
}

fn tool_result_block(tool_use_id: &str, content: &str) -> MessageContentData {
    serde_json::from_value(json!({
        "type": "tool_result",
        "tool_use_id": tool_use_id,
        "content": content
    }))
    .expect("tool_result MessageContentData")
}

async fn setup_assistant_message(
    server: &crate::common::TestServer,
) -> (Uuid, ChatCoreRepository) {
    let user = test_helpers::create_user_with_permissions(
        server,
        "append_test_user",
        &[
            "conversations::create",
            "conversations::read",
            "messages::create",
        ],
    )
    .await;
    let conv = helpers::create_conversation(server, &user.token, None, None).await;
    let branch_id = helpers::parse_uuid(&conv["active_branch_id"]);

    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&server.database_url)
        .await
        .expect("connect to per-test database");
    let repo = ChatCoreRepository::new(pool);

    let assistant = repo
        .create_message(branch_id, "assistant", None)
        .await
        .expect("create assistant message row");

    (assistant.id, repo)
}

/// The reported bug scenario at the persistence layer: iteration 1 emits two
/// parallel `write_file` tool_uses + their results (via the Continue handler);
/// iteration 2 emits `execute_command` + its result. Each `append_content`
/// must take the next strictly-increasing slot, gap-free, no collisions —
/// reloaded contents must match exactly.
#[tokio::test]
async fn append_content_yields_monotonic_sequence_order_for_parallel_tool_iteration() {
    let server = crate::common::TestServer::start().await;
    let (msg_id, repo) = setup_assistant_message(&server).await;

    // Iteration 1: 2 parallel tool_uses (finalize) + their results (Continue handler).
    let u1 = repo
        .append_content(msg_id, "tool_use", tool_use_block("w1", "write_file"))
        .await
        .expect("append u1");
    let u2 = repo
        .append_content(msg_id, "tool_use", tool_use_block("w2", "write_file"))
        .await
        .expect("append u2");
    let r1 = repo
        .append_content(msg_id, "tool_result", tool_result_block("w1", "ok"))
        .await
        .expect("append r1");
    let r2 = repo
        .append_content(msg_id, "tool_result", tool_result_block("w2", "ok"))
        .await
        .expect("append r2");

    // Iteration 2: a follow-up tool_use + its result.
    let u3 = repo
        .append_content(msg_id, "tool_use", tool_use_block("exec", "execute_command"))
        .await
        .expect("append u3");
    let r3 = repo
        .append_content(msg_id, "tool_result", tool_result_block("exec", "ran"))
        .await
        .expect("append r3");

    // Each insert takes the next slot — no collisions, no gaps.
    assert_eq!(u1.sequence_order, 0);
    assert_eq!(u2.sequence_order, 1);
    assert_eq!(r1.sequence_order, 2);
    assert_eq!(r2.sequence_order, 3);
    assert_eq!(u3.sequence_order, 4);
    assert_eq!(r3.sequence_order, 5);

    // Persisted state matches.
    let reloaded = repo
        .get_message_with_content(msg_id)
        .await
        .expect("reload")
        .expect("message exists");
    let orders: Vec<i32> = reloaded.contents.iter().map(|c| c.sequence_order).collect();
    assert_eq!(
        orders,
        vec![0, 1, 2, 3, 4, 5],
        "reloaded sequence_orders must be strictly increasing and gap-free"
    );
    assert_eq!(reloaded.contents.len(), 6);
}

/// TEST-11 (fix-duplicate-tool-result): migrations 114 and 124 each added a unique
/// index on the SAME columns, so every deployed DB carried TWO — double write cost
/// and a coin-flip over which name a violation reports. Migration 158 drops 114's
/// bare index and keeps 124's named constraint.
///
/// Asserts the outcome, not the mechanism: exactly ONE unique index remains on
/// `(message_id, sequence_order)`, it is the constraint's, and it STILL rejects a
/// colliding slot — the protection is preserved, only the duplicate removed.
#[tokio::test]
async fn message_contents_has_exactly_one_unique_sequence_guard() {
    let server = crate::common::TestServer::start().await;
    let (msg_id, repo) = setup_assistant_message(&server).await;

    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&server.database_url)
        .await
        .expect("connect to per-test database");

    // Every UNIQUE index on exactly (message_id, sequence_order).
    let names: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT i.relname::text
        FROM pg_index x
        JOIN pg_class i ON i.oid = x.indexrelid
        JOIN pg_class t ON t.oid = x.indrelid
        WHERE t.relname = 'message_contents'
          AND x.indisunique
          AND (
            SELECT array_agg(a.attname::text ORDER BY a.attname)
            FROM unnest(x.indkey) AS k(attnum)
            JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = k.attnum
          ) = ARRAY['message_id','sequence_order']
        ORDER BY i.relname
        "#,
    )
    .fetch_all(&pool)
    .await
    .expect("query unique indexes on message_contents");

    assert_eq!(
        names,
        vec!["uq_message_contents_message_sequence".to_string()],
        "exactly one unique guard on (message_id, sequence_order) must remain — 114's \
         redundant idx_message_contents_message_seq_unique is dropped by migration 158, \
         124's named constraint survives. Got: {names:?}"
    );

    // …and it still WORKS: a colliding slot is rejected. (append_content computes
    // MAX+1 so it can't collide; go around it to force the collision.)
    repo.append_content(msg_id, "tool_use", tool_use_block("t1", "search"))
        .await
        .expect("first append takes slot 0");

    let collide = sqlx::query(
        "INSERT INTO message_contents (message_id, content_type, content, sequence_order) \
         VALUES ($1, 'tool_use', '{}'::jsonb, 0)",
    )
    .bind(msg_id)
    .execute(&pool)
    .await;

    assert!(
        collide.is_err(),
        "the surviving constraint must still reject a duplicate (message_id, sequence_order)"
    );
}
