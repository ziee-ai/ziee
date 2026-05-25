//! User-accessible LLM providers endpoint integration tests

use reqwest::StatusCode;
use serde_json::json;

// =======================================================
// Get User LLM Providers Tests
// =======================================================

#[tokio::test]
async fn test_get_user_providers_returns_providers_from_groups() {
    let server = crate::common::TestServer::start().await;

    // Create test user with conversations::read permission (required for endpoint)
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "test_user",
        &["conversations::read"],
    )
    .await;

    // Create admin user to set up providers, groups, and models
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "llm_providers::create",
            "llm_providers::read",
            "llm_providers::edit",
            "llm_providers::assign_groups",
            "llm_models::create",
            "llm_models::read",
            "groups::create",
            "groups::edit",
            "groups::read",
            "groups::assign_users",
        ],
    )
    .await;

    // Create a group
    let group_response = reqwest::Client::new()
        .post(server.api_url("/groups"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "test_group",
            "description": "Test group",
            "permissions": []
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(group_response.status(), StatusCode::CREATED);
    let group: serde_json::Value = group_response.json().await.unwrap();
    let group_id = group["id"].as_str().unwrap();

    // Add user to group
    let add_member_response = reqwest::Client::new()
        .post(server.api_url("/groups/assign"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "user_id": user.user_id,
            "group_id": group_id
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(add_member_response.status(), StatusCode::NO_CONTENT);

    // Create a provider
    let provider_response = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "test_provider",
            "provider_type": "openai",
            "enabled": true,
            "api_key": "test-key",
            "base_url": "https://api.openai.com/v1"
        }))
        .send()
        .await
        .unwrap();
    let status = provider_response.status();
    assert_eq!(status, StatusCode::CREATED, "Provider creation failed with status {}", status);
    let provider: serde_json::Value = provider_response.json().await.unwrap();
    let provider_id = provider["id"].as_str().unwrap();

    // Create a model for the provider
    let model_response = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "provider_id": provider_id,
            "name": "gpt-4",
            "display_name": "GPT-4",
            "enabled": true,
            "engine_type": "none",
            "file_format": "gguf",
            "capabilities": {
                "chat": true,
                "streaming": true,
                "function_calling": true,
                "vision": false
            },
            "parameters": {
                "max_tokens": 8192,
                "context_window": 8192,
                "temperature_range": {
                    "min": 0.0,
                    "max": 2.0
                }
            }
        }))
        .send()
        .await
        .unwrap();
    let status = model_response.status();
    if status != StatusCode::CREATED {
        let error_body = model_response.text().await.unwrap();
        panic!("Model creation failed with status {}: {}", status, error_body);
    }
    let model: serde_json::Value = model_response.json().await.unwrap();
    let model_id = model["id"].as_str().unwrap();

    // Assign provider to group
    let assign_response = reqwest::Client::new()
        .put(server.api_url(&format!("/groups/{}/providers", group_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "provider_ids": [provider_id]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(assign_response.status(), StatusCode::OK);

    // Now test the user-accessible providers endpoint
    let response = reqwest::Client::new()
        .get(server.api_url("/chat/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();

    // Verify response structure
    assert!(body["providers"].is_array(), "Should have providers array");
    let providers = body["providers"].as_array().unwrap();
    assert_eq!(providers.len(), 1, "User should have access to 1 provider");

    // Verify provider details
    let returned_provider = &providers[0];
    assert_eq!(returned_provider["id"].as_str().unwrap(), provider_id);
    assert_eq!(returned_provider["name"], "test_provider");
    assert_eq!(returned_provider["provider_type"], "openai");
    assert_eq!(returned_provider["enabled"], true);

    // Verify models are included
    assert!(returned_provider["llm_models"].is_array(), "Provider should have llm_models array");
    let models = returned_provider["llm_models"].as_array().unwrap();
    assert_eq!(models.len(), 1, "Provider should have 1 enabled model");

    let returned_model = &models[0];
    assert_eq!(returned_model["id"].as_str().unwrap(), model_id);
    assert_eq!(returned_model["name"], "gpt-4");
    assert_eq!(returned_model["enabled"], true);
}

#[tokio::test]
async fn test_get_user_providers_empty_when_no_group_assignments() {
    let server = crate::common::TestServer::start().await;

    // Create user not assigned to any groups
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "lonely_user",
        &["conversations::read"],
    )
    .await;

    let response = reqwest::Client::new()
        .get(server.api_url("/chat/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();

    let providers = body["providers"].as_array().unwrap();
    assert_eq!(providers.len(), 0, "User should have no providers");
}

#[tokio::test]
async fn test_get_user_providers_filters_disabled_models() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "test_user",
        &["conversations::read"],
    )
    .await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "llm_providers::create",
            "llm_providers::read",
            "llm_providers::edit",
            "llm_providers::assign_groups",
            "llm_models::create",
            "llm_models::read",
            "llm_models::edit",
            "groups::create",
            "groups::edit",
            "groups::read",
            "groups::assign_users",
        ],
    )
    .await;

    // Create group, provider, and assign user to group
    let group_response = reqwest::Client::new()
        .post(server.api_url("/groups"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({"name": "test_group", "permissions": []}))
        .send()
        .await
        .unwrap();
    let group: serde_json::Value = group_response.json().await.unwrap();
    let group_id = group["id"].as_str().unwrap();

    reqwest::Client::new()
        .post(server.api_url("/groups/assign"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({"user_id": user.user_id, "group_id": group_id}))
        .send()
        .await
        .unwrap();

    let provider_response = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "test_provider",
            "provider_type": "openai",
            "enabled": true,
            "api_key": "test-key",
            "base_url": "https://api.openai.com/v1"
        }))
        .send()
        .await
        .unwrap();
    let provider: serde_json::Value = provider_response.json().await.unwrap();
    let provider_id = provider["id"].as_str().unwrap();

    // Create one enabled model
    reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "provider_id": provider_id,
            "name": "enabled-model",
            "display_name": "Enabled Model",
            "enabled": true,
            "engine_type": "none",
            "file_format": "gguf",
            "capabilities": {"chat": true},
            "parameters": {"max_tokens": 8192}
        }))
        .send()
        .await
        .unwrap();

    // Create one disabled model
    reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "provider_id": provider_id,
            "name": "disabled-model",
            "display_name": "Disabled Model",
            "enabled": false,
            "engine_type": "none",
            "file_format": "gguf",
            "capabilities": {"chat": true},
            "parameters": {"max_tokens": 8192}
        }))
        .send()
        .await
        .unwrap();

    // Assign provider to group
    reqwest::Client::new()
        .put(server.api_url(&format!("/groups/{}/providers", group_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({"provider_ids": [provider_id]}))
        .send()
        .await
        .unwrap();

    // Get user's providers
    let response = reqwest::Client::new()
        .get(server.api_url("/chat/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();

    let providers = body["providers"].as_array().unwrap();
    assert_eq!(providers.len(), 1);

    let models = providers[0]["llm_models"].as_array().unwrap();
    assert_eq!(models.len(), 1, "Should only return enabled model");
    assert_eq!(models[0]["name"], "enabled-model");
}

#[tokio::test]
async fn test_get_user_providers_requires_auth() {
    let server = crate::common::TestServer::start().await;

    let response = reqwest::Client::new()
        .get(server.api_url("/chat/llm-providers"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_user_providers_requires_conversations_read_permission() {
    let server = crate::common::TestServer::start().await;

    // Create user without conversations::read permission. Must use
    // `create_user_with_no_permissions` — the `_with_permissions(_, _, &[])`
    // variant leaves the user in the default Users group (migration 27)
    // which DOES grant `conversations::read`, so the assertion would
    // fail.
    let user = crate::common::test_helpers::create_user_with_no_permissions(
        &server,
        "no_perms_user",
    )
    .await;

    let response = reqwest::Client::new()
        .get(server.api_url("/chat/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
