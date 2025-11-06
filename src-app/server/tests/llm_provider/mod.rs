// Integration tests for LLM Provider module

use serde_json::json;
use reqwest::StatusCode;
use uuid::Uuid;

// =====================================================
// Permission Tests
// =====================================================

#[tokio::test]
async fn test_list_providers_requires_read_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let response = reqwest::Client::new()
        .get(&server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_list_providers_with_read_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::read"]).await;

    let response = reqwest::Client::new()
        .get(&server.api_url("/llm-providers"))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let provider_id = Uuid::new_v4();
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_create_provider_requires_create_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::read"]).await;

    let payload = json!({
        "name": "Test Provider",
        "provider_type": "openai",
        "enabled": false
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-providers"))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::read", "llm_providers::create"]).await;

    // Create a provider first
    let provider = create_test_provider(&server, &user.token).await;

    // Try to update without edit permission
    let payload = json!({
        "name": "Updated Name"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-providers/{}", provider["id"])))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::read", "llm_providers::create"]).await;

    // Create a provider first
    let provider = create_test_provider(&server, &user.token).await;

    // Try to delete without delete permission
    let response = reqwest::Client::new()
        .delete(&server.api_url(&format!("/llm-providers/{}", provider["id"])))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_assign_provider_to_group_requires_assign_groups_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::read"]).await;

    let provider_id = Uuid::new_v4();
    let payload = json!({
        "group_id": Uuid::new_v4().to_string()
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-providers/{}/groups", provider_id)))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::read"]).await;

    // Test default pagination
    let response = reqwest::Client::new()
        .get(&server.api_url("/llm-providers"))
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
        .get(&server.api_url("/llm-providers?page=1&per_page=3"))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::read", "llm_providers::create"]).await;

    // Create a provider
    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Get the provider
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/llm-providers/{}", provider_id)))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::read"]).await;

    let provider_id = Uuid::new_v4();
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_create_provider_minimal() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::create"]).await;

    let payload = json!({
        "name": "Minimal Provider",
        "provider_type": "anthropic",
        "enabled": false
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-providers"))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::create"]).await;

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
        .post(&server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["name"], "Full Provider");
    assert_eq!(body["api_key"], "sk-test123");
    assert_eq!(body["base_url"], "https://api.openai.com/v1");
    assert_eq!(body["proxy_settings"]["enabled"], true);
    assert_eq!(body["proxy_settings"]["url"], "http://proxy.example.com:8080");
}

#[tokio::test]
async fn test_update_provider_name() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::read", "llm_providers::create", "llm_providers::edit"]).await;

    // Create a provider
    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Update name
    let payload = json!({
        "name": "Updated Provider Name"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-providers/{}", provider_id)))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::read", "llm_providers::create", "llm_providers::edit"]).await;

    // Create a provider
    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Update enabled and api_key
    let payload = json!({
        "enabled": true,
        "api_key": "sk-newkey456"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["enabled"], true);
    assert_eq!(body["api_key"], "sk-newkey456");
}

#[tokio::test]
async fn test_update_provider_proxy_settings() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::read", "llm_providers::create", "llm_providers::edit"]).await;

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
        .post(&server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["proxy_settings"]["enabled"], true);
    assert_eq!(body["proxy_settings"]["url"], "http://newproxy.example.com:3128");
}

