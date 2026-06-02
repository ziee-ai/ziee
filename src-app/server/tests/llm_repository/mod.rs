// ============================================================================
// LLM Repository Module Integration Tests
// ============================================================================
//
// This test suite covers all CRUD operations and permission checks for the
// LLM Repository module, which manages external LLM model repositories like
// Hugging Face and GitHub with authentication support.

use serde_json::json;

mod test_connection_user_agent;

#[tokio::test]
async fn test_list_llm_repositories_requires_permission() {
    let server = crate::common::TestServer::start().await;

    // Create user with llm_repositories::read permission
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_repositories::read"],
    )
    .await;

    // Create user without permission
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "regular", &[]).await;

    // Admin should be able to list repositories
    let url = server.api_url("/llm-repositories");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Admin should list repositories");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(
        body.get("repositories").is_some(),
        "Should have repositories array"
    );
    assert!(body.get("total").is_some(), "Should have total count");
    assert!(body.get("page").is_some(), "Should have page number");
    assert!(body.get("per_page").is_some(), "Should have per_page");

    // Verify at least built-in repositories exist (Hugging Face, GitHub)
    let repositories = body
        .get("repositories")
        .and_then(|r| r.as_array())
        .expect("repositories should be an array");
    assert!(
        repositories.len() >= 2,
        "Should have at least 2 built-in repositories"
    );

    // Regular user without permission should get 403
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Regular user should be forbidden");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(
        body.get("error_code").and_then(|v| v.as_str()),
        Some("INSUFFICIENT_PERMISSIONS")
    );
}

#[tokio::test]
async fn test_create_llm_repository() {
    let server = crate::common::TestServer::start().await;

    // Create user with llm_repositories::create permission
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_repositories::create"],
    )
    .await;

    // Create a test repository
    let url = server.api_url("/llm-repositories");
    let create_data = json!({
        "name": "Test Repository",
        "url": "https://example.com/test",
        "auth_type": "api_key",
        "auth_config": {
            "api_key": "test-api-key-12345"
        },
        "enabled": true
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&create_data)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201, "Should create repository");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(body.get("id").is_some(), "Should have repository ID");
    assert_eq!(
        body.get("name").and_then(|v| v.as_str()),
        Some("Test Repository")
    );
    assert_eq!(
        body.get("url").and_then(|v| v.as_str()),
        Some("https://example.com/test")
    );
    assert_eq!(
        body.get("auth_type").and_then(|v| v.as_str()),
        Some("api_key")
    );
    assert_eq!(body.get("enabled").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        body.get("built_in").and_then(|v| v.as_bool()),
        Some(false),
        "Created repository should not be built-in"
    );

    // Verify auth_config is present but api_key is write-only.
    // Post-09-llm-repository-F-02 fix: api_key / password / token are
    // serde(skip_serializing). Inverting the original assertion.
    let auth_config = body.get("auth_config").expect("Should have auth_config");
    assert!(
        auth_config.get("api_key").is_none()
            || auth_config["api_key"].is_null(),
        "api_key must not be returned in response (09-llm-repository F-02); got {:?}",
        auth_config.get("api_key")
    );
}

#[tokio::test]
async fn test_create_llm_repository_validation() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_repositories::create"],
    )
    .await;

    let url = server.api_url("/llm-repositories");

    // Test 1: Invalid URL format
    let invalid_url_data = json!({
        "name": "Test Repository",
        "url": "not-a-valid-url",
        "auth_type": "none"
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&invalid_url_data)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 400, "Should reject invalid URL format");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    // Either error code is acceptable; 'INVALID_URL' is the post-F-01-fix shape.
    let code = body.get("error_code").and_then(|v| v.as_str());
    assert!(
        code == Some("VALIDATION_ERROR") || code == Some("INVALID_URL"),
        "expected VALIDATION_ERROR or INVALID_URL, got {:?}",
        code
    );

    // Test 2: Invalid auth type
    let invalid_auth_data = json!({
        "name": "Test Repository",
        "url": "https://example.com",
        "auth_type": "invalid_auth_type"
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&invalid_auth_data)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 400, "Should reject invalid auth type");

    // Test 3: Missing auth_config for api_key auth type
    let missing_auth_config_data = json!({
        "name": "Test Repository",
        "url": "https://example.com",
        "auth_type": "api_key"
        // Missing auth_config
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&missing_auth_config_data)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        400,
        "Should reject missing auth_config for api_key"
    );

    // Test 4: Empty api_key in auth_config
    let empty_api_key_data = json!({
        "name": "Test Repository",
        "url": "https://example.com",
        "auth_type": "api_key",
        "auth_config": {
            "api_key": ""
        }
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&empty_api_key_data)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 400, "Should reject empty api_key");

    // Test 5: Missing credentials for basic_auth
    let missing_basic_auth_data = json!({
        "name": "Test Repository",
        "url": "https://example.com",
        "auth_type": "basic_auth",
        "auth_config": {
            "username": "testuser"
            // Missing password
        }
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&missing_basic_auth_data)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        400,
        "Should reject incomplete basic_auth credentials"
    );
}

