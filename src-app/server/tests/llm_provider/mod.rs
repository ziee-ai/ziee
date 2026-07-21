use reqwest::StatusCode;
use serde_json::json;
use uuid::Uuid;
use ziee::resolve_api_key_for_user;
use ziee::UserKeyRepository;

// Integration tests for LLM Provider module

mod discover_models_test;
mod sync_emit_test;

// =====================================================
// Permission Tests
// =====================================================

#[tokio::test]
async fn test_list_providers_requires_read_permission() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let response = reqwest::Client::new()
        .get(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_list_providers_with_read_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::read"],
    )
    .await;

    let response = reqwest::Client::new()
        .get(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["providers"].is_array());
    assert!(body["total"].as_i64().unwrap() >= 7); // Built-in providers
}

#[tokio::test]
async fn test_get_provider_requires_read_permission() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let provider_id = Uuid::new_v4();
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_create_provider_requires_create_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::read"],
    )
    .await;

    let payload = json!({
        "name": "Test Provider",
        "provider_type": "openai",
        "enabled": false
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_update_provider_requires_edit_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::read", "llm_providers::create"],
    )
    .await;

    // Create a provider first
    let provider = create_test_provider(&server, &user.token).await;

    // Try to update without edit permission
    let payload = json!({
        "name": "Updated Name"
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{}", provider["id"])))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_delete_provider_requires_delete_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::read", "llm_providers::create"],
    )
    .await;

    // Create a provider first
    let provider = create_test_provider(&server, &user.token).await;

    // Try to delete without delete permission
    let response = reqwest::Client::new()
        .delete(server.api_url(&format!("/llm-providers/{}", provider["id"])))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_assign_provider_to_group_requires_assign_groups_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::read"],
    )
    .await;

    let provider_id = Uuid::new_v4();
    let payload = json!({
        "group_id": Uuid::new_v4().to_string()
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{}/groups", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// =====================================================
// CRUD Tests
// =====================================================

#[tokio::test]
async fn test_list_providers_with_pagination() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::read"],
    )
    .await;

    // Test default pagination
    let response = reqwest::Client::new()
        .get(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["page"], 1);
    assert_eq!(body["per_page"], 20);
    assert!(body["total"].as_i64().unwrap() >= 7);

    // Test custom pagination
    let response = reqwest::Client::new()
        .get(server.api_url("/llm-providers?page=1&per_page=3"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["page"], 1);
    assert_eq!(body["per_page"], 3);
    assert_eq!(body["providers"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn test_get_provider_by_id_success() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::read", "llm_providers::create"],
    )
    .await;

    // Create a provider
    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Get the provider
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["id"], provider["id"]);
    assert_eq!(body["name"], "Test Provider");
    assert_eq!(body["provider_type"], "openai");
}

#[tokio::test]
async fn test_get_provider_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::read"],
    )
    .await;

    let provider_id = Uuid::new_v4();
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// A remote provider created `enabled: true` WITHOUT an api_key must NOT be
/// rejected, and must stay ENABLED: the multi-tenant onboarding flow provisions
/// exactly this so each user supplies their own key (resolved per-user at
/// request time). A prior 400 broke that flow.
#[tokio::test]
async fn test_create_enabled_remote_without_key_is_allowed() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::read", "llm_providers::create"],
    )
    .await;

    // Keyless + enabled remote → created AND stays enabled (no 400, no disable).
    let resp = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "Keyless OpenAI",
            "provider_type": "openai",
            "enabled": true,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "keyless enabled remote provider must be created, not rejected"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        body["enabled"], true,
        "keyless enabled remote provider must stay enabled (per-user keys)"
    );
}

#[tokio::test]
async fn test_create_provider_minimal() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::create"],
    )
    .await;

    let payload = json!({
        "name": "Minimal Provider",
        "provider_type": "anthropic",
        "enabled": false
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["name"], "Minimal Provider");
    assert_eq!(body["provider_type"], "anthropic");
    assert_eq!(body["enabled"], false);
    assert_eq!(body["built_in"], false);
}

#[tokio::test]
async fn test_create_provider_with_all_fields() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::create"],
    )
    .await;

    let payload = json!({
        "name": "Full Provider",
        "provider_type": "openai",
        "enabled": true,
        "api_key": "sk-test123",
        "base_url": "https://api.openai.com/v1",
        "proxy_settings": {
            "enabled": true,
            "url": "http://proxy.example.com:8080",
            "username": "proxyuser",
            "password": "proxypass",
            "no_proxy": "localhost,127.0.0.1",
            "ignore_ssl_certificates": false
        }
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["name"], "Full Provider");
    // api_key is write-only post-06-llm-provider-F-01 — must NOT be returned.
    assert!(
        body.get("api_key").is_none() || body["api_key"].is_null(),
        "api_key must not be returned in response (06-llm-provider F-01); got {:?}",
        body.get("api_key")
    );
    assert_eq!(body["base_url"], "https://api.openai.com/v1");
    assert_eq!(body["proxy_settings"]["enabled"], true);
    assert_eq!(
        body["proxy_settings"]["url"],
        "http://proxy.example.com:8080"
    );
}

#[tokio::test]
async fn test_update_provider_name() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    // Create a provider
    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Update name
    let payload = json!({
        "name": "Updated Provider Name"
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["name"], "Updated Provider Name");
    assert_eq!(body["provider_type"], "openai"); // Unchanged
}

#[tokio::test]
async fn test_update_provider_enabled_and_api_key() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    // Create a provider
    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Update enabled and api_key
    let payload = json!({
        "enabled": true,
        "api_key": "sk-newkey456"
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["enabled"], true);
    // api_key is write-only post-06-llm-provider-F-01 — must NOT be returned.
    assert!(
        body.get("api_key").is_none() || body["api_key"].is_null(),
        "api_key must not be returned in response (06-llm-provider F-01)"
    );
}

#[tokio::test]
async fn test_update_provider_proxy_settings() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    // Create a provider
    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Update proxy settings
    let payload = json!({
        "proxy_settings": {
            "enabled": true,
            "url": "http://newproxy.example.com:3128"
        }
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["proxy_settings"]["enabled"], true);
    assert_eq!(
        body["proxy_settings"]["url"],
        "http://newproxy.example.com:3128"
    );
}

