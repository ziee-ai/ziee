// ============================================================================
// Extraction-pipeline tests.
//
// Exercises the JSON parser + ADD/UPDATE/DELETE/NOOP dispatch in
// `modules/memory/engine/extractor.rs`. The parser is the easy bit
// — testing the full pipeline against a real LLM is out of scope
// here; that's a Tier-5 real-LLM test. We test the parser in
// isolation by invoking the public function on canned strings.
// ============================================================================

// The parser fn is private to extractor.rs; this test exercises the
// publicly-observable surface by sending a tools/list-style /api/memories
// POST and then PATCH/DELETE and asserting the audit log shape, which
// gives us indirect coverage of ADD/UPDATE/DELETE through the
// production repo path.

use serde_json::{Value, json};

#[tokio::test]
async fn test_audit_log_records_add_update_delete() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "audit_aud",
        &["memory::read", "memory::write"],
    )
    .await;
    let client = reqwest::Client::new();
    let token = &user.token;

    // ADD via POST
    let res = client
        .post(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "content": "user prefers vim", "kind": "preference" }))
        .send()
        .await
        .unwrap();
    let row: Value = res.json().await.unwrap();
    let id = row["id"].as_str().unwrap().to_string();

    // UPDATE via PATCH
    client
        .patch(server.api_url(&format!("/memories/{id}")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "content": "user prefers emacs" }))
        .send()
        .await
        .unwrap();

    // DELETE
    client
        .delete(server.api_url(&format!("/memories/{id}")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();

    // Fetch audit log
    let res = client
        .get(server.api_url("/memory/audit-log"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let log: Vec<Value> = res.json().await.unwrap();
    let ops: Vec<&str> = log.iter().filter_map(|e| e["op"].as_str()).collect();
    assert!(ops.contains(&"ADD"), "audit log should have ADD entry");
    assert!(ops.contains(&"UPDATE"), "audit log should have UPDATE entry");
    assert!(ops.contains(&"DELETE"), "audit log should have DELETE entry");
}

#[tokio::test]
async fn test_audit_log_is_cross_user_scoped() {
    let server = crate::common::TestServer::start().await;
    let alice = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "aud_alice",
        &["memory::read", "memory::write"],
    )
    .await;
    let bob = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "aud_bob",
        &["memory::read", "memory::write"],
    )
    .await;

    // Alice writes a memory.
    reqwest::Client::new()
        .post(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {}", alice.token))
        .json(&json!({ "content": "Alice's audit" }))
        .send()
        .await
        .unwrap();

    // Bob fetches his audit log; must NOT see Alice's entries.
    let res = reqwest::Client::new()
        .get(server.api_url("/memory/audit-log"))
        .header("Authorization", format!("Bearer {}", bob.token))
        .send()
        .await
        .unwrap();
    let log: Vec<Value> = res.json().await.unwrap();
    assert!(
        log.iter()
            .all(|e| e["content_snapshot"] != Value::String("Alice's audit".to_string())),
        "user B must not see user A's audit entries"
    );
}

#[tokio::test]
async fn test_bulk_delete_audit_entry() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "aud_bulk",
        &["memory::read", "memory::write"],
    )
    .await;
    let client = reqwest::Client::new();
    let token = &user.token;

    // Create 3 memories.
    for i in 0..3 {
        client
            .post(server.api_url("/memories"))
            .header("Authorization", format!("Bearer {token}"))
            .json(&json!({ "content": format!("mem {i}") }))
            .send()
            .await
            .unwrap();
    }

    // Bulk delete.
    client
        .delete(server.api_url("/memories/all"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();

    // Audit log should have a BULK_DELETE entry with deleted_count >= 3.
    let res = client
        .get(server.api_url("/memory/audit-log"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    let log: Vec<Value> = res.json().await.unwrap();
    let bulk = log
        .iter()
        .find(|e| e["op"].as_str() == Some("BULK_DELETE"))
        .expect("BULK_DELETE audit entry missing");
    assert!(
        bulk["metadata"]["deleted_count"].as_i64().unwrap_or(0) >= 3,
        "bulk delete should record count: {:?}",
        bulk
    );
}