#[tokio::test]
async fn test_update_provider_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::edit"]).await;

    let provider_id = Uuid::new_v4();
    let payload = json!({
        "name": "Updated Name"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-providers/{}", provider_id)))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::read", "llm_providers::create", "llm_providers::delete"]).await;

    // Create a provider
    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Delete it
    let response = reqwest::Client::new()
        .delete(&server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify it's gone
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_built_in_provider_fails() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::read", "llm_providers::delete"]).await;

    // Get a built-in provider (OpenAI)
    let response = reqwest::Client::new()
        .get(&server.api_url("/llm-providers?page=1&per_page=100"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = response.json().await.unwrap();
    let providers = body["providers"].as_array().unwrap();
    let openai_provider = providers.iter()
        .find(|p| p["name"] == "OpenAI")
        .expect("OpenAI built-in provider should exist");
    let provider_id = openai_provider["id"].as_str().unwrap();

    // Try to delete it
    let response = reqwest::Client::new()
        .delete(&server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_delete_provider_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::delete"]).await;

    let provider_id = Uuid::new_v4();
    let response = reqwest::Client::new()
        .delete(&server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =====================================================
// Validation Tests
// =====================================================

#[tokio::test]
async fn test_create_provider_invalid_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::create"]).await;

    let payload = json!({
        "name": "Invalid Provider",
        "provider_type": "invalid_type",
        "enabled": false
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-providers"))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::create"]).await;

    let payload = json!({
        "name": "",
        "provider_type": "openai",
        "enabled": false
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-providers"))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::create"]).await;

    let payload = json!({
        "name": "Invalid URL Provider",
        "provider_type": "openai",
        "enabled": false,
        "base_url": "not-a-valid-url"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_enabled_remote_provider_requires_api_key() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::create"]).await;

    let payload = json!({
        "name": "Enabled Without Key",
        "provider_type": "openai",
        "enabled": true
        // No api_key provided
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_local_provider_no_api_key_required() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::create"]).await;

    let payload = json!({
        "name": "Local Provider",
        "provider_type": "local",
        "enabled": true,
        "base_url": "http://localhost:1234/v1"
        // No api_key - should be OK for local providers
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-providers"))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::read", "llm_providers::create", "llm_providers::edit"]).await;

    // Create a provider
    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Try to update with empty name
    let payload = json!({
        "name": ""
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-providers/{}", provider_id)))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::read", "llm_providers::create", "llm_providers::edit"]).await;

    // Create a provider
    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Try to update with invalid base_url
    let payload = json!({
        "base_url": "invalid-url"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-providers/{}", provider_id)))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::read", "llm_providers::create"]).await;

    // Create a provider
    let provider = create_test_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Get groups (should be empty initially)
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/llm-providers/{}/groups", provider_id)))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[
        "llm_providers::read",
        "llm_providers::create",
        "llm_providers::assign_groups",
        "groups::read"
    ]).await;

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
        .post(&server.api_url(&format!("/llm-providers/{}/groups", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify assignment
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/llm-providers/{}/groups", provider_id)))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[
        "llm_providers::read",
        "llm_providers::create",
        "llm_providers::assign_groups",
        "groups::read"
    ]).await;

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
        .post(&server.api_url(&format!("/llm-providers/{}/groups", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response1.status(), StatusCode::NO_CONTENT);

    let response2 = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-providers/{}/groups", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response2.status(), StatusCode::NO_CONTENT);

    // Should still have only one assignment
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/llm-providers/{}/groups", provider_id)))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[
        "llm_providers::read",
        "llm_providers::create",
        "llm_providers::assign_groups",
        "groups::read"
    ]).await;

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
        .post(&server.api_url(&format!("/llm-providers/{}/groups", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // Remove assignment
    let response = reqwest::Client::new()
        .delete(&server.api_url(&format!("/llm-providers/{}/groups/{}", provider_id, admin_group_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify it's removed
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/llm-providers/{}/groups", provider_id)))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[
        "llm_providers::assign_groups",
        "groups::read"
    ]).await;

    let provider_id = Uuid::new_v4();
    let group_id = Uuid::new_v4();

    let response = reqwest::Client::new()
        .delete(&server.api_url(&format!("/llm-providers/{}/groups/{}", provider_id, group_id)))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[
        "llm_providers::read",
        "groups::read"
    ]).await;

    // Get admin group
    let admin_group_id = get_admin_group_id(&server, &user.token).await;

    // Get providers for group (should be empty initially)
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/groups/{}/providers", admin_group_id)))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[
        "llm_providers::read",
        "llm_providers::create",
        "llm_providers::assign_groups",
        "groups::read"
    ]).await;

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
        .put(&server.api_url(&format!("/groups/{}/providers", admin_group_id)))
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
        .put(&server.api_url(&format!("/groups/{}/providers", admin_group_id)))
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
    let provider_ids: Vec<String> = providers.iter()
        .map(|p| p["id"].as_str().unwrap().to_string())
        .collect();
    assert!(provider_ids.contains(&provider_id2.to_string()));
    assert!(provider_ids.contains(&provider_id3.to_string()));
    assert!(!provider_ids.contains(&provider_id1.to_string()));
}

#[tokio::test]
async fn test_update_group_providers_empty_list() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[
        "llm_providers::read",
        "llm_providers::create",
        "llm_providers::assign_groups",
        "groups::read"
    ]).await;

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
        .put(&server.api_url(&format!("/groups/{}/providers", admin_group_id)))
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
        .put(&server.api_url(&format!("/groups/{}/providers", admin_group_id)))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[
        "llm_providers::read",
        "groups::read"
    ]).await;

    // Get admin group
    let admin_group_id = get_admin_group_id(&server, &user.token).await;

    let payload = json!({
        "provider_ids": []
    });

    let response = reqwest::Client::new()
        .put(&server.api_url(&format!("/groups/{}/providers", admin_group_id)))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let group_id = Uuid::new_v4();
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/groups/{}/providers", group_id)))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::read"]).await;

    let response = reqwest::Client::new()
        .get(&server.api_url("/llm-providers?per_page=100"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = response.json().await.unwrap();
    let providers = body["providers"].as_array().unwrap();

    // Check for built-in providers
    let expected_providers = vec!["OpenAI", "Anthropic", "Groq", "Google Gemini", "Mistral AI", "DeepSeek", "Local"];
    for expected in expected_providers {
        assert!(
            providers.iter().any(|p| p["name"] == expected && p["built_in"] == true),
            "Built-in provider '{}' should exist",
            expected
        );
    }
}

#[tokio::test]
async fn test_update_built_in_provider() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_providers::read", "llm_providers::edit"]).await;

    // Get OpenAI built-in provider
    let response = reqwest::Client::new()
        .get(&server.api_url("/llm-providers?per_page=100"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = response.json().await.unwrap();
    let providers = body["providers"].as_array().unwrap();
    let openai = providers.iter()
        .find(|p| p["name"] == "OpenAI")
        .expect("OpenAI provider should exist");
    let provider_id = openai["id"].as_str().unwrap();

    // Update enabled and api_key - should be allowed
    let payload = json!({
        "enabled": true,
        "api_key": "sk-testkey"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["enabled"], true);
    assert_eq!(body["api_key"], "sk-testkey");
    assert_eq!(body["built_in"], true);
}

// =====================================================
// Helper Functions
// =====================================================

async fn create_test_provider(server: &crate::common::TestServer, token: &str) -> serde_json::Value {
    let payload = json!({
        "name": "Test Provider",
        "provider_type": "openai",
        "enabled": false,
        "api_key": "sk-test123"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-providers"))
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
        .get(&server.api_url("/groups"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = response.json().await.unwrap();
    let groups = body["groups"].as_array().unwrap();
    let admin_group = groups.iter()
        .find(|g| g["name"] == "Administrators")
        .expect("Administrators group should exist");

    admin_group["id"].as_str().unwrap().to_string()
}