#[tokio::test]
async fn test_update_llm_repository() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "llm_repositories::create",
            "llm_repositories::edit",
            "llm_repositories::read",
        ],
    )
    .await;

    // Create a repository first
    let create_url = server.api_url("/llm-repositories");
    let create_data = json!({
        "name": "Update Test Repository",
        "url": "https://example.com/test",
        "auth_type": "none",
        "enabled": true
    });

    let create_response = reqwest::Client::new()
        .post(&create_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&create_data)
        .send()
        .await
        .expect("Request failed");

    let created_repo: serde_json::Value =
        create_response.json().await.expect("Failed to parse JSON");
    let repo_id = created_repo.get("id").and_then(|v| v.as_str()).unwrap();

    // Update the repository
    let update_url = server.api_url(&format!("/llm-repositories/{}", repo_id));
    let update_data = json!({
        "name": "Updated Repository Name",
        "enabled": false
    });

    let response = reqwest::Client::new()
        .post(&update_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&update_data)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Should update repository");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(
        body.get("name").and_then(|v| v.as_str()),
        Some("Updated Repository Name")
    );
    assert_eq!(body.get("enabled").and_then(|v| v.as_bool()), Some(false));
    // URL should remain unchanged
    assert_eq!(
        body.get("url").and_then(|v| v.as_str()),
        Some("https://example.com/test")
    );
}

#[tokio::test]
async fn test_update_llm_repository_built_in_protection() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "llm_repositories::read",
            "llm_repositories::edit",
            "llm_repositories::create",
        ],
    )
    .await;

    // Get the list of repositories to find a built-in one
    let list_url = server.api_url("/llm-repositories");
    let list_response = reqwest::Client::new()
        .get(&list_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    let list_body: serde_json::Value = list_response.json().await.expect("Failed to parse JSON");
    let repositories = list_body
        .get("repositories")
        .and_then(|r| r.as_array())
        .expect("Should have repositories array");

    // Find a built-in repository (Hugging Face or GitHub)
    let built_in_repo = repositories
        .iter()
        .find(|r| r.get("built_in").and_then(|v| v.as_bool()) == Some(true))
        .expect("Should have at least one built-in repository");

    let repo_id = built_in_repo
        .get("id")
        .and_then(|v| v.as_str())
        .expect("Repository should have ID");

    // Try to update the built-in repository - this should succeed
    // Built-in repositories can be updated (e.g., adding API keys)
    let update_url = server.api_url(&format!("/llm-repositories/{}", repo_id));
    let update_data = json!({
        "enabled": false
    });

    let response = reqwest::Client::new()
        .post(&update_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&update_data)
        .send()
        .await
        .expect("Request failed");

    // Built-in repositories CAN be updated, just not deleted
    assert_eq!(
        response.status(),
        200,
        "Built-in repositories can be updated"
    );
}

#[tokio::test]
async fn test_delete_llm_repository() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "llm_repositories::create",
            "llm_repositories::delete",
            "llm_repositories::read",
        ],
    )
    .await;

    // Create a repository first
    let create_url = server.api_url("/llm-repositories");
    let create_data = json!({
        "name": "Delete Test Repository",
        "url": "https://example.com/delete-test",
        "auth_type": "none"
    });

    let create_response = reqwest::Client::new()
        .post(&create_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&create_data)
        .send()
        .await
        .expect("Request failed");

    let created_repo: serde_json::Value =
        create_response.json().await.expect("Failed to parse JSON");
    let repo_id = created_repo.get("id").and_then(|v| v.as_str()).unwrap();

    // Delete the repository
    let delete_url = server.api_url(&format!("/llm-repositories/{}", repo_id));
    let response = reqwest::Client::new()
        .delete(&delete_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 204, "Should delete repository");

    // Verify repository is deleted by trying to get it
    let get_url = server.api_url(&format!("/llm-repositories/{}", repo_id));
    let get_response = reqwest::Client::new()
        .get(&get_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        get_response.status(),
        404,
        "Deleted repository should return 404"
    );
}

