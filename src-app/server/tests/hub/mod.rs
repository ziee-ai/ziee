// ============================================================================
// Hub Module Tests with Permission Checks and Locale Support
// ============================================================================

// ============================================================================
// Hub Models Tests
// ============================================================================

#[tokio::test]
async fn test_get_hub_models_requires_permission() {
    let server = crate::common::TestServer::start().await;

    // Create user with hub::models::read permission
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::models::read"]
    ).await;

    // Create user without permission
    let no_perm_user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "regular",
        &[]
    ).await;

    // User with permission should succeed
    let url = server.api_url("/hub/models?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "User with permission should get models");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(body.is_array(), "Response should be an array of models");
    assert!(body.as_array().unwrap().len() > 0, "Should have at least one model");

    // User without permission should get 403
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", no_perm_user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "User without permission should be forbidden");
}

#[tokio::test]
async fn test_get_hub_models_with_locale() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::models::read"]
    ).await;

    // Test English locale (default)
    let url_en = server.api_url("/hub/models?lang=en");
    let response_en = reqwest::Client::new()
        .get(&url_en)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response_en.status(), 200);
    let body_en: serde_json::Value = response_en.json().await.expect("Failed to parse JSON");

    // Test Vietnamese locale
    let url_vi = server.api_url("/hub/models?lang=vi");
    let response_vi = reqwest::Client::new()
        .get(&url_vi)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response_vi.status(), 200);
    let body_vi: serde_json::Value = response_vi.json().await.expect("Failed to parse JSON");

    // Both should have same number of models
    assert_eq!(
        body_en.as_array().unwrap().len(),
        body_vi.as_array().unwrap().len(),
        "Both locales should have same number of models"
    );

    // Verify that locale files are being loaded (check for translated content if available)
    // Find a model that has translations in vi.json (e.g., llama-3-1-8b-instruct)
    let models_en = body_en.as_array().unwrap();
    let models_vi = body_vi.as_array().unwrap();

    // Find llama-3-1-8b-instruct in both arrays
    let llama_en = models_en.iter().find(|m| m.get("id").and_then(|v| v.as_str()) == Some("llama-3-1-8b-instruct"));
    let llama_vi = models_vi.iter().find(|m| m.get("id").and_then(|v| v.as_str()) == Some("llama-3-1-8b-instruct"));

    if let (Some(model_en), Some(model_vi)) = (llama_en, llama_vi) {
        let desc_en = model_en.get("description").and_then(|v| v.as_str());
        let desc_vi = model_vi.get("description").and_then(|v| v.as_str());

        // If both have descriptions, they should be different (Vietnamese translation)
        if desc_en.is_some() && desc_vi.is_some() {
            assert_ne!(desc_en, desc_vi, "Descriptions should be translated for llama-3-1-8b-instruct");
        }
    }
}

#[tokio::test]
async fn test_get_hub_models_response_structure() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::models::read"]
    ).await;

    let url = server.api_url("/hub/models?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let models: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(models.is_array(), "Response should be an array");

    let first_model = models.as_array().unwrap().first().expect("Should have at least one model");

    // Verify model structure
    assert!(first_model.get("id").and_then(|v| v.as_str()).is_some(), "Model should have id");
    assert!(first_model.get("name").and_then(|v| v.as_str()).is_some(), "Model should have name");
    assert!(first_model.get("display_name").and_then(|v| v.as_str()).is_some(), "Model should have display_name");
    assert!(first_model.get("repository_url").and_then(|v| v.as_str()).is_some(), "Model should have repository_url");
    assert!(first_model.get("file_format").and_then(|v| v.as_str()).is_some(), "Model should have file_format");
    assert!(first_model.get("size_gb").and_then(|v| v.as_f64()).is_some(), "Model should have size_gb");
    assert!(first_model.get("tags").and_then(|v| v.as_array()).is_some(), "Model should have tags array");
    assert!(first_model.get("popularity_score").and_then(|v| v.as_i64()).is_some(), "Model should have popularity_score");
}

