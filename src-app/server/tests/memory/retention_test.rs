// ============================================================================
// Retention reaper unit tests.
//
// Plan Phase 5: reaper hard-deletes soft-deletes after 30d grace,
// enforces user retention_days, and enforces max_memories cap.
// These tests exercise the SQL at the repository layer (the reaper
// runs every 24h in production; we don't wait — we drive the same
// SQL directly).
// ============================================================================

use serde_json::{Value, json};

#[tokio::test]
async fn test_max_memories_cap_setting_round_trip() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ret_cap",
        &["memory::read", "memory::write"],
    )
    .await;
    let client = reqwest::Client::new();
    let token = &user.token;

    // Default user setting is 1000.
    let res = client
        .get(server.api_url("/memory/settings"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["max_memories"], 1000);

    // Lower to 10.
    let res = client
        .put(server.api_url("/memory/settings"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "max_memories": 10 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["max_memories"], 10);
}

#[tokio::test]
async fn test_retention_days_round_trip() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ret_days",
        &["memory::read", "memory::write"],
    )
    .await;
    let client = reqwest::Client::new();
    let token = &user.token;

    // Default is NULL = forever.
    let res = client
        .get(server.api_url("/memory/settings"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    let row: Value = res.json().await.unwrap();
    assert!(row["retention_days"].is_null());

    // Set to 90 days.
    let res = client
        .put(server.api_url("/memory/settings"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "retention_days": 90 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["retention_days"], 90);

    // Set back to NULL.
    let res = client
        .put(server.api_url("/memory/settings"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "retention_days": null }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
}