#[tokio::test]
async fn test_delete_built_in_repository_protected() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_repositories::read", "llm_repositories::delete"],
    )
    .await;

    // Get the list of repositories to find a built-in one
    let list_url = server.api_url("/llm-repositories");
    let list_response = reqwest::Client::new()
        .get(&list_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    let list_body: serde_json::Value = list_response.json().await.expect("Failed to parse JSON");
    let repositories = list_body
        .get("repositories")
        .and_then(|r| r.as_array())
        .expect("Should have repositories array");

    // Find a built-in repository (Hugging Face or GitHub)
    let built_in_repo = repositories
        .iter()
        .find(|r| r.get("built_in").and_then(|v| v.as_bool()) == Some(true))
        .expect("Should have at least one built-in repository");

    let repo_id = built_in_repo
        .get("id")
        .and_then(|v| v.as_str())
        .expect("Repository should have ID");

    // Try to delete the built-in repository - should be rejected
    let delete_url = server.api_url(&format!("/llm-repositories/{}", repo_id));
    let response = reqwest::Client::new()
        .delete(&delete_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        400,
        "Should reject deletion of built-in repository"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let error_message = body.get("error").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        error_message.to_lowercase().contains("built-in")
            || error_message.to_lowercase().contains("built in"),
        "Error message should mention built-in protection, got: {}",
        error_message
    );
}

#[tokio::test]
async fn test_delete_repository_not_found() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_repositories::delete"],
    )
    .await;

    // Try to delete a non-existent repository
    let fake_uuid = "00000000-0000-0000-0000-000000000000";
    let delete_url = server.api_url(&format!("/llm-repositories/{}", fake_uuid));
    let response = reqwest::Client::new()
        .delete(&delete_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        404,
        "Should return 404 for non-existent repository"
    );
}

#[tokio::test]
async fn test_repository_connection_test() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_repositories::create", "llm_repositories::read"],
    )
    .await;

    // Test connection with valid URL (use a public endpoint like GitHub)
    let test_url = server.api_url("/llm-repositories/test");
    let test_data = json!({
        "name": "Test Connection",
        "url": "https://api.github.com",
        "auth_type": "none"
    });

    let response = reqwest::Client::new()
        .post(&test_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&test_data)
        .send()
        .await
        .expect("Request failed");

    // Connection test should return 200 and success/failure status
    assert_eq!(
        response.status(),
        200,
        "Connection test endpoint should respond"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(body.get("success").is_some(), "Should have success field");
    assert!(body.get("message").is_some(), "Should have message field");
}

#[tokio::test]
async fn test_repository_connection_test_with_valid_huggingface_credentials() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_repositories::read"],
    )
    .await;

    // Get HuggingFace API key from environment (if available)
    let hf_api_key = std::env::var("HUGGINGFACE_API_KEY").ok();

    if let Some(api_key) = hf_api_key {
        let test_url = server.api_url("/llm-repositories/test");
        let test_data = json!({
            "name": "HuggingFace Test",
            "url": "https://huggingface.co",
            "auth_type": "bearer_token",
            "auth_config": {
                "token": api_key,
                "auth_test_api_endpoint": "https://huggingface.co/api/whoami-v2"
            }
        });

        let response = reqwest::Client::new()
            .post(&test_url)
            .header("Authorization", format!("Bearer {}", admin.token))
            .json(&test_data)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(response.status(), 200, "Connection test should respond");

        let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
        assert_eq!(
            body.get("success").and_then(|v| v.as_bool()),
            Some(true),
            "Connection with valid credentials should succeed. Response: {:?}",
            body
        );
    } else {
        println!("Skipping HuggingFace valid credentials test - HUGGINGFACE_API_KEY not set");
    }
}