#[tokio::test]
async fn test_get_hub_models_version_requires_permission() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::models::read_version"]
    ).await;

    let no_perm_user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "regular",
        &[]
    ).await;

    // User with permission should succeed
    let url = server.api_url("/hub/models/version");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "User with permission should get version");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(body.get("version").and_then(|v| v.as_str()).is_some(), "Should have version string");

    // User without permission should get 403
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", no_perm_user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "User without permission should be forbidden");
}

#[tokio::test]
async fn test_refresh_hub_models_requires_permission() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_admin",
        &["hub::models::refresh"]
    ).await;

    let no_perm_user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "regular",
        &[]
    ).await;

    // User with permission should succeed (though may fail due to GitHub)
    let url = server.api_url("/hub/models/refresh");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    // Could be 200 (success) or 500 (GitHub fetch failed), both acceptable
    assert!(
        response.status() == 200 || response.status() == 500,
        "Should return 200 or 500 for refresh attempt"
    );

    // User without permission should get 403
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", no_perm_user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "User without permission should be forbidden");
}

// ============================================================================
// Hub Assistants Tests
// ============================================================================

#[tokio::test]
async fn test_get_hub_assistants_requires_permission() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::assistants::read"]
    ).await;

    let no_perm_user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "regular",
        &[]
    ).await;

    // User with permission should succeed
    let url = server.api_url("/hub/assistants?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "User with permission should get assistants");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(body.is_array(), "Response should be an array of assistants");
    assert!(body.as_array().unwrap().len() > 0, "Should have at least one assistant");

    // User without permission should get 403
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", no_perm_user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "User without permission should be forbidden");
}

#[tokio::test]
async fn test_get_hub_assistants_response_structure() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::assistants::read"]
    ).await;

    let url = server.api_url("/hub/assistants?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let assistants: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(assistants.is_array(), "Response should be an array");

    let first_assistant = assistants.as_array().unwrap().first().expect("Should have at least one assistant");

    // Verify assistant structure
    assert!(first_assistant.get("id").and_then(|v| v.as_str()).is_some(), "Assistant should have id");
    assert!(first_assistant.get("name").and_then(|v| v.as_str()).is_some(), "Assistant should have name");
    assert!(first_assistant.get("display_name").and_then(|v| v.as_str()).is_some(), "Assistant should have display_name");
    assert!(first_assistant.get("parameters").is_some(), "Assistant should have parameters");
    assert!(first_assistant.get("tags").and_then(|v| v.as_array()).is_some(), "Assistant should have tags array");
    assert!(first_assistant.get("popularity_score").and_then(|v| v.as_i64()).is_some(), "Assistant should have popularity_score");
}

#[tokio::test]
async fn test_get_hub_assistants_with_locale() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::assistants::read"]
    ).await;

    // Test Chinese locale
    let url_zh = server.api_url("/hub/assistants?lang=zh");
    let response_zh = reqwest::Client::new()
        .get(&url_zh)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response_zh.status(), 200);
    let body_zh: serde_json::Value = response_zh.json().await.expect("Failed to parse JSON");
    assert!(body_zh.is_array(), "Response should be an array");
    assert!(body_zh.as_array().unwrap().len() > 0, "Should have assistants");
}

#[tokio::test]
async fn test_get_hub_assistants_version_requires_permission() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::assistants::read_version"]
    ).await;

    let url = server.api_url("/hub/assistants/version");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "User with permission should get version");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(body.get("version").and_then(|v| v.as_str()).is_some(), "Should have version string");
}

#[tokio::test]
async fn test_refresh_hub_assistants_requires_permission() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_admin",
        &["hub::assistants::refresh"]
    ).await;

    let no_perm_user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "regular",
        &[]
    ).await;

    // User with permission should succeed (though may fail due to GitHub)
    let url = server.api_url("/hub/assistants/refresh");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert!(
        response.status() == 200 || response.status() == 500,
        "Should return 200 or 500 for refresh attempt"
    );

    // User without permission should get 403
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", no_perm_user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "User without permission should be forbidden");
}

