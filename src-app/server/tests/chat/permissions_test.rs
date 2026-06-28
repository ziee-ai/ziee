//! Permission tests for chat module
//!
//! Tests that all endpoints properly enforce permission requirements

use reqwest::StatusCode;
use serde_json::json;

// =====================================================
// Conversation Permission Tests
// =====================================================

#[tokio::test]
async fn test_create_conversation_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    let payload = json!({
        "title": "Test Conversation"
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_list_conversations_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    let response = reqwest::Client::new()
        .get(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_get_conversation_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["conversations::create", "conversations::read"],
    )
    .await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    // Admin creates a conversation
    let conversation = super::helpers::create_conversation(&server, &admin.token, None, Some("Test")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // User without permission tries to get it (will fail on ownership anyway, but permission check comes first)
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_update_conversation_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["conversations::create", "conversations::read"],
    )
    .await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    // Admin creates a conversation
    let conversation = super::helpers::create_conversation(&server, &admin.token, None, Some("Test")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    let payload = json!({
        "title": "Updated Title"
    });

    // User without permission tries to update it
    let response = reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_delete_conversation_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["conversations::create", "conversations::read"],
    )
    .await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    // Admin creates a conversation
    let conversation = super::helpers::create_conversation(&server, &admin.token, None, Some("Test")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // User without permission tries to delete it
    let response = reqwest::Client::new()
        .delete(server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// =====================================================
// Message Permission Tests
// =====================================================

#[tokio::test]
async fn test_get_conversation_history_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["conversations::create"],
    )
    .await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    // Admin creates a conversation
    let conversation = super::helpers::create_conversation(&server, &admin.token, None, Some("Test")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // User without permission tries to get history
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{}/messages", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_send_message_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "conversations::create",
            "llm_models::read",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    // Admin creates a conversation
    let conversation = super::helpers::create_conversation(&server, &admin.token, None, Some("Test")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    // Get a test model (deterministic stub — the 403 fires at the
    // RequirePermissions gate before any model lookup, so the model is
    // unused; the stub just avoids an API-key dependency).
    let (_stub, model) = super::helpers::create_stub_model(&server, &admin.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // User without permission tries to send message
    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        model_id,
        branch_id,
        "Hello, world!",
    )
    .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// A user who HAS `messages::create` but is NOT in any group granted the
/// model's provider must be blocked by the streaming handler's
/// `user_has_access_to_provider` check (403 ACCESS_DENIED) — distinct from the
/// RequirePermissions gate above. This exercises the chat-stream ↔ provider
/// access-resolution path.
#[tokio::test]
async fn test_send_message_denied_without_provider_access() {
    let server = crate::common::TestServer::start().await;

    // `owner` gets the stub model (create_stub_model assigns the provider to a
    // group containing `owner`).
    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "prov_owner",
        &["llm_models::read"],
    )
    .await;
    let (_stub, model) = super::helpers::create_stub_model(&server, &owner.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // `user` can chat (has the message permissions) but is in NO group granted
    // this provider.
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "no_prov_access",
        &[
            "conversations::create",
            "conversations::read",
            "messages::create",
            "messages::read",
            "llm_models::read",
        ],
    )
    .await;
    let conversation =
        super::helpers::create_conversation(&server, &user.token, None, Some("Test")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        model_id,
        branch_id,
        "Hello, world!",
    )
    .await;

    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "no provider access must be 403"
    );
    let body = response.text().await.unwrap_or_default();
    assert!(
        body.contains("ACCESS_DENIED"),
        "403 must carry the ACCESS_DENIED code; got: {body}"
    );
}

/// A disabled model is rejected by the streaming handler with MODEL_DISABLED
/// (400) before any provider/key resolution.
#[tokio::test]
async fn test_send_message_disabled_model_is_rejected() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "disabled_model_user",
        &[
            "conversations::create",
            "conversations::read",
            "messages::create",
            "messages::read",
            "llm_models::read",
            "llm_models::edit",
        ],
    )
    .await;
    let (_stub, model) = super::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = super::helpers::parse_uuid(&model["id"]);

    // Disable the model.
    let disable = reqwest::Client::new()
        .put(server.api_url(&format!("/llm-models/{model_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "enabled": false }))
        .send()
        .await
        .unwrap();
    assert!(disable.status().is_success(), "disable model → {}", disable.status());

    let conversation =
        super::helpers::create_conversation(&server, &user.token, None, Some("Test")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    let response = super::helpers::send_message_simple(
        &server,
        &user.token,
        conversation_id,
        model_id,
        branch_id,
        "Hello, world!",
    )
    .await;

    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "disabled model must be 400"
    );
    let body = response.text().await.unwrap_or_default();
    assert!(
        body.contains("MODEL_DISABLED"),
        "400 must carry the MODEL_DISABLED code; got: {body}"
    );
}

#[tokio::test]
async fn test_get_message_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    // Use a random UUID (permission check happens before existence check)
    let fake_message_id = uuid::Uuid::new_v4();

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/messages/{}", fake_message_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_edit_message_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    // Use random UUIDs (permission check happens before existence check)
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

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_delete_message_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    // Use a random UUID (permission check happens before existence check)
    let fake_message_id = uuid::Uuid::new_v4();

    let response = reqwest::Client::new()
        .delete(server.api_url(&format!("/messages/{}", fake_message_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// =====================================================
// Branch Permission Tests
// =====================================================

#[tokio::test]
async fn test_create_branch_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["conversations::create"],
    )
    .await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    // Admin creates a conversation
    let conversation = super::helpers::create_conversation(&server, &admin.token, None, Some("Test")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    let payload = json!({});

    // User without permission tries to create branch
    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{}/branches", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_list_branches_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["conversations::create"],
    )
    .await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    // Admin creates a conversation
    let conversation = super::helpers::create_conversation(&server, &admin.token, None, Some("Test")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // User without permission tries to list branches
    // Note: list_branches uses conversations::read permission
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{}/branches", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_activate_branch_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["conversations::create"],
    )
    .await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    // Admin creates a conversation
    let conversation = super::helpers::create_conversation(&server, &admin.token, None, Some("Test")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
    let branch_id = super::helpers::parse_uuid(&conversation["active_branch_id"]);

    // User without permission tries to activate branch
    let response = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/conversations/{}/branches/{}/activate",
            conversation_id, branch_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// =====================================================
// Permission Grant Tests (verify permissions work when granted)
// =====================================================

#[tokio::test]
async fn test_create_conversation_succeeds_with_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create"],
    )
    .await;

    let payload = json!({
        "title": "Test Conversation"
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_list_conversations_succeeds_with_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::read"],
    )
    .await;

    let response = reqwest::Client::new()
        .get(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_get_conversation_succeeds_with_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::read"],
    )
    .await;

    // Create a conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, Some("Test")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // Get it back
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_update_conversation_succeeds_with_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::edit"],
    )
    .await;

    // Create a conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, Some("Test")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    let payload = json!({
        "title": "Updated Title"
    });

    // Update it
    let response = reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_delete_conversation_succeeds_with_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::delete"],
    )
    .await;

    // Create a conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, Some("Test")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // Delete it
    let response = reqwest::Client::new()
        .delete(server.api_url(&format!("/conversations/{}", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_get_conversation_history_succeeds_with_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "messages::read"],
    )
    .await;

    // Create a conversation
    let conversation = super::helpers::create_conversation(&server, &user.token, None, Some("Test")).await;
    let conversation_id = super::helpers::parse_uuid(&conversation["id"]);

    // Get history
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{}/messages", conversation_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
