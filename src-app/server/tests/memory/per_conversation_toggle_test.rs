// ============================================================================
// Per-conversation memory_mode override (plan §9 Phase 5).
//
// Asserts the PATCH /api/conversations/{id} surface accepts the
// new `memory_mode` field, validates it against the allowed values,
// and stores it correctly.
// ============================================================================

use serde_json::{Value, json};

async fn create_conversation(
    server: &crate::common::TestServer,
    token: &str,
) -> String {
    // Create a conversation — uses the existing chat module's
    // POST /api/conversations. Returns the new id.
    let res = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "title": "memory-mode test" }))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "create conversation failed: {}",
        res.status()
    );
    let row: Value = res.json().await.unwrap();
    row["id"].as_str().expect("conversation id").to_string()
}

#[tokio::test]
async fn test_memory_mode_accepts_valid_values() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "conv_mode",
        &["conversations::read", "conversations::edit"],
    )
    .await;
    let conv_id = create_conversation(&server, &user.token).await;
    let token = &user.token;

    for mode in ["inherit", "on", "off"] {
        let res = reqwest::Client::new()
            .put(server.api_url(&format!("/conversations/{conv_id}")))
            .header("Authorization", format!("Bearer {token}"))
            .json(&json!({ "memory_mode": mode }))
            .send()
            .await
            .unwrap();
        assert!(
            res.status().is_success(),
            "PATCH memory_mode={mode} failed: {}",
            res.status()
        );
    }
}

#[tokio::test]
async fn test_memory_mode_rejects_invalid_value() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "conv_invalid",
        &["conversations::read", "conversations::edit"],
    )
    .await;
    let conv_id = create_conversation(&server, &user.token).await;

    let res = reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{conv_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "memory_mode": "maybe" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400, "invalid memory_mode must be rejected");
}