// ============================================================================
// Hub MCP Servers Tests
// ============================================================================

#[tokio::test]
async fn test_get_hub_mcp_servers_requires_permission() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::mcp_servers::read"]
    ).await;

    let no_perm_user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "regular",
        &[]
    ).await;

    // User with permission should succeed
    let url = server.api_url("/hub/mcp-servers?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "User with permission should get MCP servers");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(body.is_array(), "Response should be an array of MCP servers");
    assert!(body.as_array().unwrap().len() > 0, "Should have at least one MCP server");

    // User without permission should get 403
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", no_perm_user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "User without permission should be forbidden");
}

#[tokio::test]
async fn test_get_hub_mcp_servers_response_structure() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::mcp_servers::read"]
    ).await;

    let url = server.api_url("/hub/mcp-servers?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let servers: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(servers.is_array(), "Response should be an array");

    let first_server = servers.as_array().unwrap().first().expect("Should have at least one MCP server");

    // Verify MCP server structure
    assert!(first_server.get("id").and_then(|v| v.as_str()).is_some(), "Server should have id");
    assert!(first_server.get("name").and_then(|v| v.as_str()).is_some(), "Server should have name");
    assert!(first_server.get("display_name").and_then(|v| v.as_str()).is_some(), "Server should have display_name");
    // command and args are optional (for HTTP transport servers)
    assert!(first_server.get("tags").and_then(|v| v.as_array()).is_some(), "Server should have tags array");
    assert!(first_server.get("popularity_score").and_then(|v| v.as_f64()).is_some(), "Server should have popularity_score");
}

#[tokio::test]
async fn test_get_hub_mcp_servers_version_requires_permission() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::mcp_servers::read_version"]
    ).await;

    let url = server.api_url("/hub/mcp-servers/version");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "User with permission should get version");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(body.get("version").and_then(|v| v.as_str()).is_some(), "Should have version string");
}

#[tokio::test]
async fn test_refresh_hub_mcp_servers_requires_permission() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_admin",
        &["hub::mcp_servers::refresh"]
    ).await;

    let no_perm_user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "regular",
        &[]
    ).await;

    // User with permission should succeed (though may fail due to GitHub)
    let url = server.api_url("/hub/mcp-servers/refresh");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert!(
        response.status() == 200 || response.status() == 500,
        "Should return 200 or 500 for refresh attempt"
    );

    // User without permission should get 403
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", no_perm_user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "User without permission should be forbidden");
}

// ============================================================================
// Unauthorized Access Tests
// ============================================================================

#[tokio::test]
async fn test_hub_endpoints_require_authentication() {
    let server = crate::common::TestServer::start().await;

    let endpoints = vec![
        "/hub/models?lang=en",
        "/hub/models/version",
        "/hub/assistants?lang=en",
        "/hub/assistants/version",
        "/hub/mcp-servers?lang=en",
        "/hub/mcp-servers/version",
    ];

    for endpoint in endpoints {
        let url = server.api_url(endpoint);
        let response = reqwest::Client::new()
            .get(&url)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(
            response.status(),
            401,
            "Endpoint {} should require authentication",
            endpoint
        );
    }

    // Test POST endpoints
    let post_endpoints = vec![
        "/hub/models/refresh",
        "/hub/assistants/refresh",
        "/hub/mcp-servers/refresh",
    ];

    for endpoint in post_endpoints {
        let url = server.api_url(endpoint);
        let response = reqwest::Client::new()
            .post(&url)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(
            response.status(),
            401,
            "Endpoint {} should require authentication",
            endpoint
        );
    }
}

// ============================================================================
// Hub Entity Tracking Tests
// ============================================================================