#[tokio::test]
async fn test_update_provider_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::edit"],
    )
    .await;

    let provider_id = Uuid::new_v4();
    let payload = json!({
        "name": "Updated Name"
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_custom_provider() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::delete",
        ],
    )
    .await;

    // Create a provider
    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Delete it
    let response = reqwest::Client::new()
        .delete(server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify it's gone
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_built_in_provider_fails() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::read", "llm_providers::delete"],
    )
    .await;

    // Get a built-in provider (OpenAI)
    let response = reqwest::Client::new()
        .get(server.api_url("/llm-providers?page=1&per_page=100"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = response.json().await.unwrap();
    let providers = body["providers"].as_array().unwrap();
    let openai_provider = providers
        .iter()
        .find(|p| p["name"] == "OpenAI")
        .expect("OpenAI built-in provider should exist");
    let provider_id = openai_provider["id"].as_str().unwrap();

    // Try to delete it
    let response = reqwest::Client::new()
        .delete(server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_delete_provider_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::delete"],
    )
    .await;

    let provider_id = Uuid::new_v4();
    let response = reqwest::Client::new()
        .delete(server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    // DELETE is idempotent: deleting an absent provider is a no-op success
    // (204), not a spurious 404. See `delete_provider` in handlers/admin.rs.
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

// =====================================================
// Validation Tests
// =====================================================

#[tokio::test]
async fn test_create_provider_invalid_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::create"],
    )
    .await;

    let payload = json!({
        "name": "Invalid Provider",
        "provider_type": "invalid_type",
        "enabled": false
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    // Just verify we got a bad request - the exact error format may vary
}

#[tokio::test]
async fn test_create_provider_empty_name() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::create"],
    )
    .await;

    let payload = json!({
        "name": "",
        "provider_type": "openai",
        "enabled": false
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_provider_invalid_base_url() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::create"],
    )
    .await;

    let payload = json!({
        "name": "Invalid URL Provider",
        "provider_type": "openai",
        "enabled": false,
        "base_url": "not-a-valid-url"
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// A remote provider may be created ENABLED without an admin-supplied API key:
/// the multi-tenant onboarding flow provisions exactly this so each user pastes
/// their OWN key on the AI-Providers step (per-user keys are resolved at request
/// time via `resolve_api_key_for_user`). The old spurious 400 was deliberately
/// removed (see `handlers::admin::create_provider`) — creating such a provider
/// must SUCCEED (201); it simply doesn't serve a user until that user has a key.
#[tokio::test]
async fn test_create_enabled_remote_provider_without_key_succeeds_for_per_user_onboarding() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::create"],
    )
    .await;

    let payload = json!({
        "name": "Enabled Without Key",
        "provider_type": "openai",
        "enabled": true
        // No api_key provided — valid for per-user onboarding.
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["enabled"], true);
}

#[tokio::test]
async fn test_create_local_provider_no_api_key_required() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::create"],
    )
    .await;

    let payload = json!({
        "name": "Local Provider",
        "provider_type": "local",
        "enabled": true,
        "base_url": "http://localhost:1234/v1"
        // No api_key - should be OK for local providers
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["provider_type"], "local");
    assert_eq!(body["enabled"], true);
}

#[tokio::test]
async fn test_update_provider_empty_name() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    // Create a provider
    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Try to update with empty name
    let payload = json!({
        "name": ""
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_update_provider_invalid_base_url() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;

    // Create a provider
    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Try to update with invalid base_url
    let payload = json!({
        "base_url": "invalid-url"
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// =====================================================
// Group Assignment Tests
// =====================================================

#[tokio::test]
async fn test_get_provider_groups() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::read", "llm_providers::create"],
    )
    .await;

    // Create a provider
    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Get groups (should be empty initially)
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-providers/{}/groups", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body.is_array());
    assert_eq!(body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_assign_provider_to_group() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::assign_groups",
            "groups::read",
        ],
    )
    .await;

    // Create a provider
    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Get admin group
    let admin_group_id = get_admin_group_id(&server, &user.token).await;

    // Assign provider to group
    let payload = json!({
        "group_id": admin_group_id
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{}/groups", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify assignment
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-providers/{}/groups", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body.as_array().unwrap().len(), 1);
    assert_eq!(body[0]["id"], admin_group_id);
}

#[tokio::test]
async fn test_assign_provider_to_group_idempotent() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::assign_groups",
            "groups::read",
        ],
    )
    .await;

    // Create a provider
    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Get admin group
    let admin_group_id = get_admin_group_id(&server, &user.token).await;

    let payload = json!({
        "group_id": admin_group_id
    });

    // Assign twice - should be idempotent
    let response1 = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{}/groups", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response1.status(), StatusCode::NO_CONTENT);

    let response2 = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{}/groups", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response2.status(), StatusCode::NO_CONTENT);

    // Should still have only one assignment
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-providers/{}/groups", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_remove_provider_from_group() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::assign_groups",
            "groups::read",
        ],
    )
    .await;

    // Create a provider
    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Get admin group
    let admin_group_id = get_admin_group_id(&server, &user.token).await;

    // Assign provider to group
    let payload = json!({
        "group_id": &admin_group_id
    });

    reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{}/groups", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // Remove assignment
    let response = reqwest::Client::new()
        .delete(server.api_url(&format!(
            "/llm-providers/{}/groups/{}",
            provider_id, admin_group_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify it's removed
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-providers/{}/groups", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_remove_provider_from_group_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::assign_groups", "groups::read"],
    )
    .await;

    let provider_id = Uuid::new_v4();
    let group_id = Uuid::new_v4();

    let response = reqwest::Client::new()
        .delete(server.api_url(&format!(
            "/llm-providers/{}/groups/{}",
            provider_id, group_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =====================================================
// Group-Centric Provider Assignment Tests
// =====================================================

#[tokio::test]
async fn test_get_group_providers() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::read", "groups::read"],
    )
    .await;

    // Get admin group
    let admin_group_id = get_admin_group_id(&server, &user.token).await;

    // Get providers for group (should be empty initially)
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/groups/{}/providers", admin_group_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["providers"].is_array());
    assert_eq!(body["providers"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_update_group_providers_bulk() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::assign_groups",
            "groups::read",
        ],
    )
    .await;

    // Create multiple providers
    let provider1 = create_test_provider(&server, &user.token).await;
    let provider2 = create_test_provider(&server, &user.token).await;
    let provider3 = create_test_provider(&server, &user.token).await;

    let provider_id1 = provider1["id"].as_str().unwrap();
    let provider_id2 = provider2["id"].as_str().unwrap();
    let provider_id3 = provider3["id"].as_str().unwrap();

    // Get admin group
    let admin_group_id = get_admin_group_id(&server, &user.token).await;

    // Assign two providers to group
    let payload = json!({
        "provider_ids": [provider_id1, provider_id2]
    });

    let response = reqwest::Client::new()
        .put(server.api_url(&format!("/groups/{}/providers", admin_group_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["providers"].as_array().unwrap().len(), 2);

    // Update assignment - remove provider1, keep provider2, add provider3
    let payload = json!({
        "provider_ids": [provider_id2, provider_id3]
    });

    let response = reqwest::Client::new()
        .put(server.api_url(&format!("/groups/{}/providers", admin_group_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    let providers = body["providers"].as_array().unwrap();
    assert_eq!(providers.len(), 2);

    // Verify correct providers are assigned
    let provider_ids: Vec<String> = providers
        .iter()
        .map(|p| p["id"].as_str().unwrap().to_string())
        .collect();
    assert!(provider_ids.contains(&provider_id2.to_string()));
    assert!(provider_ids.contains(&provider_id3.to_string()));
    assert!(!provider_ids.contains(&provider_id1.to_string()));
}

#[tokio::test]
async fn test_update_group_providers_empty_list() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::assign_groups",
            "groups::read",
        ],
    )
    .await;

    // Create a provider
    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Get admin group
    let admin_group_id = get_admin_group_id(&server, &user.token).await;

    // Assign provider
    let payload = json!({
        "provider_ids": [provider_id]
    });

    reqwest::Client::new()
        .put(server.api_url(&format!("/groups/{}/providers", admin_group_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // Clear all assignments with empty list
    let payload = json!({
        "provider_ids": []
    });

    let response = reqwest::Client::new()
        .put(server.api_url(&format!("/groups/{}/providers", admin_group_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["providers"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_update_group_providers_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::read", "groups::read"],
    )
    .await;

    // Get admin group
    let admin_group_id = get_admin_group_id(&server, &user.token).await;

    let payload = json!({
        "provider_ids": []
    });

    let response = reqwest::Client::new()
        .put(server.api_url(&format!("/groups/{}/providers", admin_group_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_get_group_providers_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let group_id = Uuid::new_v4();
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/groups/{}/providers", group_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// =====================================================
// Built-in Provider Tests
// =====================================================

#[tokio::test]
async fn test_built_in_providers_exist() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::read"],
    )
    .await;

    let response = reqwest::Client::new()
        .get(server.api_url("/llm-providers?per_page=100"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = response.json().await.unwrap();
    let providers = body["providers"].as_array().unwrap();

    // Check for built-in providers
    let expected_providers = vec![
        "OpenAI",
        "Anthropic",
        "Groq",
        "Google Gemini",
        "Mistral AI",
        "DeepSeek",
        "Local",
    ];
    for expected in expected_providers {
        assert!(
            providers
                .iter()
                .any(|p| p["name"] == expected && p["built_in"] == true),
            "Built-in provider '{}' should exist",
            expected
        );
    }
}

#[tokio::test]
async fn test_update_built_in_provider() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_providers::read", "llm_providers::edit"],
    )
    .await;

    // Get OpenAI built-in provider
    let response = reqwest::Client::new()
        .get(server.api_url("/llm-providers?per_page=100"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = response.json().await.unwrap();
    let providers = body["providers"].as_array().unwrap();
    let openai = providers
        .iter()
        .find(|p| p["name"] == "OpenAI")
        .expect("OpenAI provider should exist");
    let provider_id = openai["id"].as_str().unwrap();

    // Update enabled and api_key - should be allowed
    let payload = json!({
        "enabled": true,
        "api_key": "sk-testkey"
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["enabled"], true);
    // api_key is write-only post-06-llm-provider-F-01 — must NOT be returned.
    assert!(
        body.get("api_key").is_none() || body["api_key"].is_null(),
        "api_key must not be returned in response (06-llm-provider F-01)"
    );
    assert_eq!(body["built_in"], true);
}

// =====================================================
// Helper Functions
// =====================================================

async fn create_test_provider(
    server: &crate::common::TestServer,
    token: &str,
) -> serde_json::Value {
    let payload = json!({
        "name": "Test Provider",
        "provider_type": "openai",
        "enabled": false,
        "api_key": "sk-test123"
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    response.json().await.unwrap()
}

async fn get_admin_group_id(server: &crate::common::TestServer, token: &str) -> String {
    let response = reqwest::Client::new()
        .get(server.api_url("/groups"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = response.json().await.unwrap();
    let groups = body["groups"].as_array().unwrap();
    let admin_group = groups
        .iter()
        .find(|g| g["name"] == "Administrators")
        .expect("Administrators group should exist");

    admin_group["id"].as_str().unwrap().to_string()
}

// =====================================================
// User-Facing LLM Provider API Key Management Tests
// =====================================================
//
// Tests for the routes registered in src/modules/llm_provider/handlers/user.rs:
//   GET    /api/user-llm-providers              (requires user_llm_providers::read)
//   GET    /api/user-llm-providers/api-keys      (requires profile::read)
//   POST   /api/user-llm-providers/api-keys      (requires profile::edit)
//   DELETE /api/user-llm-providers/api-keys/{id} (requires profile::edit)

/// Like `create_test_provider`, but lets the caller choose the system api_key
/// (passing None creates a provider with no system-side key — needed for
/// `api_key_configured` flag tests).
async fn create_test_provider_with_key(
    server: &crate::common::TestServer,
    token: &str,
    name: &str,
    api_key: Option<&str>,
) -> serde_json::Value {
    // Use "custom" when no api_key — the backend validates that enabled non-local
    // / non-custom providers must have an api_key, so an `enabled: true, api_key: None`
    // openai provider would 400. "custom" skips that check.
    let provider_type = if api_key.is_some() { "openai" } else { "custom" };
    let mut payload = json!({
        "name": name,
        "provider_type": provider_type,
        "enabled": true,  // get_providers_for_user filters on p.enabled = true
    });
    if let Some(k) = api_key {
        payload["api_key"] = json!(k);
    }

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    response.json().await.unwrap()
}

/// Migration 27 grants `profile::read`, `profile::edit`, and
/// `user_llm_providers::read` to the default "Users" group, so a vanilla
/// `create_user_with_permissions(server, "u", &[])` user would have these
/// perms automatically — useless for testing 403.
///
/// This helper registers a user normally and then REMOVES them from the
/// default Users group so only their explicitly-granted permissions apply.
async fn create_user_without_default_group(
    server: &crate::common::TestServer,
    name: &str,
    permissions: &[&str],
) -> crate::common::test_helpers::TestUser {
    let user = crate::common::test_helpers::create_user_with_permissions(server, name, permissions)
        .await;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let user_uuid = Uuid::parse_str(&user.user_id).expect("Invalid user id");
    sqlx::query(
        "DELETE FROM user_groups
         WHERE user_id = $1
           AND group_id IN (SELECT id FROM groups WHERE is_default = true)",
    )
    .bind(user_uuid)
    .execute(&pool)
    .await
    .expect("Failed to remove default group membership");

    user
}

// ---------- Permission gating (4 tests) ----------

#[tokio::test]
async fn test_get_user_llm_providers_requires_user_llm_providers_read() {
    let server = crate::common::TestServer::start().await;
    let user = create_user_without_default_group(&server, "no_perm", &[]).await;

    let response = reqwest::Client::new()
        .get(server.api_url("/user-llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_list_user_api_keys_requires_profile_read() {
    let server = crate::common::TestServer::start().await;
    let user = create_user_without_default_group(&server, "no_perm", &[]).await;

    let response = reqwest::Client::new()
        .get(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_save_user_api_key_requires_profile_edit() {
    let server = crate::common::TestServer::start().await;
    let user = create_user_without_default_group(&server, "no_perm", &[]).await;

    let response = reqwest::Client::new()
        .post(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "provider_id": Uuid::new_v4(),
            "api_key": "sk-test-1234",
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_delete_user_api_key_requires_profile_edit() {
    let server = crate::common::TestServer::start().await;
    let user = create_user_without_default_group(&server, "no_perm", &[]).await;
    let provider_id = Uuid::new_v4();

    let response = reqwest::Client::new()
        .delete(server.api_url(&format!("/user-llm-providers/api-keys/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// ---------- Happy-path CRUD round-trip (3 tests) ----------

#[tokio::test]
async fn test_save_user_api_key_round_trip() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_providers::create"],
    )
    .await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let provider = create_test_provider_with_key(&server, &admin.token, "OpenAI", None).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Save a key.
    let save_response = reqwest::Client::new()
        .post(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "provider_id": provider_id, "api_key": "sk-secret-key-1234" }))
        .send()
        .await
        .unwrap();
    assert_eq!(save_response.status(), StatusCode::NO_CONTENT);

    // List keys, assert the entry is present and the plaintext is NOT leaked
    // (masked_key = first 4 chars + "***").
    let list_response = reqwest::Client::new()
        .get(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let body: serde_json::Value = list_response.json().await.unwrap();
    let keys = body["keys"].as_array().expect("keys array");
    assert_eq!(keys.len(), 1, "expected exactly one saved key");
    assert_eq!(keys[0]["provider_id"].as_str().unwrap(), provider_id);
    let masked = keys[0]["masked_key"].as_str().unwrap();
    assert_eq!(masked, "sk-s***", "key must be masked as first-4 + ***");
    assert!(
        !masked.contains("secret"),
        "plaintext must never appear in the masked key"
    );
}

#[tokio::test]
async fn test_save_user_api_key_upserts_existing() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_providers::create"],
    )
    .await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let provider = create_test_provider_with_key(&server, &admin.token, "OpenAI", None).await;
    let provider_id = provider["id"].as_str().unwrap();
    let client = reqwest::Client::new();

    // Save twice — second time with a different key.
    for key in &["aaaa-first", "bbbb-second"] {
        let r = client
            .post(server.api_url("/user-llm-providers/api-keys"))
            .header("Authorization", format!("Bearer {}", user.token))
            .json(&json!({ "provider_id": provider_id, "api_key": key }))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::NO_CONTENT);
    }

    // List — expect exactly one entry (upsert, not duplicate) with the SECOND
    // key's masked prefix.
    let list = client
        .get(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = list.json().await.unwrap();
    let keys = body["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 1, "upsert must not produce a duplicate row");
    assert_eq!(
        keys[0]["masked_key"].as_str().unwrap(),
        "bbbb***",
        "second save must overwrite the first"
    );
}

#[tokio::test]
async fn test_delete_user_api_key_removes_key_and_is_idempotent() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_providers::create"],
    )
    .await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let provider = create_test_provider_with_key(&server, &admin.token, "OpenAI", None).await;
    let provider_id = provider["id"].as_str().unwrap();
    let client = reqwest::Client::new();

    // Save then delete.
    client
        .post(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "provider_id": provider_id, "api_key": "sk-x" }))
        .send()
        .await
        .unwrap();

    let delete1 = client
        .delete(server.api_url(&format!("/user-llm-providers/api-keys/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(delete1.status(), StatusCode::NO_CONTENT);

    // List should be empty.
    let list = client
        .get(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = list.json().await.unwrap();
    assert!(body["keys"].as_array().unwrap().is_empty());

    // Second delete must also succeed (idempotent).
    let delete2 = client
        .delete(server.api_url(&format!("/user-llm-providers/api-keys/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        delete2.status(),
        StatusCode::NO_CONTENT,
        "delete must be idempotent (NO_CONTENT even when no key exists)"
    );
}

// ---------- Validation errors (3 tests) ----------

#[tokio::test]
async fn test_save_user_api_key_rejects_empty_key() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let response = reqwest::Client::new()
        .post(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "provider_id": Uuid::new_v4(), "api_key": "" }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body.get("error_code").and_then(|v| v.as_str()),
        Some("VALIDATION_ERROR")
    );
}

#[tokio::test]
async fn test_save_user_api_key_rejects_oversized_key() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let response = reqwest::Client::new()
        .post(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "provider_id": Uuid::new_v4(),
            "api_key": "x".repeat(501),
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body.get("error_code").and_then(|v| v.as_str()),
        Some("VALIDATION_ERROR")
    );
}

#[tokio::test]
async fn test_save_user_api_key_control_char_handling() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_providers::create"],
    )
    .await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let provider = create_test_provider_with_key(&server, &admin.token, "OpenAI", None).await;
    let provider_id = provider["id"].as_str().unwrap();
    let client = reqwest::Client::new();

    // Control char other than \t is rejected.
    let r_bad = client
        .post(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "provider_id": provider_id,
            "api_key": "sk-bad\u{01}",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r_bad.status(), StatusCode::BAD_REQUEST);
    let body: serde_json::Value = r_bad.json().await.unwrap();
    assert_eq!(
        body.get("error_code").and_then(|v| v.as_str()),
        Some("VALIDATION_ERROR")
    );

    // \t (tab) is the documented allowed exception.
    let r_ok = client
        .post(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "provider_id": provider_id,
            "api_key": "sk-ok\there",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        r_ok.status(),
        StatusCode::NO_CONTENT,
        "\\t must be an allowed exception in the control-char filter"
    );
}

// ---------- `api_key_configured` flag logic (2 tests) ----------

#[tokio::test]
async fn test_get_user_providers_flag_true_with_system_key() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_providers::create", "groups::edit", "groups::read"],
    )
    .await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    // Admin creates a provider WITH a system api_key.
    let provider =
        create_test_provider_with_key(&server, &admin.token, "WithKey", Some("sk-admin-key")).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Assign provider to default users group so the user sees it.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO user_group_llm_providers (id, group_id, provider_id)
         SELECT gen_random_uuid(), id, $1 FROM groups WHERE is_default = true
         ON CONFLICT DO NOTHING",
    )
    .bind(Uuid::parse_str(provider_id).unwrap())
    .execute(&pool)
    .await
    .expect("Failed to assign provider to default group");

    let r = reqwest::Client::new()
        .get(server.api_url("/user-llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: serde_json::Value = r.json().await.unwrap();
    let providers = body["providers"].as_array().unwrap();
    // ProviderWithModels uses #[serde(flatten)] on the inner LlmProvider, so the
    // fields (id, name, etc.) live at the top level alongside api_key_configured
    // and llm_models — there is no `provider` sub-object in the JSON.
    let our = providers
        .iter()
        .find(|p| p["id"].as_str() == Some(provider_id))
        .expect("provider must be visible to the user");
    assert_eq!(
        our["api_key_configured"].as_bool(),
        Some(true),
        "system key set → api_key_configured must be true even without a user key"
    );
}

#[tokio::test]
async fn test_get_user_providers_flag_with_and_without_user_key() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_providers::create"],
    )
    .await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    // Admin creates a provider with NO system key.
    let provider =
        create_test_provider_with_key(&server, &admin.token, "NoSystemKey", None).await;
    let provider_id = provider["id"].as_str().unwrap();

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO user_group_llm_providers (id, group_id, provider_id)
         SELECT gen_random_uuid(), id, $1 FROM groups WHERE is_default = true
         ON CONFLICT DO NOTHING",
    )
    .bind(Uuid::parse_str(provider_id).unwrap())
    .execute(&pool)
    .await
    .expect("Failed to assign provider to default group");

    let client = reqwest::Client::new();

    // BEFORE any user key: flag should be false (no system key, no user key).
    let r_before = client
        .get(server.api_url("/user-llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    let body_before: serde_json::Value = r_before.json().await.unwrap();
    let p_before = body_before["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"].as_str() == Some(provider_id))
        .expect("provider visible");
    assert_eq!(
        p_before["api_key_configured"].as_bool(),
        Some(false),
        "no system + no user key → flag must be false"
    );

    // Save a personal key.
    client
        .post(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "provider_id": provider_id, "api_key": "sk-mine" }))
        .send()
        .await
        .unwrap();

    // AFTER user key: flag must flip to true even though system key remains empty.
    let r_after = client
        .get(server.api_url("/user-llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    let body_after: serde_json::Value = r_after.json().await.unwrap();
    let p_after = body_after["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"].as_str() == Some(provider_id))
        .expect("provider visible");
    assert_eq!(
        p_after["api_key_configured"].as_bool(),
        Some(true),
        "user key set → flag must be true"
    );
}

// ---------- Local providers: keyless proxy-token mint + no API key required ----------

/// The boot-time reseed mints AND persists a proxy token for any local provider
/// that has no usable api_key — NULL or blank — so it can authenticate proxy
/// requests. After reseed each token is (a) persisted in the DB, (b) resolvable,
/// (c) present in the proxy token cache, and the operation is idempotent across
/// reseeds (no re-mint). Uses freshly-inserted providers (not the seeded
/// built-in) so the subprocess boot reseed — which runs once before these rows
/// exist — cannot race the assertions.
#[tokio::test]
async fn test_reseed_mints_and_persists_token_for_keyless_local_provider() {
    use sqlx::Row;

    let server = crate::common::TestServer::start().await;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();

    // Two keyless local providers — one with a NULL api_key, one with a BLANK
    // string — both of which must take the mint path. Inserted AFTER boot so the
    // fire-and-forget boot reseed (which only saw the seeded built-in) can't
    // touch them, making the mint path deterministic.
    let null_id = Uuid::new_v4();
    let blank_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO llm_providers (id, name, provider_type, enabled, built_in)
         VALUES ($1, 'Reseed Null Local', 'local', false, false)",
    )
    .bind(null_id)
    .execute(&pool)
    .await
    .expect("insert NULL-key local provider");
    sqlx::query(
        "INSERT INTO llm_providers (id, name, provider_type, enabled, built_in, api_key)
         VALUES ($1, 'Reseed Blank Local', 'local', false, false, '')",
    )
    .bind(blank_id)
    .execute(&pool)
    .await
    .expect("insert blank-key local provider");

    // Drive reseed explicitly (boot reseed is fire-and-forget → not deterministic).
    ziee::test_internals::proxy_reseed_from_db(&pool)
        .await
        .expect("reseed should succeed");

    // Assert a provider got a token that is persisted, resolvable, and accepted
    // by the proxy cache; return the persisted columns for the idempotency check.
    async fn assert_minted(pool: &sqlx::PgPool, id: Uuid) -> (Option<String>, Option<Vec<u8>>) {
        use sqlx::Row;
        let row = sqlx::query("SELECT api_key, api_key_encrypted FROM llm_providers WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap();
        let plain: Option<String> = row.try_get("api_key").unwrap();
        let enc: Option<Vec<u8>> = row.try_get("api_key_encrypted").unwrap();
        assert!(
            plain
                .as_deref()
                .map(str::trim)
                .map(|s| !s.is_empty())
                .unwrap_or(false)
                || enc.as_ref().map(|e| !e.is_empty()).unwrap_or(false),
            "reseed must persist a minted token for keyless local provider {id}"
        );
        let token =
            ziee::resolve_optional_secret(pool, enc.clone(), plain.clone())
                .await
                .expect("minted token must resolve");
        assert!(!token.trim().is_empty(), "resolved token must be non-empty");
        assert_eq!(
            ziee::test_internals::proxy_lookup_token(&token).await,
            Some(id),
            "minted token must authenticate against the proxy cache"
        );
        (plain, enc)
    }

    let null_after = assert_minted(&pool, null_id).await;
    let blank_after = assert_minted(&pool, blank_id).await;

    // Idempotency: a second reseed must NOT re-mint (key material unchanged).
    ziee::test_internals::proxy_reseed_from_db(&pool)
        .await
        .expect("second reseed should succeed");
    for (id, (plain, enc)) in [(null_id, null_after), (blank_id, blank_after)] {
        let row = sqlx::query("SELECT api_key, api_key_encrypted FROM llm_providers WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let again_plain: Option<String> = row.try_get("api_key").unwrap();
        let again_enc: Option<Vec<u8>> = row.try_get("api_key_encrypted").unwrap();
        assert_eq!(again_plain, plain, "api_key must be stable across reseeds for {id}");
        assert_eq!(again_enc, enc, "api_key_encrypted must be stable across reseeds for {id}");
    }
}

/// A local provider reports `api_key_configured = true` to the user WITHOUT any
/// system- or user-supplied API key (the reseed minted an internal proxy token).
/// This is what stops the chat model selector from prompting for a key.
#[tokio::test]
async fn test_get_user_providers_local_reports_configured_without_user_key() {
    use sqlx::Row;

    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();

    // Enable the built-in keyless `Local` provider and make it visible to the user.
    let row = sqlx::query(
        "UPDATE llm_providers SET enabled = true
         WHERE provider_type = 'local' AND built_in = true
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("built-in local provider must exist");
    let provider_id: Uuid = row.try_get("id").unwrap();
    let pid_str = provider_id.to_string();

    sqlx::query(
        "INSERT INTO user_group_llm_providers (id, group_id, provider_id)
         SELECT gen_random_uuid(), id, $1 FROM groups WHERE is_default = true
         ON CONFLICT DO NOTHING",
    )
    .bind(provider_id)
    .execute(&pool)
    .await
    .expect("assign local provider to default group");

    // Persist a minted proxy token to the provider's DB row. `api_key_configured`
    // is computed from the DB system key (not the in-process cache), so the
    // DB persist — not the cache insert — is what this assertion depends on.
    ziee::test_internals::proxy_reseed_from_db(&pool).await.unwrap();

    let r = reqwest::Client::new()
        .get(server.api_url("/user-llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: serde_json::Value = r.json().await.unwrap();
    let our = body["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"].as_str() == Some(pid_str.as_str()))
        .expect("local provider must be visible to the user");
    assert_eq!(our["provider_type"].as_str(), Some("local"));
    assert_eq!(
        our["api_key_configured"].as_bool(),
        Some(true),
        "local provider must report api_key_configured = true without any user/system key entry"
    );
}

/// Saving a USER API key for a local provider is rejected server-side. A stored
/// user key would be sent to the local proxy as the bearer and rejected (it
/// isn't the minted proxy token), breaking local inference.
#[tokio::test]
async fn test_save_user_api_key_rejected_for_local_provider() {
    use sqlx::Row;

    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let row = sqlx::query(
        "SELECT id FROM llm_providers WHERE provider_type = 'local' AND built_in = true LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .expect("built-in local provider must exist");
    let provider_id: Uuid = row.try_get("id").unwrap();

    let r = reqwest::Client::new()
        .post(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "provider_id": provider_id.to_string(), "api_key": "sk-should-be-rejected" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(
        body.get("error_code").and_then(|v| v.as_str()),
        Some("PROVIDER_IS_LOCAL"),
        "expected PROVIDER_IS_LOCAL error_code, got: {body}"
    );
}

/// A missing provider_id is rejected with 404 BEFORE any key is stored — pins
/// the new lookup ordering (the existence check happens before the upsert) added
/// alongside the local-provider guard.
#[tokio::test]
async fn test_save_user_api_key_missing_provider_returns_404() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let r = reqwest::Client::new()
        .post(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "provider_id": Uuid::new_v4(), "api_key": "sk-valid-key" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

/// The admin update path (POST /llm-providers/{id}) also refuses to set an
/// api_key on a local provider — symmetric to the user-key guard. The proxy
/// token is server-minted and changed via the rotate-proxy-token endpoint, so
/// accepting an api_key here would desync the DB from the in-memory cache.
#[tokio::test]
async fn test_update_provider_api_key_rejected_for_local() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;
    let client = reqwest::Client::new();

    // Create a local provider.
    let resp = client
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "name": "Local Update Guard", "provider_type": "local", "enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Setting an api_key on it must be rejected with PROVIDER_IS_LOCAL.
    let r = client
        .post(server.api_url(&format!("/llm-providers/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "api_key": "sk-should-be-rejected" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(
        body.get("error_code").and_then(|v| v.as_str()),
        Some("PROVIDER_IS_LOCAL"),
        "expected PROVIDER_IS_LOCAL error_code, got: {body}"
    );

    // A local edit WITHOUT an api_key must still succeed (no false 400).
    let ok = client
        .post(server.api_url(&format!("/llm-providers/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "enabled": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::OK);
}

// ---------- Cross-user isolation (1 test) ----------

#[tokio::test]
async fn test_user_keys_are_isolated_between_users() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_providers::create"],
    )
    .await;
    let user_a = crate::common::test_helpers::create_user_with_permissions(&server, "alice", &[]).await;
    let user_b = crate::common::test_helpers::create_user_with_permissions(&server, "bob", &[]).await;

    let provider = create_test_provider_with_key(&server, &admin.token, "Shared", None).await;
    let provider_id = provider["id"].as_str().unwrap();

    let client = reqwest::Client::new();

    // A and B each save their own key for the same provider.
    for (token, key) in &[
        (&user_a.token, "aaaa-alice"),
        (&user_b.token, "bbbb-bob"),
    ] {
        let r = client
            .post(server.api_url("/user-llm-providers/api-keys"))
            .header("Authorization", format!("Bearer {}", token))
            .json(&json!({ "provider_id": provider_id, "api_key": key }))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::NO_CONTENT);
    }

    // A sees only A's masked key.
    let list_a: serde_json::Value = client
        .get(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", user_a.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let keys_a = list_a["keys"].as_array().unwrap();
    assert_eq!(keys_a.len(), 1);
    assert_eq!(keys_a[0]["masked_key"].as_str().unwrap(), "aaaa***");

    // B sees only B's masked key.
    let list_b: serde_json::Value = client
        .get(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", user_b.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let keys_b = list_b["keys"].as_array().unwrap();
    assert_eq!(keys_b.len(), 1);
    assert_eq!(keys_b[0]["masked_key"].as_str().unwrap(), "bbbb***");

    // A deletes — must not affect B.
    client
        .delete(server.api_url(&format!("/user-llm-providers/api-keys/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user_a.token))
        .send()
        .await
        .unwrap();

    let list_b_after: serde_json::Value = client
        .get(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", user_b.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        list_b_after["keys"].as_array().unwrap().len(),
        1,
        "B's key must survive A's delete (cross-user isolation)"
    );
}

/// Build a UserKeyRepository talking directly to the test server's database
/// — bypasses the global `Repos` (which lives in a different process from
/// the integration tests).
async fn test_user_key_repo(server: &crate::common::TestServer) -> UserKeyRepository {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");
    UserKeyRepository::new(pool)
}

/// User has a personal key → it wins, even if a system key is set.
#[tokio::test]
async fn test_resolve_api_key_user_key_wins_over_system_key() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_providers::create"],
    )
    .await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    // Provider WITH a system key.
    let provider = create_test_provider_with_key(
        &server,
        &admin.token,
        "WithSysKey",
        Some("sk-system-admin"),
    )
    .await;
    let provider_id = provider["id"].as_str().unwrap();

    // User saves their personal key.
    reqwest::Client::new()
        .post(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({
            "provider_id": provider_id,
            "api_key": "sk-user-personal",
        }))
        .send()
        .await
        .unwrap();

    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();
    let provider_uuid = Uuid::parse_str(provider_id).unwrap();

    let repo = test_user_key_repo(&server).await;
    let resolved = resolve_api_key_for_user(
        &repo,
        user_uuid,
        provider_uuid,
        Some("sk-system-admin".to_string()),
    )
    .await
    .expect("resolution must not error");

    assert_eq!(
        resolved, "sk-user-personal",
        "user's personal key must take precedence over the system key"
    );
}

/// User has no personal key → system key is used.
#[tokio::test]
async fn test_resolve_api_key_falls_back_to_system_key() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    // No user key saved. Random provider_id (resolution shouldn't depend on
    // whether the provider row exists — it only reads user_llm_provider_api_keys).
    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();
    let provider_uuid = Uuid::new_v4();

    let repo = test_user_key_repo(&server).await;
    let resolved = resolve_api_key_for_user(
        &repo,
        user_uuid,
        provider_uuid,
        Some("sk-system-fallback".to_string()),
    )
    .await
    .expect("resolution must not error");

    assert_eq!(
        resolved, "sk-system-fallback",
        "with no user key, the resolver must return the supplied system key"
    );
}

/// User has a personal key and the system key is None → user key wins.
#[tokio::test]
async fn test_resolve_api_key_user_key_used_when_no_system_key() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_providers::create"],
    )
    .await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let provider = create_test_provider_with_key(&server, &admin.token, "NoSysKey", None).await;
    let provider_id = provider["id"].as_str().unwrap();

    reqwest::Client::new()
        .post(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({
            "provider_id": provider_id,
            "api_key": "sk-only-user",
        }))
        .send()
        .await
        .unwrap();

    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();
    let provider_uuid = Uuid::parse_str(provider_id).unwrap();

    let repo = test_user_key_repo(&server).await;
    let resolved = resolve_api_key_for_user(&repo, user_uuid, provider_uuid, None)
        .await
        .expect("resolution must not error");

    assert_eq!(
        resolved, "sk-only-user",
        "with no system key, the user's personal key must be returned"
    );
}

/// Neither user nor system key → resolver returns an empty string
/// (some provider types — `local`, `custom` — accept this and don't authenticate).
#[tokio::test]
async fn test_resolve_api_key_returns_empty_string_when_no_keys() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();

    let repo = test_user_key_repo(&server).await;
    let resolved = resolve_api_key_for_user(&repo, user_uuid, Uuid::new_v4(), None)
        .await
        .expect("resolution must not error even when both keys are absent");

    assert_eq!(
        resolved, "",
        "with no user key and no system key, resolver must return an empty string"
    );
}

/// Cross-user isolation at the resolver level: a key saved by user A must
/// not leak to user B's resolution.
#[tokio::test]
async fn test_resolve_api_key_is_isolated_between_users() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_providers::create"],
    )
    .await;
    let user_a =
        crate::common::test_helpers::create_user_with_permissions(&server, "alice", &[]).await;
    let user_b =
        crate::common::test_helpers::create_user_with_permissions(&server, "bob", &[]).await;

    let provider = create_test_provider_with_key(&server, &admin.token, "Shared", None).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Only A saves a key.
    reqwest::Client::new()
        .post(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", user_a.token))
        .json(&serde_json::json!({
            "provider_id": provider_id,
            "api_key": "sk-alice-only",
        }))
        .send()
        .await
        .unwrap();

    let a_uuid = Uuid::parse_str(&user_a.user_id).unwrap();
    let b_uuid = Uuid::parse_str(&user_b.user_id).unwrap();
    let provider_uuid = Uuid::parse_str(provider_id).unwrap();
    let system_key = Some("sk-system".to_string());

    let repo = test_user_key_repo(&server).await;
    let resolved_for_a =
        resolve_api_key_for_user(&repo, a_uuid, provider_uuid, system_key.clone()).await.unwrap();
    let resolved_for_b =
        resolve_api_key_for_user(&repo, b_uuid, provider_uuid, system_key.clone()).await.unwrap();

    assert_eq!(resolved_for_a, "sk-alice-only", "A's resolution should return A's key");
    assert_eq!(
        resolved_for_b, "sk-system",
        "B has no personal key — resolution must fall through to system key"
    );
}

// =====================================================
// SSRF regression tests — close 06-llm-provider F-03
// =====================================================

async fn create_provider_with_base_url(
    server: &crate::common::TestServer,
    admin_token: &str,
    bad_url: &str,
) -> reqwest::Response {
    reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", admin_token))
        .json(&json!({
            "name": format!("ssrf-test-{}", Uuid::new_v4()),
            "provider_type": "openai",
            "base_url": bad_url,
            "enabled": false,
        }))
        .send()
        .await
        .expect("request failed")
}

#[tokio::test]
async fn test_ssrf_provider_rejects_aws_imds_base_url() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_providers::create"],
    )
    .await;
    let res = create_provider_with_base_url(
        &server,
        &admin.token,
        "http://169.254.169.254/v1",
    )
    .await;
    assert_eq!(res.status(), 400, "AWS IMDS base_url must be rejected");
}

#[tokio::test]
async fn test_ssrf_provider_accepts_loopback_base_url() {
    // INTENTIONAL: loopback URLs are ALLOWED on provider base_url
    // because local LLM providers (llama.cpp, mistralrs at
    // http://localhost:1234/v1) are a legitimate first-class use case.
    // The validator uses OutboundUrlPolicy::DEV_LOCAL which lets
    // localhost through but still blocks RFC 1918, link-local (AWS
    // IMDS), ULA, CGNAT, and non-HTTP schemes. The provider-create
    // endpoint is admin-only so the "admin probes localhost services"
    // risk is gated by trust.
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_providers::create"],
    )
    .await;
    let res = create_provider_with_base_url(&server, &admin.token, "http://127.0.0.1:8000/v1").await;
    assert!(
        res.status().is_success(),
        "loopback base_url must be accepted (local providers); got {}",
        res.status()
    );
}

#[tokio::test]
async fn test_ssrf_provider_rejects_file_scheme_base_url() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_providers::create"],
    )
    .await;
    let res = create_provider_with_base_url(&server, &admin.token, "file:///etc/passwd").await;
    assert_eq!(res.status(), 400, "file:// base_url must be rejected");
}

#[tokio::test]
async fn test_ssrf_provider_rejects_rfc1918_base_url() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_providers::create"],
    )
    .await;
    let res = create_provider_with_base_url(&server, &admin.token, "http://192.168.1.1/v1").await;
    assert_eq!(res.status(), 400, "RFC 1918 base_url must be rejected");
}

// audit id all-f676bb31f850 — the discover_models endpoint
// (GET /llm-providers/{id}/discover-models) was completely untested. It returns
// the curated catalog (source="catalog") for a remote provider and, with no
// base_url, notes the skipped live /v1/models call; a local provider returns no
// models and a note pointing at /api/llm-models. No network needed for either.
#[tokio::test]
async fn test_discover_models_catalog_and_local_paths() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "discover_user",
        &["llm_providers::read", "llm_providers::create"],
    )
    .await;

    // (a) openai provider, NO base_url → catalog models + skipped-live note.
    let provider = create_test_provider(&server, &user.token).await;
    let pid = provider["id"].as_str().unwrap();
    let body: serde_json::Value = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-providers/{pid}/discover-models")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["provider_type"], "openai");
    let models = body["models"].as_array().expect("models array");
    assert!(!models.is_empty(), "openai catalog must surface known models: {body}");
    assert!(
        models.iter().all(|m| m["source"] == "catalog"),
        "with no base_url every model comes from the catalog: {body}"
    );
    let notes = body["notes"].as_array().unwrap();
    assert!(
        notes.iter().any(|n| n.as_str().unwrap_or("").contains("base_url")),
        "must note the skipped live /v1/models call: {body}"
    );

    // (b) local provider → no models + a note pointing at /api/llm-models.
    let local: serde_json::Value = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "name": "Local P", "provider_type": "local", "enabled": false }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let lid = local["id"].as_str().unwrap();
    let lbody: serde_json::Value = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-providers/{lid}/discover-models")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(lbody["provider_type"], "local");
    assert!(lbody["models"].as_array().unwrap().is_empty(), "local: no discovered models: {lbody}");
    assert!(
        lbody["notes"].as_array().unwrap().iter().any(|n| n.as_str().unwrap_or("").contains("llm-models")),
        "local must point at /api/llm-models: {lbody}"
    );
}

/// Concurrency: multiple admins assigning the SAME provider→group at the same
/// time must converge to exactly ONE assignment (no duplicate rows, no lost
/// update, no 500). The existing idempotent test only assigns sequentially.
#[tokio::test]
async fn test_concurrent_provider_group_assignment_is_safe() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "concurrent_assign",
        &[
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::assign_groups",
            "groups::read",
        ],
    )
    .await;

    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap().to_string();
    let admin_group_id = get_admin_group_id(&server, &user.token).await;

    // Fire N identical assignments concurrently (simulating several admins).
    let url = server.api_url(&format!("/llm-providers/{}/groups", provider_id));
    let mut handles = Vec::new();
    for _ in 0..6 {
        let url = url.clone();
        let token = user.token.clone();
        let gid = admin_group_id.clone();
        handles.push(tokio::spawn(async move {
            reqwest::Client::new()
                .post(&url)
                .header("Authorization", format!("Bearer {}", token))
                .json(&json!({ "group_id": gid }))
                .send()
                .await
                .unwrap()
                .status()
        }));
    }
    for h in handles {
        let status = h.await.unwrap();
        assert_eq!(
            status,
            StatusCode::NO_CONTENT,
            "every concurrent assign must succeed, got {status}"
        );
    }

    // Exactly one assignment survives — no duplicate rows from the race.
    let body: serde_json::Value = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-providers/{}/groups", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        body.as_array().unwrap().len(),
        1,
        "concurrent identical assigns must converge to a single assignment"
    );
}

#[tokio::test]
async fn test_discover_models_not_found_provider() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "discover_404",
        &["llm_providers::read"],
    )
    .await;
    let missing = uuid::Uuid::new_v4();
    let res = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-providers/{missing}/discover-models")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_discover_models_local_provider_returns_note_no_network() {
    // A `local` provider short-circuits before any live /v1/models call: it
    // returns an empty model list + a note pointing at /api/llm-models. This
    // exercises the discover handler end-to-end without touching the network.
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "discover_local",
        &["llm_providers::read", "llm_providers::create"],
    )
    .await;

    let provider: serde_json::Value = {
        let res = reqwest::Client::new()
            .post(server.api_url("/llm-providers"))
            .header("Authorization", format!("Bearer {}", user.token))
            .json(&json!({
                "name": "Local Runtime",
                "provider_type": "local",
                "enabled": false
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        res.json().await.unwrap()
    };
    let provider_id = provider["id"].as_str().unwrap();

    let res = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-providers/{provider_id}/discover-models")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["provider_type"], "local");
    assert_eq!(body["models"].as_array().unwrap().len(), 0);
    let notes = body["notes"].as_array().unwrap();
    assert!(
        notes.iter().any(|n| n.as_str().unwrap_or("").contains("llm-models")),
        "expected a note redirecting to /api/llm-models; got {body}"
    );
}

/// discover-models on a NON-local provider exercises Layer 1 (the curated
/// catalog) AND the live `/v1/models` error-fallback branch — deterministically
/// and WITHOUT real network: the base_url points at loopback, which the
/// outbound-URL SSRF policy rejects up front, so `fetch_v1_models` fails fast
/// and the handler falls back to catalog-only with an explanatory note. (The
/// existing discover tests only cover the 404 + local short-circuit paths.)
#[tokio::test]
async fn test_discover_models_openai_catalog_with_live_call_fallback() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "discover_openai",
        &["llm_providers::read", "llm_providers::create"],
    )
    .await;

    // A loopback base_url is blocked by the PUBLIC_HTTP_OR_HTTPS SSRF policy
    // before any socket is opened → the live call errors instantly (no network).
    let provider: serde_json::Value = {
        let res = reqwest::Client::new()
            .post(server.api_url("/llm-providers"))
            .header("Authorization", format!("Bearer {}", user.token))
            .json(&json!({
                "name": "OpenAI Compatible",
                "provider_type": "openai",
                "api_key": "sk-test-dummy",
                "base_url": "http://127.0.0.1:9/v1",
                "enabled": false
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CREATED, "create: {}", res.status());
        res.json().await.unwrap()
    };
    let provider_id = provider["id"].as_str().unwrap();

    let res = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-providers/{provider_id}/discover-models")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["provider_type"], "openai");

    // Layer 1: the curated openai catalog populates models, all sourced
    // "catalog" (the live augment failed, so nothing is "discovery").
    let models = body["models"].as_array().unwrap();
    assert!(!models.is_empty(), "openai catalog must yield models: {body}");
    assert!(
        models.iter().all(|m| m["source"] == "catalog"),
        "all models must come from the catalog when the live call fails: {body}"
    );

    // The live-call failure is surfaced as a note (catalog-only fallback).
    let notes = body["notes"].as_array().unwrap();
    assert!(
        notes
            .iter()
            .any(|n| n.as_str().unwrap_or("").contains("live /v1/models call failed")),
        "expected a live-call fallback note; got {body}"
    );
}

/// Concurrency: many admins assigning the SAME provider to the SAME group at
/// once must converge to a single assignment row (the `ON CONFLICT` upsert is
/// race-safe), with every concurrent request succeeding — no duplicate rows,
/// no 5xx. The existing idempotent test only fires sequentially. Mirrors the
/// `llm_provider_files` concurrent-upsert convergence test.
#[tokio::test]
async fn test_concurrent_provider_group_assignment_converges_to_one_row() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "concurrent_assign",
        &[
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::assign_groups",
            "groups::read",
        ],
    )
    .await;

    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap().to_string();
    let admin_group_id = get_admin_group_id(&server, &user.token).await;

    // Fire N concurrent identical assign requests.
    let url = server.api_url(&format!("/llm-providers/{provider_id}/groups"));
    let mut handles = Vec::new();
    for _ in 0..8 {
        let url = url.clone();
        let token = user.token.clone();
        let group = admin_group_id.clone();
        handles.push(tokio::spawn(async move {
            reqwest::Client::new()
                .post(&url)
                .header("Authorization", format!("Bearer {token}"))
                .json(&json!({ "group_id": group }))
                .send()
                .await
                .unwrap()
                .status()
        }));
    }
    for h in handles {
        let status = h.await.unwrap();
        assert!(
            status == StatusCode::NO_CONTENT || status == StatusCode::CONFLICT,
            "each concurrent assign must succeed or be a benign conflict, got {status}"
        );
    }

    // Exactly one assignment row survives the race.
    let body: serde_json::Value = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        body.as_array().unwrap().len(),
        1,
        "concurrent assigns must converge to a single row: {body}"
    );
}