#[tokio::test]
async fn test_repository_connection_test_with_invalid_huggingface_credentials() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_repositories::read"],
    )
    .await;

    // Test with invalid HuggingFace credentials - should fail quickly (within 10s timeout)
    let test_url = server.api_url("/llm-repositories/test");
    let test_data = json!({
        "name": "HuggingFace Invalid Test",
        "url": "https://huggingface.co",
        "auth_type": "bearer_token",
        "auth_config": {
            "token": "hf_invalid_token_12345",
            "auth_test_api_endpoint": "https://huggingface.co/api/whoami-v2"
        }
    });

    let start = std::time::Instant::now();
    let response = reqwest::Client::new()
        .post(&test_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&test_data)
        .send()
        .await
        .expect("Request failed");

    let duration = start.elapsed();

    assert_eq!(response.status(), 200, "Connection test should respond");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(
        body.get("success").and_then(|v| v.as_bool()),
        Some(false),
        "Connection with invalid credentials should fail"
    );

    // Should fail quickly (within 15 seconds including network overhead)
    assert!(
        duration.as_secs() < 15,
        "Connection test with invalid credentials should fail quickly, took {}s",
        duration.as_secs()
    );
}

#[tokio::test]
async fn test_repository_connection_test_with_invalid_url() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_repositories::read"],
    )
    .await;

    // Test with invalid URL - should fail quickly
    let test_url = server.api_url("/llm-repositories/test");
    let test_data = json!({
        "name": "Invalid URL Test",
        "url": "https://invalid-test-url-that-does-not-exist-12345.com",
        "auth_type": "none"
    });

    let start = std::time::Instant::now();
    let response = reqwest::Client::new()
        .post(&test_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&test_data)
        .send()
        .await
        .expect("Request failed");

    let duration = start.elapsed();

    assert_eq!(response.status(), 200, "Connection test should respond");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(
        body.get("success").and_then(|v| v.as_bool()),
        Some(false),
        "Connection to invalid URL should fail"
    );

    // Should fail quickly (within 15 seconds)
    assert!(
        duration.as_secs() < 15,
        "Connection test with invalid URL should fail quickly, took {}s",
        duration.as_secs()
    );
}

#[tokio::test]
async fn test_create_requires_permission() {
    let server = crate::common::TestServer::start().await;

    // Create user without llm_repositories::create permission
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "regular", &[]).await;

    let url = server.api_url("/llm-repositories");
    let create_data = json!({
        "name": "Test Repository",
        "url": "https://example.com/test",
        "auth_type": "none"
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&create_data)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        403,
        "Should be forbidden without permission"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(
        body.get("error_code").and_then(|v| v.as_str()),
        Some("INSUFFICIENT_PERMISSIONS")
    );
}

#[tokio::test]
async fn test_edit_requires_permission() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_repositories::create"],
    )
    .await;

    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "regular", &[]).await;

    // Create a repository as admin
    let create_url = server.api_url("/llm-repositories");
    let create_data = json!({
        "name": "Permission Test Repository",
        "url": "https://example.com/test",
        "auth_type": "none"
    });

    let create_response = reqwest::Client::new()
        .post(&create_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&create_data)
        .send()
        .await
        .expect("Request failed");

    let created_repo: serde_json::Value =
        create_response.json().await.expect("Failed to parse JSON");
    let repo_id = created_repo.get("id").and_then(|v| v.as_str()).unwrap();

    // Try to update as regular user without permission
    let update_url = server.api_url(&format!("/llm-repositories/{}", repo_id));
    let update_data = json!({
        "enabled": false
    });

    let response = reqwest::Client::new()
        .post(&update_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&update_data)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        403,
        "Should be forbidden without permission"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(
        body.get("error_code").and_then(|v| v.as_str()),
        Some("INSUFFICIENT_PERMISSIONS")
    );
}

#[tokio::test]
async fn test_delete_requires_permission() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_repositories::create"],
    )
    .await;

    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "regular", &[]).await;

    // Create a repository as admin
    let create_url = server.api_url("/llm-repositories");
    let create_data = json!({
        "name": "Delete Permission Test",
        "url": "https://example.com/test",
        "auth_type": "none"
    });

    let create_response = reqwest::Client::new()
        .post(&create_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&create_data)
        .send()
        .await
        .expect("Request failed");

    let created_repo: serde_json::Value =
        create_response.json().await.expect("Failed to parse JSON");
    let repo_id = created_repo.get("id").and_then(|v| v.as_str()).unwrap();

    // Try to delete as regular user without permission
    let delete_url = server.api_url(&format!("/llm-repositories/{}", repo_id));
    let response = reqwest::Client::new()
        .delete(&delete_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        403,
        "Should be forbidden without permission"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(
        body.get("error_code").and_then(|v| v.as_str()),
        Some("INSUFFICIENT_PERMISSIONS")
    );
}