#[tokio::test]
async fn test_create_assistant_from_hub() {
    let server = crate::common::TestServer::start().await;

    // Create user with necessary permissions
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::assistants::create", "hub::assistants::read"]
    ).await;

    // Get available hub assistants
    let url = server.api_url("/hub/assistants?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);
    let assistants: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(assistants.as_array().unwrap().len() > 0, "Should have at least one hub assistant");

    // Get first assistant hub_id
    let first_assistant = &assistants.as_array().unwrap()[0];
    let hub_id = first_assistant.get("id").and_then(|v| v.as_str()).unwrap();

    // Verify created_ids is initially empty
    let created_ids = first_assistant.get("created_ids").and_then(|v| v.as_array());
    assert!(created_ids.is_none() || created_ids.unwrap().is_empty(),
        "Created IDs should be empty initially");

    // Create assistant from hub
    let url = server.api_url("/hub/assistants/create");
    let request_body = serde_json::json!({
        "hub_id": hub_id,
        "is_default": false,
        "enabled": true
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201, "Should create assistant successfully");
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");

    // Verify response structure
    assert!(body.get("assistant").is_some(), "Response should contain assistant");
    assert!(body.get("hub_tracking").is_some(), "Response should contain hub_tracking");

    let assistant_id = body.get("assistant")
        .and_then(|a| a.get("id"))
        .and_then(|v| v.as_str())
        .expect("Should have assistant ID");

    // Verify hub tracking
    let hub_tracking = body.get("hub_tracking").unwrap();
    assert_eq!(
        hub_tracking.get("entity_type").and_then(|v| v.as_str()).unwrap(),
        "assistant"
    );
    assert_eq!(
        hub_tracking.get("hub_id").and_then(|v| v.as_str()).unwrap(),
        hub_id
    );
    assert_eq!(
        hub_tracking.get("hub_category").and_then(|v| v.as_str()).unwrap(),
        "assistant"
    );

    // Get hub assistants again and verify created_ids is populated
    let url = server.api_url("/hub/assistants?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);
    let assistants: serde_json::Value = response.json().await.expect("Failed to parse JSON");

    let updated_assistant = assistants.as_array().unwrap()
        .iter()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .expect("Should find the hub assistant");

    let created_ids = updated_assistant.get("created_ids")
        .and_then(|v| v.as_array())
        .expect("Should have created_ids");

    assert_eq!(created_ids.len(), 1, "Should have exactly one created ID");
    assert_eq!(
        created_ids[0].as_str().unwrap(),
        assistant_id,
        "Created ID should match the assistant we just created"
    );
}

#[tokio::test]
async fn test_create_mcp_server_from_hub() {
    let server = crate::common::TestServer::start().await;

    // Create user with necessary permissions
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::mcp_servers::create", "hub::mcp_servers::read"]
    ).await;

    // Get available hub MCP servers
    let url = server.api_url("/hub/mcp-servers?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);
    let servers: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(servers.as_array().unwrap().len() > 0, "Should have at least one hub MCP server");

    // Get first server hub_id
    let first_server = &servers.as_array().unwrap()[0];
    let hub_id = first_server.get("id").and_then(|v| v.as_str()).unwrap();

    // Verify created_ids is initially empty
    let created_ids = first_server.get("created_ids").and_then(|v| v.as_array());
    assert!(created_ids.is_none() || created_ids.unwrap().is_empty(),
        "Created IDs should be empty initially");

    // Create MCP server from hub
    let url = server.api_url("/hub/mcp-servers/create");
    let request_body = serde_json::json!({
        "hub_id": hub_id,
        "enabled": true
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201, "Should create MCP server successfully");
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");

    // Verify response structure
    assert!(body.get("server").is_some(), "Response should contain server");
    assert!(body.get("hub_tracking").is_some(), "Response should contain hub_tracking");

    let server_id = body.get("server")
        .and_then(|s| s.get("id"))
        .and_then(|v| v.as_str())
        .expect("Should have server ID");

    // Verify server is created as user server (not system server)
    let is_system = body.get("server")
        .and_then(|s| s.get("is_system"))
        .and_then(|v| v.as_bool())
        .expect("Should have is_system field");
    assert!(!is_system, "Hub-created servers should be user servers, not system servers");

    // Verify hub tracking
    let hub_tracking = body.get("hub_tracking").unwrap();
    assert_eq!(
        hub_tracking.get("entity_type").and_then(|v| v.as_str()).unwrap(),
        "mcp_server"
    );
    assert_eq!(
        hub_tracking.get("hub_id").and_then(|v| v.as_str()).unwrap(),
        hub_id
    );
    assert_eq!(
        hub_tracking.get("hub_category").and_then(|v| v.as_str()).unwrap(),
        "mcp_server"
    );

    // Get hub MCP servers again and verify created_ids is populated
    let url = server.api_url("/hub/mcp-servers?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);
    let servers: serde_json::Value = response.json().await.expect("Failed to parse JSON");

    let updated_server = servers.as_array().unwrap()
        .iter()
        .find(|s| s.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .expect("Should find the hub MCP server");

    let created_ids = updated_server.get("created_ids")
        .and_then(|v| v.as_array())
        .expect("Should have created_ids");

    assert_eq!(created_ids.len(), 1, "Should have exactly one created ID");
    assert_eq!(
        created_ids[0].as_str().unwrap(),
        server_id,
        "Created ID should match the server we just created"
    );
}

#[tokio::test]
async fn test_created_ids_are_user_specific() {
    let server = crate::common::TestServer::start().await;

    // Create two users with necessary permissions
    let user1 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user1",
        &["hub::assistants::create", "hub::assistants::read"]
    ).await;

    let user2 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user2",
        &["hub::assistants::create", "hub::assistants::read"]
    ).await;

    // Get hub assistants for user1
    let url = server.api_url("/hub/assistants?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);
    let assistants: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let hub_id = assistants.as_array().unwrap()[0]
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap();

    // User1 creates an assistant from hub
    let url = server.api_url("/hub/assistants/create");
    let request_body = serde_json::json!({
        "hub_id": hub_id,
        "is_default": false,
        "enabled": true
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user1.token))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201);
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let user1_assistant_id = body.get("assistant")
        .and_then(|a| a.get("id"))
        .and_then(|v| v.as_str())
        .unwrap();

    // User2 creates an assistant from the same hub
    let response = reqwest::Client::new()
        .post(server.api_url("/hub/assistants/create"))
        .header("Authorization", format!("Bearer {}", user2.token))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201);
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let user2_assistant_id = body.get("assistant")
        .and_then(|a| a.get("id"))
        .and_then(|v| v.as_str())
        .unwrap();

    // Verify different assistant IDs
    assert_ne!(user1_assistant_id, user2_assistant_id, "Each user should get their own assistant instance");

    // User1 sees only their own created assistant
    let url = server.api_url("/hub/assistants?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);
    let assistants: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let assistant = assistants.as_array().unwrap()
        .iter()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .unwrap();

    let created_ids = assistant.get("created_ids")
        .and_then(|v| v.as_array())
        .unwrap();

    assert_eq!(created_ids.len(), 1);
    assert_eq!(created_ids[0].as_str().unwrap(), user1_assistant_id);

    // User2 sees only their own created assistant
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user2.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);
    let assistants: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let assistant = assistants.as_array().unwrap()
        .iter()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .unwrap();

    let created_ids = assistant.get("created_ids")
        .and_then(|v| v.as_array())
        .unwrap();

    assert_eq!(created_ids.len(), 1);
    assert_eq!(created_ids[0].as_str().unwrap(), user2_assistant_id);
}

#[tokio::test]
async fn test_multiple_creations_from_same_hub_item() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::assistants::create", "hub::assistants::read"]
    ).await;

    // Get hub assistants
    let url = server.api_url("/hub/assistants?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    let assistants: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let hub_id = assistants.as_array().unwrap()[0]
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap();

    // Create multiple assistants from the same hub item
    let mut assistant_ids = Vec::new();

    for i in 0..3 {
        let url = server.api_url("/hub/assistants/create");
        let request_body = serde_json::json!({
            "hub_id": hub_id,
            "name": format!("Custom Assistant {}", i),
            "is_default": false,
            "enabled": true
        });

        let response = reqwest::Client::new()
            .post(&url)
            .header("Authorization", format!("Bearer {}", user.token))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(response.status(), 201, "Should create assistant successfully");
        let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
        let assistant_id = body.get("assistant")
            .and_then(|a| a.get("id"))
            .and_then(|v| v.as_str())
            .unwrap();

        assistant_ids.push(assistant_id.to_string());
    }

    // Verify all three assistant IDs are tracked
    let url = server.api_url("/hub/assistants?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    let assistants: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let assistant = assistants.as_array().unwrap()
        .iter()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .unwrap();

    let created_ids = assistant.get("created_ids")
        .and_then(|v| v.as_array())
        .unwrap();

    assert_eq!(created_ids.len(), 3, "Should track all three created assistants");

    // Verify all IDs are present
    for assistant_id in assistant_ids {
        assert!(
            created_ids.iter().any(|id| id.as_str() == Some(&assistant_id)),
            "Created ID {} should be in the list",
            assistant_id
        );
    }
}

// ============================================================================
// Event Bus Integration Tests - Hub Entity Cleanup on Deletion
// ============================================================================

#[tokio::test]
async fn test_hub_entity_cleaned_up_when_assistant_deleted() {
    let server = crate::common::TestServer::start().await;

    // Create user with necessary permissions
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::assistants::create", "hub::assistants::read", "assistants::delete"]
    ).await;

    // Get hub assistants
    let url = server.api_url("/hub/assistants?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    let assistants: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let hub_id = assistants.as_array().unwrap()[0]
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap();

    // Create assistant from hub
    let url = server.api_url("/hub/assistants/create");
    let request_body = serde_json::json!({
        "hub_id": hub_id,
        "is_default": false,
        "enabled": true
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201);
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let assistant_id = body.get("assistant")
        .and_then(|a| a.get("id"))
        .and_then(|v| v.as_str())
        .unwrap();

    // Verify hub entity tracking exists
    let url = server.api_url("/hub/assistants?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    let assistants: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let assistant = assistants.as_array().unwrap()
        .iter()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .unwrap();

    let created_ids = assistant.get("created_ids")
        .and_then(|v| v.as_array())
        .unwrap();

    assert_eq!(created_ids.len(), 1, "Should have hub tracking before deletion");
    assert_eq!(created_ids[0].as_str().unwrap(), assistant_id);

    // Delete the assistant
    let url = server.api_url(&format!("/assistants/{}", assistant_id));
    let response = reqwest::Client::new()
        .delete(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 204, "Should delete assistant successfully");

    // Give event handler time to process deletion event
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify hub entity tracking is removed
    let url = server.api_url("/hub/assistants?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    let assistants: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let assistant = assistants.as_array().unwrap()
        .iter()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .unwrap();

    let created_ids = assistant.get("created_ids").and_then(|v| v.as_array());

    assert!(
        created_ids.is_none() || created_ids.unwrap().is_empty(),
        "Hub tracking should be cleaned up after assistant deletion"
    );
}

#[tokio::test]
async fn test_hub_entity_cleaned_up_when_user_mcp_server_deleted() {
    let server = crate::common::TestServer::start().await;

    // Create user with necessary permissions
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::mcp_servers::create", "hub::mcp_servers::read", "mcp_servers::delete"]
    ).await;

    // Get hub MCP servers
    let url = server.api_url("/hub/mcp-servers?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    let servers: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let hub_id = servers.as_array().unwrap()[0]
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap();

    // Create MCP server from hub
    let url = server.api_url("/hub/mcp-servers/create");
    let request_body = serde_json::json!({
        "hub_id": hub_id,
        "enabled": true
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201);
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let server_id = body.get("server")
        .and_then(|s| s.get("id"))
        .and_then(|v| v.as_str())
        .unwrap();

    // Verify hub entity tracking exists
    let url = server.api_url("/hub/mcp-servers?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    let servers: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let mcp_server = servers.as_array().unwrap()
        .iter()
        .find(|s| s.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .unwrap();

    let created_ids = mcp_server.get("created_ids")
        .and_then(|v| v.as_array())
        .unwrap();

    assert_eq!(created_ids.len(), 1, "Should have hub tracking before deletion");
    assert_eq!(created_ids[0].as_str().unwrap(), server_id);

    // Delete the MCP server
    let url = server.api_url(&format!("/mcp/servers/{}", server_id));
    let response = reqwest::Client::new()
        .delete(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 204, "Should delete MCP server successfully");

    // Give event handler time to process deletion event
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify hub entity tracking is removed
    let url = server.api_url("/hub/mcp-servers?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    let servers: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let mcp_server = servers.as_array().unwrap()
        .iter()
        .find(|s| s.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .unwrap();

    let created_ids = mcp_server.get("created_ids").and_then(|v| v.as_array());

    assert!(
        created_ids.is_none() || created_ids.unwrap().is_empty(),
        "Hub tracking should be cleaned up after MCP server deletion"
    );
}

#[tokio::test]
async fn test_multiple_hub_entities_cleanup_when_multiple_assistants_deleted() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::assistants::create", "hub::assistants::read", "assistants::delete"]
    ).await;

    // Get hub assistants
    let url = server.api_url("/hub/assistants?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    let assistants: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let hub_id = assistants.as_array().unwrap()[0]
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap();

    // Create 3 assistants from the same hub item
    let mut assistant_ids = Vec::new();
    for i in 0..3 {
        let url = server.api_url("/hub/assistants/create");
        let request_body = serde_json::json!({
            "hub_id": hub_id,
            "name": format!("Test Assistant {}", i),
            "is_default": false,
            "enabled": true
        });

        let response = reqwest::Client::new()
            .post(&url)
            .header("Authorization", format!("Bearer {}", user.token))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(response.status(), 201);
        let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
        let assistant_id = body.get("assistant")
            .and_then(|a| a.get("id"))
            .and_then(|v| v.as_str())
            .unwrap()
            .to_string();

        assistant_ids.push(assistant_id);
    }

    // Verify all 3 are tracked
    let url = server.api_url("/hub/assistants?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    let assistants: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let assistant = assistants.as_array().unwrap()
        .iter()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .unwrap();

    let created_ids = assistant.get("created_ids")
        .and_then(|v| v.as_array())
        .unwrap();

    assert_eq!(created_ids.len(), 3, "Should track all 3 assistants");

    // Delete the first assistant
    let url = server.api_url(&format!("/assistants/{}", assistant_ids[0]));
    let response = reqwest::Client::new()
        .delete(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 204);

    // Give event handler time to process
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify only 2 are tracked now
    let url = server.api_url("/hub/assistants?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    let assistants: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let assistant = assistants.as_array().unwrap()
        .iter()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .unwrap();

    let created_ids = assistant.get("created_ids")
        .and_then(|v| v.as_array())
        .unwrap();

    assert_eq!(created_ids.len(), 2, "Should have 2 assistants after deleting 1");
    assert!(!created_ids.iter().any(|id| id.as_str() == Some(&assistant_ids[0])),
        "Deleted assistant should not be in tracking");
    assert!(created_ids.iter().any(|id| id.as_str() == Some(&assistant_ids[1])),
        "Second assistant should still be tracked");
    assert!(created_ids.iter().any(|id| id.as_str() == Some(&assistant_ids[2])),
        "Third assistant should still be tracked");

    // Delete remaining two
    for i in 1..3 {
        let url = server.api_url(&format!("/assistants/{}", assistant_ids[i]));
        let response = reqwest::Client::new()
            .delete(&url)
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .expect("Request failed");

        assert_eq!(response.status(), 204);
    }

    // Give event handler time to process
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify all tracking is cleaned up
    let url = server.api_url("/hub/assistants?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    let assistants: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let assistant = assistants.as_array().unwrap()
        .iter()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .unwrap();

    let created_ids = assistant.get("created_ids").and_then(|v| v.as_array());

    assert!(
        created_ids.is_none() || created_ids.unwrap().is_empty(),
        "All hub tracking should be cleaned up after deleting all assistants"
    );
}
