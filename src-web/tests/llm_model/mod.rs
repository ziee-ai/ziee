// Integration tests for LLM Model module

mod download_management_test;
mod download_test;
mod upload_test;

use serde_json::json;
use reqwest::StatusCode;
use uuid::Uuid;

// =====================================================
// Permission Tests
// =====================================================

#[tokio::test]
async fn test_list_models_requires_read_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let response = reqwest::Client::new()
        .get(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_list_models_with_read_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read"]).await;

    let response = reqwest::Client::new()
        .get(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["models"].is_array());
    assert!(body["total"].is_number());
}

#[tokio::test]
async fn test_get_model_requires_read_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let model_id = Uuid::new_v4();
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_create_model_requires_create_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_providers::read"]).await;

    // Get a provider first
    let provider = get_first_provider(&server, &user.token).await;

    let payload = json!({
        "provider_id": provider["id"],
        "name": "test-model",
        "display_name": "Test Model",
        "engine_type": "llamacpp",
        "file_format": "gguf"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_update_model_requires_edit_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_providers::read"]).await;

    // Create a model first
    let model = create_test_model(&server, &user.token).await;

    // Try to update without edit permission
    let payload = json!({
        "display_name": "Updated Name"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}", model["id"])))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_delete_model_requires_delete_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_providers::read"]).await;

    // Create a model first
    let model = create_test_model(&server, &user.token).await;

    // Try to delete without delete permission
    let response = reqwest::Client::new()
        .delete(&server.api_url(&format!("/llm-models/{}", model["id"])))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_enable_model_requires_edit_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_providers::read"]).await;

    // Create a model first
    let model = create_test_model(&server, &user.token).await;

    // Try to enable without edit permission
    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}/enable", model["id"])))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_disable_model_requires_edit_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_providers::read"]).await;

    // Create a model first
    let model = create_test_model(&server, &user.token).await;

    // Try to disable without edit permission
    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}/disable", model["id"])))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// =====================================================
// CRUD Tests
// =====================================================

#[tokio::test]
async fn test_list_models_with_pagination() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read"]).await;

    // Test default pagination
    let response = reqwest::Client::new()
        .get(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["page"], 1);
    assert_eq!(body["per_page"], 10);
    assert!(body["total"].is_number());

    // Test custom pagination (using camelCase as per serde configuration)
    let response = reqwest::Client::new()
        .get(&server.api_url("/llm-models?page=1&perPage=5"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["page"], 1);
    assert_eq!(body["per_page"], 5);
    let models = body["models"].as_array().unwrap();
    assert!(models.len() <= 5);
}

#[tokio::test]
async fn test_list_models_with_provider_filter() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_providers::read"]).await;

    // Get a provider
    let provider = get_first_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Create a model for this provider
    let _model = create_test_model(&server, &user.token).await;

    // List models filtered by provider
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/llm-models?providerId={}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();

    // All returned models should belong to this provider
    for model in body["models"].as_array().unwrap() {
        assert_eq!(model["provider_id"], provider_id);
    }
}

#[tokio::test]
async fn test_get_model_by_id_success() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_providers::read"]).await;

    // Create a model
    let model = create_test_model(&server, &user.token).await;
    let model_id = model["id"].as_str().unwrap();

    // Get the model
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["id"], model["id"]);
    assert_eq!(body["name"], "test-model");
    assert_eq!(body["display_name"], "Test Model");
}

#[tokio::test]
async fn test_get_model_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read"]).await;

    let model_id = Uuid::new_v4();
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_create_model_minimal() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let provider = get_first_provider(&server, &user.token).await;

    let payload = json!({
        "provider_id": provider["id"],
        "name": "minimal-model",
        "display_name": "Minimal Model",
        "engine_type": "llamacpp",
        "file_format": "gguf"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["name"], "minimal-model");
    assert_eq!(body["display_name"], "Minimal Model");
    assert_eq!(body["engine_type"], "llamacpp");
    assert_eq!(body["file_format"], "gguf");
    assert!(body["id"].is_string());
}

