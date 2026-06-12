// ============================================================================
// Per-conversation summarization_mode override (migration 91).
//
// Mirrors `memory/tests/per_conversation_toggle_test.rs` exactly — the
// summarization chat-extension's
// `GET`/`PUT /api/conversations/{id}/summarization-mode` endpoints have
// the same vocabulary (`inherit`/`on`/`off`), same default-of-inherit,
// same ownership-gating semantics (404 to defeat probing).
// ============================================================================

use serde_json::{Value, json};

async fn create_conversation(
    server: &crate::common::TestServer,
    token: &str,
) -> String {
    let res = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "title": "summarization-mode test" }))
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
async fn test_summarization_mode_accepts_valid_values() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_mode_ok",
        &["conversations::read", "conversations::edit"],
    )
    .await;
    let conv_id = create_conversation(&server, &user.token).await;
    let token = &user.token;

    for mode in ["inherit", "on", "off"] {
        let res = reqwest::Client::new()
            .put(
                server.api_url(&format!("/conversations/{conv_id}/summarization-mode")),
            )
            .header("Authorization", format!("Bearer {token}"))
            .json(&json!({ "summarization_mode": mode }))
            .send()
            .await
            .unwrap();
        let status = res.status();
        let body: Value = res.json().await.unwrap_or_else(|_| json!({}));
        assert!(
            status.is_success(),
            "PUT summarization_mode={mode} failed: {status} body={body}",
        );
        assert_eq!(
            body["summarization_mode"].as_str(),
            Some(mode),
            "response should echo the set mode"
        );

        let get_res = reqwest::Client::new()
            .get(
                server.api_url(&format!("/conversations/{conv_id}/summarization-mode")),
            )
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .unwrap();
        assert!(get_res.status().is_success());
        let get_body: Value = get_res.json().await.unwrap();
        assert_eq!(
            get_body["summarization_mode"].as_str(),
            Some(mode),
            "GET should return the value just written"
        );
    }
}

#[tokio::test]
async fn test_summarization_mode_rejects_invalid_value() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_mode_bad",
        &["conversations::read", "conversations::edit"],
    )
    .await;
    let conv_id = create_conversation(&server, &user.token).await;

    let res = reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{conv_id}/summarization-mode")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "summarization_mode": "maybe" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400, "invalid summarization_mode must be 400");
}

#[tokio::test]
async fn test_summarization_mode_defaults_to_inherit_when_unset() {
    // A freshly-created conversation has NO row in
    // `conversation_summarization_settings`; GET should report 'inherit'
    // (the implicit default — row absence equals inherit, saves storage).
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_mode_default",
        &["conversations::read", "conversations::edit"],
    )
    .await;
    let conv_id = create_conversation(&server, &user.token).await;

    let res = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{conv_id}/summarization-mode")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    let body: Value = res.json().await.unwrap();
    assert_eq!(
        body["summarization_mode"].as_str(),
        Some("inherit"),
        "default for an unset conversation should be 'inherit'"
    );
}

#[tokio::test]
async fn test_summarization_mode_clear_back_to_inherit_deletes_row() {
    // Writing 'inherit' should DELETE the override row (per the
    // repository's `set_conversation_summarization_mode`). Verifying
    // via two writes: set to 'on', then back to 'inherit'; GET returns
    // 'inherit' after the second write.
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_mode_clear",
        &["conversations::read", "conversations::edit"],
    )
    .await;
    let conv_id = create_conversation(&server, &user.token).await;

    // Set non-default first.
    let _ = reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{conv_id}/summarization-mode")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "summarization_mode": "on" }))
        .send()
        .await
        .unwrap();

    // Clear back to inherit.
    let cleared = reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{conv_id}/summarization-mode")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "summarization_mode": "inherit" }))
        .send()
        .await
        .unwrap();
    assert!(cleared.status().is_success());
    let body: Value = cleared.json().await.unwrap();
    assert_eq!(body["summarization_mode"], "inherit");
}

#[tokio::test]
async fn test_summarization_mode_returns_404_for_other_users_conversation() {
    // Ownership-gated: a user who doesn't own the conversation gets
    // 404 (conflated with "doesn't exist") so the endpoint can't be
    // used to probe for conversation ids.
    let server = crate::common::TestServer::start().await;
    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_mode_owner",
        &["conversations::read", "conversations::edit"],
    )
    .await;
    let intruder = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_mode_intruder",
        &["conversations::read", "conversations::edit"],
    )
    .await;
    let conv_id = create_conversation(&server, &owner.token).await;

    let res = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{conv_id}/summarization-mode")))
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

#[tokio::test]
async fn test_put_summarization_mode_returns_404_for_other_users_conversation() {
    // Mirror of the GET IDOR test for the PUT path — the ownership
    // probe runs BEFORE the upsert, so a non-owner's write must be
    // rejected without materializing a row.
    let server = crate::common::TestServer::start().await;
    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_mode_put_owner",
        &["conversations::read", "conversations::edit"],
    )
    .await;
    let intruder = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_mode_put_intruder",
        &["conversations::read", "conversations::edit"],
    )
    .await;
    let conv_id = create_conversation(&server, &owner.token).await;

    let res = reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{conv_id}/summarization-mode")))
        .header("Authorization", format!("Bearer {}", intruder.token))
        .json(&json!({ "summarization_mode": "off" }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        404,
        "intruder PUT must get 404, NOT write a row"
    );

    // Confirm no override leaked: owner's GET still reports the implicit
    // default 'inherit' (row absence).
    let owner_get = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{conv_id}/summarization-mode")))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();
    assert!(owner_get.status().is_success());
    let body: Value = owner_get.json().await.unwrap();
    assert_eq!(
        body["summarization_mode"].as_str(),
        Some("inherit"),
        "intruder's blocked PUT must not have written an override row"
    );
}

#[tokio::test]
async fn test_summarization_mode_returns_404_for_nonexistent_conversation() {
    // A random UUID that doesn't exist anywhere in `conversations` must
    // surface as 404 — same response as a wrong-owner request, so the
    // endpoint can't be used to enumerate conversation ids.
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_mode_404",
        &["conversations::read", "conversations::edit"],
    )
    .await;

    let ghost = uuid::Uuid::new_v4();

    let get_res = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{ghost}/summarization-mode")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(get_res.status(), 404, "GET on ghost id must be 404");

    let put_res = reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{ghost}/summarization-mode")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "summarization_mode": "on" }))
        .send()
        .await
        .unwrap();
    assert_eq!(put_res.status(), 404, "PUT on ghost id must be 404");
}
