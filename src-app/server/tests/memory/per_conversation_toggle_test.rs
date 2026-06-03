// ============================================================================
// Per-conversation memory_mode override (plan §9 Phase 5).
//
// Asserts the memory-owned `PUT /api/conversations/{id}/memory-mode`
// endpoint accepts the allowed values, rejects bad ones, and round-trips
// via `GET /api/conversations/{id}/memory-mode`. The endpoint replaces
// the earlier `PATCH /api/conversations/{id}` body field (chat no
// longer knows the memory_mode vocabulary — migration 76 moved the
// column into `conversation_memory_settings`).
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
            .put(server.api_url(&format!("/conversations/{conv_id}/memory-mode")))
            .header("Authorization", format!("Bearer {token}"))
            .json(&json!({ "memory_mode": mode }))
            .send()
            .await
            .unwrap();
        let status = res.status();
        let body: Value = res.json().await.unwrap_or_else(|_| json!({}));
        assert!(
            status.is_success(),
            "PUT memory_mode={mode} failed: {status} body={body}",
        );
        assert_eq!(
            body["memory_mode"].as_str(),
            Some(mode),
            "response should echo the set mode"
        );

        // GET round-trip should see the value we just wrote.
        let get_res = reqwest::Client::new()
            .get(server.api_url(&format!("/conversations/{conv_id}/memory-mode")))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .unwrap();
        assert!(get_res.status().is_success(), "GET memory-mode failed");
        let get_body: Value = get_res.json().await.unwrap();
        assert_eq!(
            get_body["memory_mode"].as_str(),
            Some(mode),
            "GET should return the value just written"
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
        .put(server.api_url(&format!("/conversations/{conv_id}/memory-mode")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "memory_mode": "maybe" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400, "invalid memory_mode must be rejected");
}

#[tokio::test]
async fn test_memory_mode_defaults_to_inherit_when_unset() {
    // A freshly-created conversation has NO row in
    // conversation_memory_settings; GET should report 'inherit' (the
    // implicit default per migration 76).
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "conv_default",
        &["conversations::read", "conversations::edit"],
    )
    .await;
    let conv_id = create_conversation(&server, &user.token).await;

    let res = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{conv_id}/memory-mode")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "GET memory-mode failed");
    let body: Value = res.json().await.unwrap();
    assert_eq!(
        body["memory_mode"].as_str(),
        Some("inherit"),
        "default for an unset conversation should be 'inherit'"
    );
}

#[tokio::test]
async fn test_memory_mode_returns_404_for_other_users_conversation() {
    // The endpoint must NOT leak existence of a conversation owned by
    // another user — same conflation pattern as the assistant bridge's
    // GET /api/messages/{id}/assistant.
    let server = crate::common::TestServer::start().await;
    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "conv_owner",
        &["conversations::read", "conversations::edit"],
    )
    .await;
    let intruder = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "conv_intruder",
        &["conversations::read", "conversations::edit"],
    )
    .await;
    let conv_id = create_conversation(&server, &owner.token).await;

    let res = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{conv_id}/memory-mode")))
        .header("Authorization", format!("Bearer {}", intruder.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        404,
        "intruder must get 404 (conflated to defeat probing)"
    );
}