#[tokio::test]
async fn test_create_model_with_all_fields() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let provider = get_first_provider(&server, &user.token).await;

    let payload = json!({
        "provider_id": provider["id"],
        "name": "full-model",
        "display_name": "Full Model",
        "description": "A test model with additional fields",
        "enabled": true,
        "engine_type": "llamacpp",
        "file_format": "gguf",
        "capabilities": {
            "chat": true,
            "completion": true,
            "embedding": false,
            "function_calling": false
        }
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["name"], "full-model");
    assert_eq!(body["display_name"], "Full Model");
    assert_eq!(body["description"], "A test model with additional fields");
    assert_eq!(body["enabled"], true);
    assert_eq!(body["capabilities"]["chat"], true);
}

#[tokio::test]
async fn test_update_model_display_name() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_models::edit", "llm_providers::read"]).await;

    // Create a model
    let model = create_test_model(&server, &user.token).await;
    let model_id = model["id"].as_str().unwrap();

    // Update display name
    let payload = json!({
        "display_name": "Updated Model Name"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["display_name"], "Updated Model Name");
    assert_eq!(body["name"], "test-model"); // Unchanged
}

#[tokio::test]
async fn test_update_model_enabled_and_description() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_models::edit", "llm_providers::read"]).await;

    // Create a model
    let model = create_test_model(&server, &user.token).await;
    let model_id = model["id"].as_str().unwrap();

    // Update enabled and description
    let payload = json!({
        "enabled": true,
        "description": "New description for the model"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["enabled"], true);
    assert_eq!(body["description"], "New description for the model");
}

#[tokio::test]
async fn test_update_model_engine_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_models::edit", "llm_providers::read"]).await;

    // Create a model
    let model = create_test_model(&server, &user.token).await;
    let model_id = model["id"].as_str().unwrap();

    // Update engine_type
    let payload = json!({
        "engine_type": "mistralrs"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["engine_type"], "mistralrs");
}

#[tokio::test]
async fn test_update_model_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::edit"]).await;

    let model_id = Uuid::new_v4();
    let payload = json!({
        "display_name": "Updated Name"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_model() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_models::delete", "llm_providers::read"]).await;

    // Create a model
    let model = create_test_model(&server, &user.token).await;
    let model_id = model["id"].as_str().unwrap();

    // Delete it
    let response = reqwest::Client::new()
        .delete(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify it's gone
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_model_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::delete"]).await;

    let model_id = Uuid::new_v4();
    let response = reqwest::Client::new()
        .delete(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_enable_model() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_models::edit", "llm_providers::read"]).await;

    // Create a model (disabled by default in test helper)
    let model = create_test_model(&server, &user.token).await;
    let model_id = model["id"].as_str().unwrap();

    // Enable it
    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}/enable", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["enabled"], true);
}

#[tokio::test]
async fn test_disable_model() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_models::edit", "llm_providers::read"]).await;

    // Create a model and enable it
    let model = create_test_model(&server, &user.token).await;
    let model_id = model["id"].as_str().unwrap();

    // First enable it
    reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}/enable", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    // Then disable it
    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}/disable", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["enabled"], false);
}