// =====================================================
// SSRF regression tests — close 09-llm-repository F-01
// =====================================================
//
// The original validate_url accepted any URL that reqwest::Url::parse
// succeeds on. That admitted file://, ftp://, gopher://, data:, http://
// to private IPs (RFC 1918, 169.254/16 — AWS IMDS) — every kind of SSRF
// the audit flagged as Critical. These tests pin the post-fix behavior:
// repositories with such URLs are rejected at the create-time validation
// layer with a 400.

async fn create_repo_request(
    server: &crate::common::TestServer,
    admin_token: &str,
    bad_url: &str,
) -> reqwest::Response {
    reqwest::Client::new()
        .post(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {}", admin_token))
        .json(&json!({
            "name": format!("ssrf-test-{}", uuid::Uuid::new_v4()),
            "url": bad_url,
            "auth_type": "none",
            "enabled": true,
        }))
        .send()
        .await
        .expect("request failed")
}

#[tokio::test]
async fn test_ssrf_create_rejects_aws_imds_ip() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_repositories::create"],
    )
    .await;

    let res = create_repo_request(
        &server,
        &admin.token,
        "http://169.254.169.254/latest/meta-data/",
    )
    .await;
    assert_eq!(
        res.status(),
        400,
        "AWS IMDS link-local IP must be rejected at create-time"
    );
}

#[tokio::test]
async fn test_ssrf_create_rejects_loopback_ip() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_repositories::create"],
    )
    .await;

    let res = create_repo_request(&server, &admin.token, "http://127.0.0.1/").await;
    assert_eq!(res.status(), 400, "loopback IP must be rejected");
}

#[tokio::test]
async fn test_ssrf_create_rejects_rfc1918_ip() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_repositories::create"],
    )
    .await;

    let res = create_repo_request(&server, &admin.token, "http://10.0.0.1/").await;
    assert_eq!(res.status(), 400, "RFC 1918 IP must be rejected");
}

#[tokio::test]
async fn test_ssrf_create_rejects_file_scheme() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_repositories::create"],
    )
    .await;

    let res = create_repo_request(&server, &admin.token, "file:///etc/passwd").await;
    assert_eq!(res.status(), 400, "file:// scheme must be rejected");
}

#[tokio::test]
async fn test_ssrf_create_rejects_ftp_scheme() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_repositories::create"],
    )
    .await;

    let res = create_repo_request(&server, &admin.token, "ftp://example.com/").await;
    assert_eq!(res.status(), 400, "ftp:// scheme must be rejected");
}

#[tokio::test]
async fn test_ssrf_create_rejects_url_with_credentials() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["llm_repositories::create"],
    )
    .await;

    let res = create_repo_request(&server, &admin.token, "https://user:pass@example.com/").await;
    assert_eq!(
        res.status(),
        400,
        "URL embedding credentials must be rejected"
    );
}

/// Switching a repository's auth_type must PRUNE the previous type's secret from
/// the stored blob (data-at-rest hygiene). Observed indirectly: after switching
/// away from api_key and back, the old api_key must no longer satisfy validation.
#[tokio::test]
async fn test_auth_type_switch_prunes_previous_secret() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "repo_switch_admin",
        &[
            "llm_repositories::create",
            "llm_repositories::edit",
            "llm_repositories::read",
        ],
    )
    .await;

    // 1) Create a custom api_key repo with a secret.
    let create = reqwest::Client::new()
        .post(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "Switch Test Repo",
            "url": "https://example.com/switch-test",
            "auth_type": "api_key",
            "auth_config": { "api_key": "custom-api-key" }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), 201, "create should succeed");
    let repo_id = create.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // 2) Switch to bearer_token (providing the new secret). This must PRUNE the
    //    old api_key from the stored blob.
    let switch = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-repositories/{}", repo_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "auth_type": "bearer_token",
            "auth_config": { "token": "custom-token" }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(switch.status(), 200, "auth_type switch should succeed");

    // 3) Switch BACK to api_key WITHOUT providing a key. If the old api_key was
    //    pruned in step 2, neither the request nor the stored config has one, so
    //    validation rejects (400). If pruning had failed, the stale api_key would
    //    still satisfy validation and this would NOT be a client error.
    let back = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-repositories/{}", repo_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "auth_type": "api_key",
            "auth_config": {}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        back.status(),
        400,
        "switching back to api_key with no key must be rejected because the old \
         api_key was pruned on the earlier switch"
    );
}