#[tokio::test]
async fn test_enable_model_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::edit"]).await;

    let model_id = Uuid::new_v4();
    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}/enable", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_disable_model_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::edit"]).await;

    let model_id = Uuid::new_v4();
    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}/disable", model_id)))
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
async fn test_create_model_empty_name() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let provider = get_first_provider(&server, &user.token).await;

    let payload = json!({
        "provider_id": provider["id"],
        "name": "",
        "display_name": "Test Model",
        "engine_type": "llamacpp",
        "file_format": "gguf"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_create_model_empty_display_name() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let provider = get_first_provider(&server, &user.token).await;

    let payload = json!({
        "provider_id": provider["id"],
        "name": "test-model",
        "display_name": "",
        "engine_type": "llamacpp",
        "file_format": "gguf"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_create_model_name_too_long() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let provider = get_first_provider(&server, &user.token).await;

    let long_name = "a".repeat(256); // Over 255 character limit

    let payload = json!({
        "provider_id": provider["id"],
        "name": long_name,
        "display_name": "Test Model",
        "engine_type": "llamacpp",
        "file_format": "gguf"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_create_model_display_name_too_long() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let provider = get_first_provider(&server, &user.token).await;

    let long_display_name = "a".repeat(256); // Over 255 character limit

    let payload = json!({
        "provider_id": provider["id"],
        "name": "test-model",
        "display_name": long_display_name,
        "engine_type": "llamacpp",
        "file_format": "gguf"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_update_model_empty_name() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_models::edit", "llm_providers::read"]).await;

    // Create a model
    let model = create_test_model(&server, &user.token).await;
    let model_id = model["id"].as_str().unwrap();

    // Try to update with empty name
    let payload = json!({
        "name": ""
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_update_model_empty_display_name() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_models::edit", "llm_providers::read"]).await;

    // Create a model
    let model = create_test_model(&server, &user.token).await;
    let model_id = model["id"].as_str().unwrap();

    // Try to update with empty display name
    let payload = json!({
        "display_name": ""
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_create_model_invalid_provider() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create"]).await;

    let invalid_provider_id = Uuid::new_v4();

    let payload = json!({
        "provider_id": invalid_provider_id,
        "name": "test-model",
        "display_name": "Test Model",
        "engine_type": "llamacpp",
        "file_format": "gguf"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // Should fail because provider doesn't exist
    assert!(response.status().is_client_error() || response.status().is_server_error());
}

// =====================================================
// Missing Field Tests
// =====================================================

#[tokio::test]
async fn test_create_model_missing_provider_id() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let payload = json!({
        "name": "test-model",
        "display_name": "Test Model",
        "engine_type": "llamacpp",
        "file_format": "gguf"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_create_model_missing_name() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let provider = get_first_provider(&server, &user.token).await;

    let payload = json!({
        "provider_id": provider["id"],
        "display_name": "Test Model",
        "engine_type": "llamacpp",
        "file_format": "gguf"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_create_model_missing_display_name() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let provider = get_first_provider(&server, &user.token).await;

    let payload = json!({
        "provider_id": provider["id"],
        "name": "test-model",
        "engine_type": "llamacpp",
        "file_format": "gguf"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_create_model_missing_engine_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let provider = get_first_provider(&server, &user.token).await;

    let payload = json!({
        "provider_id": provider["id"],
        "name": "test-model",
        "display_name": "Test Model",
        "file_format": "gguf"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_create_model_missing_file_format() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let provider = get_first_provider(&server, &user.token).await;

    let payload = json!({
        "provider_id": provider["id"],
        "name": "test-model",
        "display_name": "Test Model",
        "engine_type": "llamacpp"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// =====================================================
// Wrong Type Tests
// =====================================================

#[tokio::test]
async fn test_create_model_provider_id_wrong_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let payload = json!({
        "provider_id": 12345, // Should be UUID string
        "name": "test-model",
        "display_name": "Test Model",
        "engine_type": "llamacpp",
        "file_format": "gguf"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_create_model_name_wrong_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let provider = get_first_provider(&server, &user.token).await;

    let payload = json!({
        "provider_id": provider["id"],
        "name": 12345, // Should be string
        "display_name": "Test Model",
        "engine_type": "llamacpp",
        "file_format": "gguf"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_create_model_display_name_wrong_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let provider = get_first_provider(&server, &user.token).await;

    let payload = json!({
        "provider_id": provider["id"],
        "name": "test-model",
        "display_name": true, // Should be string
        "engine_type": "llamacpp",
        "file_format": "gguf"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_create_model_enabled_wrong_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let provider = get_first_provider(&server, &user.token).await;

    let payload = json!({
        "provider_id": provider["id"],
        "name": "test-model",
        "display_name": "Test Model",
        "enabled": "yes", // Should be boolean
        "engine_type": "llamacpp",
        "file_format": "gguf"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_create_model_description_wrong_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let provider = get_first_provider(&server, &user.token).await;

    let payload = json!({
        "provider_id": provider["id"],
        "name": "test-model",
        "display_name": "Test Model",
        "description": ["array", "instead", "of", "string"], // Should be string
        "engine_type": "llamacpp",
        "file_format": "gguf"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_create_model_capabilities_wrong_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let provider = get_first_provider(&server, &user.token).await;

    let payload = json!({
        "provider_id": provider["id"],
        "name": "test-model",
        "display_name": "Test Model",
        "engine_type": "llamacpp",
        "file_format": "gguf",
        "capabilities": "not an object" // Should be object
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_create_model_parameters_wrong_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let provider = get_first_provider(&server, &user.token).await;

    let payload = json!({
        "provider_id": provider["id"],
        "name": "test-model",
        "display_name": "Test Model",
        "engine_type": "llamacpp",
        "file_format": "gguf",
        "parameters": [1, 2, 3] // Should be object
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_create_model_engine_settings_wrong_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let provider = get_first_provider(&server, &user.token).await;

    let payload = json!({
        "provider_id": provider["id"],
        "name": "test-model",
        "display_name": "Test Model",
        "engine_type": "llamacpp",
        "file_format": "gguf",
        "engine_settings": "string instead of object" // Should be object
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// =====================================================
// Invalid Enum Value Tests
// =====================================================

#[tokio::test]
async fn test_create_model_invalid_engine_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let provider = get_first_provider(&server, &user.token).await;

    let payload = json!({
        "provider_id": provider["id"],
        "name": "test-model",
        "display_name": "Test Model",
        "engine_type": "invalid_engine", // Invalid enum value
        "file_format": "gguf"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_create_model_invalid_file_format() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::create", "llm_providers::read"]).await;

    let provider = get_first_provider(&server, &user.token).await;

    let payload = json!({
        "provider_id": provider["id"],
        "name": "test-model",
        "display_name": "Test Model",
        "engine_type": "llamacpp",
        "file_format": "invalid_format" // Invalid enum value
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// =====================================================
// Update Field Wrong Type Tests
// =====================================================

#[tokio::test]
async fn test_update_model_name_wrong_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_models::edit", "llm_providers::read"]).await;

    let model = create_test_model(&server, &user.token).await;
    let model_id = model["id"].as_str().unwrap();

    let payload = json!({
        "name": 12345 // Should be string
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_update_model_display_name_wrong_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_models::edit", "llm_providers::read"]).await;

    let model = create_test_model(&server, &user.token).await;
    let model_id = model["id"].as_str().unwrap();

    let payload = json!({
        "display_name": false // Should be string
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_update_model_enabled_wrong_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_models::edit", "llm_providers::read"]).await;

    let model = create_test_model(&server, &user.token).await;
    let model_id = model["id"].as_str().unwrap();

    let payload = json!({
        "enabled": "true" // Should be boolean
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_update_model_description_wrong_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_models::edit", "llm_providers::read"]).await;

    let model = create_test_model(&server, &user.token).await;
    let model_id = model["id"].as_str().unwrap();

    let payload = json!({
        "description": {"object": "instead of string"} // Should be string
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_update_model_invalid_engine_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_models::edit", "llm_providers::read"]).await;

    let model = create_test_model(&server, &user.token).await;
    let model_id = model["id"].as_str().unwrap();

    let payload = json!({
        "engine_type": "nonexistent_engine" // Invalid enum value
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_update_model_invalid_file_format() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_models::edit", "llm_providers::read"]).await;

    let model = create_test_model(&server, &user.token).await;
    let model_id = model["id"].as_str().unwrap();

    let payload = json!({
        "file_format": "unsupported_format" // Invalid enum value
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_update_model_capabilities_wrong_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_models::edit", "llm_providers::read"]).await;

    let model = create_test_model(&server, &user.token).await;
    let model_id = model["id"].as_str().unwrap();

    let payload = json!({
        "capabilities": "wrong type" // Should be object
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_update_model_parameters_wrong_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_models::edit", "llm_providers::read"]).await;

    let model = create_test_model(&server, &user.token).await;
    let model_id = model["id"].as_str().unwrap();

    let payload = json!({
        "parameters": "not an object" // Should be object
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_update_model_engine_settings_wrong_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["llm_models::read", "llm_models::create", "llm_models::edit", "llm_providers::read"]).await;

    let model = create_test_model(&server, &user.token).await;
    let model_id = model["id"].as_str().unwrap();

    let payload = json!({
        "engine_settings": [1, 2, 3] // Should be object
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// =====================================================
// Helper Functions
// =====================================================

async fn create_test_model(server: &crate::common::TestServer, token: &str) -> serde_json::Value {
    let provider = get_first_provider(server, token).await;

    let payload = json!({
        "provider_id": provider["id"],
        "name": "test-model",
        "display_name": "Test Model",
        "description": "A test model for integration tests",
        "enabled": false,
        "engine_type": "llamacpp",
        "file_format": "gguf"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    response.json().await.unwrap()
}

async fn get_first_provider(server: &crate::common::TestServer, token: &str) -> serde_json::Value {
    // Ensure user has permission to read providers
    let response = reqwest::Client::new()
        .get(&server.api_url("/llm-providers?per_page=1"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = response.json().await.unwrap();
    let providers = body["providers"].as_array().unwrap();
    assert!(!providers.is_empty(), "No providers found - tests need at least one provider");

    providers[0].clone()
}
