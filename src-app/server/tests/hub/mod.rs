// ============================================================================
// Hub Module Tests with Permission Checks and Locale Support
// ============================================================================

// Phase 1 — unified catalog endpoints (GET /hub/{index,version,manifest},
// POST /hub/refresh, GET /hub/installed). Kept in a separate file because
// the legacy suite below is large and locale-focused.
mod catalog_v1;
// Hermetic catalog tests (mock release server, no network/cosign).
mod catalog_hermetic;
mod mock_release_server;
// Realtime-sync emission for the `hub_settings` entity (reuses the hermetic
// mock release server to drive POST /hub/activate).
mod sync_emit_test;

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
        &["hub::models::read"],
    )
    .await;

    // Create user without permission
    let no_perm_user =
        crate::common::test_helpers::create_user_with_no_permissions(&server, "regular").await;

    // User with permission should succeed
    let url = server.api_url("/hub/models?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        200,
        "User with permission should get models"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(body.is_array(), "Response should be an array of models");
    assert!(
        !body.as_array().unwrap().is_empty(),
        "Should have at least one model"
    );

    // User without permission should get 403
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", no_perm_user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        403,
        "User without permission should be forbidden"
    );
}

// Locale tests are deprecated by the unified catalog: the new hub ships
// English-only (Phase 1 plan). Localization (per-language manifest
// overrides) is deferred — until it returns, these tests would fail
// against the seed catalog where every description is English.
#[tokio::test]
#[ignore = "locale support deferred — new catalog is English-only at v1"]
async fn test_get_hub_models_with_locale() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::models::read"],
    )
    .await;

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
    let llama_en = models_en
        .iter()
        .find(|m| m.get("id").and_then(|v| v.as_str()) == Some("llama-3-1-8b-instruct"));
    let llama_vi = models_vi
        .iter()
        .find(|m| m.get("id").and_then(|v| v.as_str()) == Some("llama-3-1-8b-instruct"));

    if let (Some(model_en), Some(model_vi)) = (llama_en, llama_vi) {
        let desc_en = model_en.get("description").and_then(|v| v.as_str());
        let desc_vi = model_vi.get("description").and_then(|v| v.as_str());

        // If both have descriptions, they should be different (Vietnamese translation)
        if desc_en.is_some() && desc_vi.is_some() {
            assert_ne!(
                desc_en, desc_vi,
                "Descriptions should be translated for llama-3-1-8b-instruct"
            );
        }
    }
}

#[tokio::test]
async fn test_get_hub_models_response_structure() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::models::read"],
    )
    .await;

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

    let first_model = models
        .as_array()
        .unwrap()
        .first()
        .expect("Should have at least one model");

    // Verify model structure
    assert!(
        first_model.get("id").and_then(|v| v.as_str()).is_some(),
        "Model should have id"
    );
    assert!(
        first_model.get("name").and_then(|v| v.as_str()).is_some(),
        "Model should have name"
    );
    assert!(
        first_model
            .get("display_name")
            .and_then(|v| v.as_str())
            .is_some(),
        "Model should have display_name"
    );
    assert!(
        first_model
            .get("repository_url")
            .and_then(|v| v.as_str())
            .is_some(),
        "Model should have repository_url"
    );
    assert!(
        first_model
            .get("file_format")
            .and_then(|v| v.as_str())
            .is_some(),
        "Model should have file_format"
    );
    assert!(
        first_model
            .get("size_gb")
            .and_then(|v| v.as_f64())
            .is_some(),
        "Model should have size_gb"
    );
    assert!(
        first_model.get("tags").and_then(|v| v.as_array()).is_some(),
        "Model should have tags array"
    );
    assert!(
        first_model
            .get("popularity_score")
            .and_then(|v| v.as_f64())
            .is_some(),
        "Model should have popularity_score"
    );
}

#[tokio::test]
async fn test_get_hub_models_version_requires_permission() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::models::read_version"],
    )
    .await;

    let no_perm_user =
        crate::common::test_helpers::create_user_with_no_permissions(&server, "regular").await;

    // User with permission should succeed
    let url = server.api_url("/hub/models/version");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        200,
        "User with permission should get version"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(
        body.get("version").and_then(|v| v.as_str()).is_some(),
        "Should have version string"
    );

    // User without permission should get 403
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", no_perm_user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        403,
        "User without permission should be forbidden"
    );
}

#[tokio::test]
async fn test_refresh_hub_models_requires_permission() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_admin",
        &["hub::models::refresh"],
    )
    .await;

    let no_perm_user =
        crate::common::test_helpers::create_user_with_no_permissions(&server, "regular").await;

    // User with permission should succeed (though may fail due to GitHub)
    let url = server.api_url("/hub/models/refresh");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    // Refresh against the placeholder GITHUB_HUB_REPO is now blocked
    // pre-network with 400 HUB_NOT_CONFIGURED (closes 11-hub F-01).
    // 200 (success) and 500 (network failure against a real URL) remain
    // acceptable for configured deployments.
    assert!(
        response.status() == 200 || response.status() == 400 || response.status() == 500,
        "Should return 200 / 400 / 500 for refresh attempt, got {}",
        response.status()
    );

    // User without permission should get 403
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", no_perm_user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        403,
        "User without permission should be forbidden"
    );
}

// ============================================================================
// Hub Models Auth Required Tests
// ============================================================================

#[tokio::test]
async fn test_hub_models_have_auth_required_field() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::models::read"],
    )
    .await;

    let url = server.api_url("/hub/models?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    let status = response.status();
    if status != 200 {
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "Could not read error".to_string());
        panic!("Expected 200, got {}: {}", status, error_body);
    }

    let models: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(models.is_array(), "Response should be an array");

    let models_array = models.as_array().unwrap();
    assert!(!models_array.is_empty(), "Should have at least one model");

    // Verify all models have auth_required field
    for model in models_array {
        let model_id = model
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        assert!(
            model.get("auth_required").is_some(),
            "Model {} should have auth_required field",
            model_id
        );
    }
}

#[tokio::test]
async fn test_hub_models_auth_required_defaults_to_true() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::models::read"],
    )
    .await;

    let url = server.api_url("/hub/models?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let models: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let models_array = models.as_array().unwrap();

    // Verify all current models have auth_required set to true
    // (as per configuration in base.json)
    for model in models_array {
        let model_id = model
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let auth_required = model.get("auth_required").and_then(|v| v.as_bool());

        assert_eq!(
            auth_required,
            Some(true),
            "Model {} should have auth_required: true",
            model_id
        );
    }
}

#[tokio::test]
async fn test_hub_models_auth_required_field_type() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::models::read"],
    )
    .await;

    let url = server.api_url("/hub/models?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let models: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let first_model = models
        .as_array()
        .unwrap()
        .first()
        .expect("Should have at least one model");

    // Verify auth_required is a boolean
    let auth_required = first_model.get("auth_required");
    assert!(
        auth_required.is_some(),
        "Model should have auth_required field"
    );
    assert!(
        auth_required.unwrap().is_boolean(),
        "auth_required should be a boolean type"
    );
}

#[tokio::test]
async fn test_hub_models_auth_required_in_all_locales() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::models::read"],
    )
    .await;

    let locales = vec!["en", "zh", "vi"];

    for locale in locales {
        let url = server.api_url(&format!("/hub/models?lang={}", locale));
        let response = reqwest::Client::new()
            .get(&url)
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .expect("Request failed");

        assert_eq!(
            response.status(),
            200,
            "Should succeed for locale {}",
            locale
        );

        let models: serde_json::Value = response.json().await.expect("Failed to parse JSON");
        let models_array = models.as_array().unwrap();

        // Verify all models in this locale have auth_required field
        for model in models_array {
            let model_id = model
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            assert!(
                model.get("auth_required").is_some(),
                "Model {} in locale {} should have auth_required field",
                model_id,
                locale
            );

            // All current models should have auth_required: true
            let auth_required = model.get("auth_required").and_then(|v| v.as_bool());
            assert_eq!(
                auth_required,
                Some(true),
                "Model {} in locale {} should have auth_required: true",
                model_id,
                locale
            );
        }
    }
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
        &["hub::assistants::read"],
    )
    .await;

    let no_perm_user =
        crate::common::test_helpers::create_user_with_no_permissions(&server, "regular").await;

    // User with permission should succeed
    let url = server.api_url("/hub/assistants?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        200,
        "User with permission should get assistants"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(body.is_array(), "Response should be an array of assistants");
    assert!(
        !body.as_array().unwrap().is_empty(),
        "Should have at least one assistant"
    );

    // User without permission should get 403
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", no_perm_user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        403,
        "User without permission should be forbidden"
    );
}

#[tokio::test]
async fn test_get_hub_assistants_response_structure() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::assistants::read"],
    )
    .await;

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

    let first_assistant = assistants
        .as_array()
        .unwrap()
        .first()
        .expect("Should have at least one assistant");

    // Verify assistant structure
    assert!(
        first_assistant.get("id").and_then(|v| v.as_str()).is_some(),
        "Assistant should have id"
    );
    assert!(
        first_assistant
            .get("name")
            .and_then(|v| v.as_str())
            .is_some(),
        "Assistant should have name"
    );
    assert!(
        first_assistant
            .get("display_name")
            .and_then(|v| v.as_str())
            .is_some(),
        "Assistant should have display_name"
    );
    assert!(
        first_assistant.get("parameters").is_some(),
        "Assistant should have parameters"
    );
    assert!(
        first_assistant
            .get("tags")
            .and_then(|v| v.as_array())
            .is_some(),
        "Assistant should have tags array"
    );
    assert!(
        first_assistant
            .get("popularity_score")
            .and_then(|v| v.as_f64())
            .is_some(),
        "Assistant should have popularity_score"
    );
}

#[tokio::test]
async fn test_get_hub_assistants_with_locale() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::assistants::read"],
    )
    .await;

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
    assert!(
        !body_zh.as_array().unwrap().is_empty(),
        "Should have assistants"
    );
}

#[tokio::test]
async fn test_get_hub_assistants_version_requires_permission() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::assistants::read_version"],
    )
    .await;

    let url = server.api_url("/hub/assistants/version");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        200,
        "User with permission should get version"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(
        body.get("version").and_then(|v| v.as_str()).is_some(),
        "Should have version string"
    );
}

#[tokio::test]
async fn test_refresh_hub_assistants_requires_permission() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_admin",
        &["hub::assistants::refresh"],
    )
    .await;

    let no_perm_user =
        crate::common::test_helpers::create_user_with_no_permissions(&server, "regular").await;

    // User with permission should succeed (though may fail due to GitHub)
    let url = server.api_url("/hub/assistants/refresh");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    // Refresh against the placeholder GITHUB_HUB_REPO is blocked
    // pre-network with 400 HUB_NOT_CONFIGURED (closes 11-hub F-01).
    assert!(
        response.status() == 200 || response.status() == 400 || response.status() == 500,
        "Should return 200 / 400 / 500 for refresh attempt, got {}",
        response.status()
    );

    // User without permission should get 403
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", no_perm_user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        403,
        "User without permission should be forbidden"
    );
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
        &["hub::mcp_servers::read"],
    )
    .await;

    let no_perm_user =
        crate::common::test_helpers::create_user_with_no_permissions(&server, "regular").await;

    // User with permission should succeed
    let url = server.api_url("/hub/mcp-servers?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        200,
        "User with permission should get MCP servers"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(
        body.is_array(),
        "Response should be an array of MCP servers"
    );
    assert!(
        !body.as_array().unwrap().is_empty(),
        "Should have at least one MCP server"
    );

    // User without permission should get 403
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", no_perm_user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        403,
        "User without permission should be forbidden"
    );
}

#[tokio::test]
async fn test_get_hub_mcp_servers_response_structure() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::mcp_servers::read"],
    )
    .await;

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

    let first_server = servers
        .as_array()
        .unwrap()
        .first()
        .expect("Should have at least one MCP server");

    // Verify MCP server structure
    assert!(
        first_server.get("id").and_then(|v| v.as_str()).is_some(),
        "Server should have id"
    );
    assert!(
        first_server.get("name").and_then(|v| v.as_str()).is_some(),
        "Server should have name"
    );
    assert!(
        first_server
            .get("display_name")
            .and_then(|v| v.as_str())
            .is_some(),
        "Server should have display_name"
    );
    // command and args are optional (for HTTP transport servers)
    assert!(
        first_server
            .get("tags")
            .and_then(|v| v.as_array())
            .is_some(),
        "Server should have tags array"
    );
    assert!(
        first_server
            .get("popularity_score")
            .and_then(|v| v.as_f64())
            .is_some(),
        "Server should have popularity_score"
    );
}

#[tokio::test]
async fn test_get_hub_mcp_servers_version_requires_permission() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &["hub::mcp_servers::read_version"],
    )
    .await;

    let url = server.api_url("/hub/mcp-servers/version");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        200,
        "User with permission should get version"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(
        body.get("version").and_then(|v| v.as_str()).is_some(),
        "Should have version string"
    );
}

#[tokio::test]
async fn test_refresh_hub_mcp_servers_requires_permission() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_admin",
        &["hub::mcp_servers::refresh"],
    )
    .await;

    let no_perm_user =
        crate::common::test_helpers::create_user_with_no_permissions(&server, "regular").await;

    // User with permission should succeed (though may fail due to GitHub)
    let url = server.api_url("/hub/mcp-servers/refresh");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    // Refresh against the placeholder GITHUB_HUB_REPO is blocked
    // pre-network with 400 HUB_NOT_CONFIGURED (closes 11-hub F-01).
    assert!(
        response.status() == 200 || response.status() == 400 || response.status() == 500,
        "Should return 200 / 400 / 500 for refresh attempt, got {}",
        response.status()
    );

    // User without permission should get 403
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", no_perm_user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        403,
        "User without permission should be forbidden"
    );
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
        "/hub/models/local-providers",
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
        &["hub::assistants::create", "hub::assistants::read"],
    )
    .await;

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
    assert!(
        !assistants.as_array().unwrap().is_empty(),
        "Should have at least one hub assistant"
    );

    // Get first assistant hub_id
    let first_assistant = &assistants.as_array().unwrap()[0];
    let hub_id = first_assistant.get("id").and_then(|v| v.as_str()).unwrap();

    // Verify created_ids is initially empty
    let created_ids = first_assistant
        .get("created_ids")
        .and_then(|v| v.as_array());
    assert!(
        created_ids.is_none() || created_ids.unwrap().is_empty(),
        "Created IDs should be empty initially"
    );

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

    assert_eq!(
        response.status(),
        201,
        "Should create assistant successfully"
    );
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");

    // Verify response structure
    assert!(
        body.get("assistant").is_some(),
        "Response should contain assistant"
    );
    assert!(
        body.get("hub_tracking").is_some(),
        "Response should contain hub_tracking"
    );

    let assistant_id = body
        .get("assistant")
        .and_then(|a| a.get("id"))
        .and_then(|v| v.as_str())
        .expect("Should have assistant ID");

    // Verify hub tracking
    let hub_tracking = body.get("hub_tracking").unwrap();
    assert_eq!(
        hub_tracking
            .get("entity_type")
            .and_then(|v| v.as_str())
            .unwrap(),
        "assistant"
    );
    assert_eq!(
        hub_tracking.get("hub_id").and_then(|v| v.as_str()).unwrap(),
        hub_id
    );
    assert_eq!(
        hub_tracking
            .get("hub_category")
            .and_then(|v| v.as_str())
            .unwrap(),
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

    let updated_assistant = assistants
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .expect("Should find the hub assistant");

    let created_ids = updated_assistant
        .get("created_ids")
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
        &["hub::mcp_servers::create", "hub::mcp_servers::read"],
    )
    .await;

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
    assert!(
        !servers.as_array().unwrap().is_empty(),
        "Should have at least one hub MCP server"
    );

    // Get first server hub_id
    let first_server = &servers.as_array().unwrap()[0];
    let hub_id = first_server.get("id").and_then(|v| v.as_str()).unwrap();

    // Verify created_ids is initially empty
    let created_ids = first_server.get("created_ids").and_then(|v| v.as_array());
    assert!(
        created_ids.is_none() || created_ids.unwrap().is_empty(),
        "Created IDs should be empty initially"
    );

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

    assert_eq!(
        response.status(),
        201,
        "Should create MCP server successfully"
    );
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");

    // Verify response structure
    assert!(
        body.get("server").is_some(),
        "Response should contain server"
    );
    assert!(
        body.get("hub_tracking").is_some(),
        "Response should contain hub_tracking"
    );

    let server_id = body
        .get("server")
        .and_then(|s| s.get("id"))
        .and_then(|v| v.as_str())
        .expect("Should have server ID");

    // Verify server is created as user server (not system server)
    let is_system = body
        .get("server")
        .and_then(|s| s.get("is_system"))
        .and_then(|v| v.as_bool())
        .expect("Should have is_system field");
    assert!(
        !is_system,
        "Hub-created servers should be user servers, not system servers"
    );

    // Verify hub tracking
    let hub_tracking = body.get("hub_tracking").unwrap();
    assert_eq!(
        hub_tracking
            .get("entity_type")
            .and_then(|v| v.as_str())
            .unwrap(),
        "mcp_server"
    );
    assert_eq!(
        hub_tracking.get("hub_id").and_then(|v| v.as_str()).unwrap(),
        hub_id
    );
    assert_eq!(
        hub_tracking
            .get("hub_category")
            .and_then(|v| v.as_str())
            .unwrap(),
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

    let updated_server = servers
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .expect("Should find the hub MCP server");

    let created_ids = updated_server
        .get("created_ids")
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
        &["hub::assistants::create", "hub::assistants::read"],
    )
    .await;

    let user2 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user2",
        &["hub::assistants::create", "hub::assistants::read"],
    )
    .await;

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
    let user1_assistant_id = body
        .get("assistant")
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
    let user2_assistant_id = body
        .get("assistant")
        .and_then(|a| a.get("id"))
        .and_then(|v| v.as_str())
        .unwrap();

    // Verify different assistant IDs
    assert_ne!(
        user1_assistant_id, user2_assistant_id,
        "Each user should get their own assistant instance"
    );

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
    let assistant = assistants
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .unwrap();

    let created_ids = assistant
        .get("created_ids")
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
    let assistant = assistants
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .unwrap();

    let created_ids = assistant
        .get("created_ids")
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
        &["hub::assistants::create", "hub::assistants::read"],
    )
    .await;

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

        assert_eq!(
            response.status(),
            201,
            "Should create assistant successfully"
        );
        let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
        let assistant_id = body
            .get("assistant")
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
    let assistant = assistants
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .unwrap();

    let created_ids = assistant
        .get("created_ids")
        .and_then(|v| v.as_array())
        .unwrap();

    assert_eq!(
        created_ids.len(),
        3,
        "Should track all three created assistants"
    );

    // Verify all IDs are present
    for assistant_id in assistant_ids {
        assert!(
            created_ids
                .iter()
                .any(|id| id.as_str() == Some(&assistant_id)),
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
        &[
            "hub::assistants::create",
            "hub::assistants::read",
            "assistants::delete",
        ],
    )
    .await;

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
    let assistant_id = body
        .get("assistant")
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
    let assistant = assistants
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .unwrap();

    let created_ids = assistant
        .get("created_ids")
        .and_then(|v| v.as_array())
        .unwrap();

    assert_eq!(
        created_ids.len(),
        1,
        "Should have hub tracking before deletion"
    );
    assert_eq!(created_ids[0].as_str().unwrap(), assistant_id);

    // Delete the assistant
    let url = server.api_url(&format!("/assistants/{}", assistant_id));
    let response = reqwest::Client::new()
        .delete(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        204,
        "Should delete assistant successfully"
    );

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
    let assistant = assistants
        .as_array()
        .unwrap()
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
        &[
            "hub::mcp_servers::create",
            "hub::mcp_servers::read",
            "mcp_servers::delete",
        ],
    )
    .await;

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
    let server_id = body
        .get("server")
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
    let mcp_server = servers
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .unwrap();

    let created_ids = mcp_server
        .get("created_ids")
        .and_then(|v| v.as_array())
        .unwrap();

    assert_eq!(
        created_ids.len(),
        1,
        "Should have hub tracking before deletion"
    );
    assert_eq!(created_ids[0].as_str().unwrap(), server_id);

    // Delete the MCP server
    let url = server.api_url(&format!("/mcp/servers/{}", server_id));
    let response = reqwest::Client::new()
        .delete(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        204,
        "Should delete MCP server successfully"
    );

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
    let mcp_server = servers
        .as_array()
        .unwrap()
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
        &[
            "hub::assistants::create",
            "hub::assistants::read",
            "assistants::delete",
        ],
    )
    .await;

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
        let assistant_id = body
            .get("assistant")
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
    let assistant = assistants
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .unwrap();

    let created_ids = assistant
        .get("created_ids")
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
    let assistant = assistants
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .unwrap();

    let created_ids = assistant
        .get("created_ids")
        .and_then(|v| v.as_array())
        .unwrap();

    assert_eq!(
        created_ids.len(),
        2,
        "Should have 2 assistants after deleting 1"
    );
    assert!(
        !created_ids
            .iter()
            .any(|id| id.as_str() == Some(&assistant_ids[0])),
        "Deleted assistant should not be in tracking"
    );
    assert!(
        created_ids
            .iter()
            .any(|id| id.as_str() == Some(&assistant_ids[1])),
        "Second assistant should still be tracked"
    );
    assert!(
        created_ids
            .iter()
            .any(|id| id.as_str() == Some(&assistant_ids[2])),
        "Third assistant should still be tracked"
    );

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
    let assistant = assistants
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .unwrap();

    let created_ids = assistant.get("created_ids").and_then(|v| v.as_array());

    assert!(
        created_ids.is_none() || created_ids.unwrap().is_empty(),
        "All hub tracking should be cleaned up after deleting all assistants"
    );
}

// =====================================================
// MODEL FROM HUB TESTS
// =====================================================

#[tokio::test]
async fn test_create_model_from_hub() {
    let server = crate::common::TestServer::start().await;

    // Create user with necessary permissions
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_model_user",
        &[
            "hub::models::download",
            "hub::models::read",
            "llm_models::create",
            "llm_models::read",
            "llm_providers::create",
            "llm_providers::read",
            "llm_repositories::read",
        ],
    )
    .await;

    // Get provider
    let provider = crate::llm_model::download_test::get_local_provider(&server, &user.token).await;
    let provider_id = provider.get("id").and_then(|v| v.as_str()).unwrap();

    // Get available hub models
    let url = server.api_url("/hub/models?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);
    let models: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(
        !models.as_array().unwrap().is_empty(),
        "Should have at least one hub model"
    );

    // Get first model hub_id
    let first_model = &models.as_array().unwrap()[0];
    let hub_id = first_model.get("id").and_then(|v| v.as_str()).unwrap();

    // Verify created_ids is initially empty
    let created_ids = first_model.get("created_ids").and_then(|v| v.as_array());
    assert!(
        created_ids.is_none() || created_ids.unwrap().is_empty(),
        "Created IDs should be empty initially"
    );

    // This model is auth_required; configure the source repo credential so the
    // pre-download gate passes.
    configure_hf_repo_credential(&server).await;

    // Create model download from hub
    let url = server.api_url("/hub/models/download");
    let request_body = serde_json::json!({
        "hub_id": hub_id,
        "provider_id": provider_id,
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

    let status = response.status();
    if status != 201 {
        let error_body = response.text().await.expect("Failed to read error body");
        panic!("Should create model download successfully. Status: {}, Body: {}", status, error_body);
    }
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");

    // Verify response structure
    assert!(
        body.get("download").is_some(),
        "Response should contain download instance"
    );
    assert!(
        body.get("hub_tracking").is_some(),
        "Response should contain hub_tracking"
    );

    let download_id = body
        .get("download")
        .and_then(|d| d.get("id"))
        .and_then(|v| v.as_str())
        .expect("Should have download ID");

    // Verify hub tracking
    let hub_tracking = body.get("hub_tracking").unwrap();
    assert_eq!(
        hub_tracking
            .get("entity_type")
            .and_then(|v| v.as_str())
            .unwrap(),
        "llm_model"
    );
    assert_eq!(
        hub_tracking.get("hub_id").and_then(|v| v.as_str()).unwrap(),
        hub_id
    );
    assert_eq!(
        hub_tracking
            .get("hub_category")
            .and_then(|v| v.as_str())
            .unwrap(),
        "model"
    );

    // Get hub models again and verify created_ids is populated
    let url = server.api_url("/hub/models?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);
    let models: serde_json::Value = response.json().await.expect("Failed to parse JSON");

    let updated_model = models
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .expect("Should find the hub model");

    let created_ids = updated_model
        .get("created_ids")
        .and_then(|v| v.as_array())
        .expect("Should have created_ids");

    assert_eq!(created_ids.len(), 1, "Should have exactly one created ID");
    assert_eq!(
        created_ids[0].as_str().unwrap(),
        download_id,
        "Created ID should match the download instance we just created"
    );
}

#[tokio::test]
async fn test_create_model_from_hub_requires_permission() {
    let server = crate::common::TestServer::start().await;

    // Create user WITHOUT hub::models::download permission
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "no_permission_user",
        &[
            "hub::models::read",
            "llm_models::create",
            "llm_providers::read",
        ],
    )
    .await;

    // Get provider
    let provider = crate::llm_model::download_test::get_local_provider(&server, &user.token).await;
    let provider_id = provider.get("id").and_then(|v| v.as_str()).unwrap();

    // Get hub models
    let url = server.api_url("/hub/models?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);
    let models: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let first_model = &models.as_array().unwrap()[0];
    let hub_id = first_model.get("id").and_then(|v| v.as_str()).unwrap();

    // Try to create model download without permission
    let url = server.api_url("/hub/models/download");
    let request_body = serde_json::json!({
        "hub_id": hub_id,
        "provider_id": provider_id,
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

    assert_eq!(
        response.status(),
        403,
        "Should return 403 Forbidden without hub::models::download permission"
    );

    let error_body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(
        error_body
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("hub::models::download"),
        "Error should mention missing permission"
    );
}

#[tokio::test]
async fn test_create_model_from_hub_invalid_hub_id() {
    let server = crate::common::TestServer::start().await;

    // Endpoint requires hub::models::download (the HubModelsCreate
    // permission resolves to that string per modules/hub/permissions.rs)
    // AND llm_models::create (11-hub F-05 closure — back-door defense).
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &[
            "hub::models::download",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
        ],
    )
    .await;

    // Get provider
    let provider = crate::llm_model::download_test::get_local_provider(&server, &user.token).await;
    let provider_id = provider.get("id").and_then(|v| v.as_str()).unwrap();

    // Try to create download with invalid hub_id
    let url = server.api_url("/hub/models/download");
    let request_body = serde_json::json!({
        "hub_id": "nonexistent-hub-model-id",
        "provider_id": provider_id,
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

    assert_eq!(
        response.status(),
        404,
        "Should return 404 for nonexistent hub model"
    );
}

#[tokio::test]
async fn test_create_model_from_hub_invalid_provider_id() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &[
            "hub::models::download",
            "hub::models::read",
            "llm_models::create",
            "llm_repositories::read",
        ],
    )
    .await;

    // Configure the source repo credential so the request passes the auth gate
    // and actually reaches provider validation — otherwise it would 422 at the
    // gate and this test would not exercise the invalid-provider path.
    configure_hf_repo_credential(&server).await;

    // Get hub models
    let url = server.api_url("/hub/models?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    let models: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let first_model = &models.as_array().unwrap()[0];
    let hub_id = first_model.get("id").and_then(|v| v.as_str()).unwrap();

    // Try to create download with invalid provider_id
    let url = server.api_url("/hub/models/download");
    let invalid_provider_id = uuid::Uuid::new_v4();
    let request_body = serde_json::json!({
        "hub_id": hub_id,
        "provider_id": invalid_provider_id,
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

    assert!(
        response.status().is_client_error() || response.status().is_server_error(),
        "Should return error for invalid provider_id, got {}",
        response.status()
    );
    // Must fail on the invalid provider, NOT short-circuit at the auth gate.
    assert_ne!(
        response.status(),
        422,
        "should reach provider validation, not be blocked by the auth gate"
    );
}

#[tokio::test]
async fn test_create_model_from_hub_with_quantization() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &[
            "hub::models::download",
            "hub::models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_repositories::read",
        ],
    )
    .await;

    // Get provider
    let provider = crate::llm_model::download_test::get_local_provider(&server, &user.token).await;
    let provider_id = provider.get("id").and_then(|v| v.as_str()).unwrap();

    // Get hub models
    let url = server.api_url("/hub/models?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    let models: serde_json::Value = response.json().await.expect("Failed to parse JSON");

    // Find a model with quantization options
    let model_with_quant = models
        .as_array()
        .unwrap()
        .iter()
        .find(|m| {
            m.get("quantization_options")
                .and_then(|q| q.as_array())
                .map(|arr| !arr.is_empty())
                .unwrap_or(false)
        });

    if model_with_quant.is_none() {
        println!("No models with quantization options found, skipping test");
        return;
    }

    let model = model_with_quant.unwrap();
    let hub_id = model.get("id").and_then(|v| v.as_str()).unwrap();
    let quant_options = model.get("quantization_options").and_then(|v| v.as_array()).unwrap();
    let first_quant = &quant_options[0];
    let quant_name = first_quant.get("name").and_then(|v| v.as_str()).unwrap();

    // auth_required model: configure the source repo credential so the gate passes.
    configure_hf_repo_credential(&server).await;

    // Create download with specific quantization
    let url = server.api_url("/hub/models/download");
    let request_body = serde_json::json!({
        "hub_id": hub_id,
        "provider_id": provider_id,
        "enabled": true,
        "quantization_name": quant_name
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        201,
        "Should create download with quantization selection"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let download = body.get("download").expect("Should have download");

    // Verify the download was created (we can't easily verify the quantization was applied
    // without checking the download instance's request_data, which is internal)
    assert!(download.get("id").is_some(), "Download should have an ID");
}

#[tokio::test]
async fn test_duplicate_download_prevention() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_user",
        &[
            "hub::models::download",
            "hub::models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_repositories::read",
        ],
    )
    .await;

    // Get provider
    let provider = crate::llm_model::download_test::get_local_provider(&server, &user.token).await;
    let provider_id = provider.get("id").and_then(|v| v.as_str()).unwrap();

    // Get hub models
    let url = server.api_url("/hub/models?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    let models: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let first_model = &models.as_array().unwrap()[0];
    let hub_id = first_model.get("id").and_then(|v| v.as_str()).unwrap();

    // auth_required model: configure the source repo credential so the gate passes.
    configure_hf_repo_credential(&server).await;

    // Create first download
    let url = server.api_url("/hub/models/download");
    let request_body = serde_json::json!({
        "hub_id": hub_id,
        "provider_id": provider_id,
        "enabled": true
    });

    let response1 = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response1.status(), 201);
    let body1: serde_json::Value = response1.json().await.expect("Failed to parse JSON");
    let download_id1 = body1
        .get("download")
        .and_then(|d| d.get("id"))
        .and_then(|v| v.as_str())
        .unwrap();

    // Try to create second download from same hub model (should return existing)
    let response2 = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response2.status(), 201);
    let body2: serde_json::Value = response2.json().await.expect("Failed to parse JSON");
    let download_id2 = body2
        .get("download")
        .and_then(|d| d.get("id"))
        .and_then(|v| v.as_str())
        .unwrap();

    // Verify they have the SAME ID (deduplication working)
    assert_eq!(
        download_id1, download_id2,
        "Duplicate download should return the same download instance"
    );

    // Get hub models again and verify created_ids contains only one entry
    let url = server.api_url("/hub/models?lang=en");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    let models: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let updated_model = models
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m.get("id").and_then(|v| v.as_str()) == Some(hub_id))
        .expect("Should find the hub model");

    let created_ids = updated_model
        .get("created_ids")
        .and_then(|v| v.as_array())
        .expect("Should have created_ids");

    assert_eq!(
        created_ids.len(),
        1,
        "Should have only 1 download ID (deduplicated)"
    );

    assert_eq!(
        created_ids[0].as_str().unwrap(),
        download_id1,
        "Should contain the original download ID"
    );
}

// ============================================================================
// Hub Local Providers Tests (GET /hub/models/local-providers)
// ============================================================================

/// Create an enabled local LLM provider via the API and return its JSON.
/// The migration-seeded built-in `Local` provider is `enabled = false`, so
/// `list_local_providers` (which filters `enabled = true`) ignores it — tests
/// must create their own enabled provider to get a non-empty result.
async fn create_enabled_provider(
    server: &crate::common::TestServer,
    token: &str,
    name: &str,
    provider_type: &str,
) -> serde_json::Value {
    let response = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "name": name,
            "provider_type": provider_type,
            "enabled": true,
        }))
        .send()
        .await
        .expect("Request failed");

    let status = response.status();
    if status != 201 {
        let body = response.text().await.unwrap_or_default();
        panic!("Failed to create provider {name}. Status: {status}, Body: {body}");
    }
    response.json().await.expect("Failed to parse provider JSON")
}

/// Remove all group memberships for a user, leaving them with zero effective
/// permissions. Registration auto-assigns the default `Users` group; stripping
/// memberships guarantees the 403 path regardless of what that group grants.
async fn strip_all_permissions(server: &crate::common::TestServer, user_id: &str) {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");
    let uuid = uuid::Uuid::parse_str(user_id).expect("Invalid user ID");
    sqlx::query("DELETE FROM user_groups WHERE user_id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .expect("Failed to strip user group memberships");
}

#[tokio::test]
async fn test_get_hub_local_providers_requires_permission() {
    let server = crate::common::TestServer::start().await;

    // The endpoint is gated on HubModelsCreate, whose permission string is
    // `hub::models::download`. Migration 37 removed it from the default Users
    // group, so a user must be granted it explicitly to gain access.
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_localprov_user",
        &["hub::models::download"],
    )
    .await;
    let no_perm_user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_localprov_noperm",
        &[],
    )
    .await;
    // Strip the default group so this user genuinely lacks the permission.
    strip_all_permissions(&server, &no_perm_user.user_id).await;

    let url = server.api_url("/hub/models/local-providers");

    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        response.status(),
        200,
        "User with the default hub::models::download permission should list local providers"
    );

    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", no_perm_user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        response.status(),
        403,
        "User stripped of all permissions should be forbidden"
    );
}

#[tokio::test]
async fn test_get_hub_local_providers_response_structure() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_localprov_struct",
        &["hub::models::download", "llm_providers::create"],
    )
    .await;

    let created = create_enabled_provider(&server, &user.token, "E2E Local Alpha", "local").await;
    let created_id = created
        .get("id")
        .and_then(|v| v.as_str())
        .expect("created provider should have id");

    let url = server.api_url("/hub/models/local-providers");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let providers = body
        .get("providers")
        .and_then(|v| v.as_array())
        .expect("Response should have a `providers` array");

    let entry = providers
        .iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(created_id))
        .expect("Created enabled local provider should appear in the list");
    assert_eq!(
        entry.get("name").and_then(|v| v.as_str()),
        Some("E2E Local Alpha"),
        "Provider entry should carry its name"
    );
    // Response items expose only id + name.
    assert!(
        entry.get("id").and_then(|v| v.as_str()).is_some(),
        "Provider entry should have an id"
    );
}

#[tokio::test]
async fn test_get_hub_local_providers_excludes_non_local_and_disabled() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_localprov_excl",
        &["hub::models::download", "llm_providers::create"],
    )
    .await;

    // Enabled local — must appear.
    let local =
        create_enabled_provider(&server, &user.token, "E2E Local Included", "local").await;
    let local_id = local.get("id").and_then(|v| v.as_str()).unwrap();

    // Enabled non-local (custom is exempt from the api_key requirement) — must NOT appear.
    create_enabled_provider(&server, &user.token, "E2E Custom Excluded", "custom").await;

    let url = server.api_url("/hub/models/local-providers");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let providers = body
        .get("providers")
        .and_then(|v| v.as_array())
        .expect("Response should have a `providers` array");

    let names: Vec<&str> = providers
        .iter()
        .filter_map(|p| p.get("name").and_then(|v| v.as_str()))
        .collect();

    assert!(
        providers
            .iter()
            .any(|p| p.get("id").and_then(|v| v.as_str()) == Some(local_id)),
        "Enabled local provider should be included"
    );
    assert!(
        !names.contains(&"E2E Custom Excluded"),
        "Non-local (custom) provider should be excluded, got: {names:?}"
    );
    assert!(
        !names.contains(&"Local"),
        "Disabled built-in 'Local' provider should be excluded, got: {names:?}"
    );
}

/// `source_auth_configured` is computed per source repository: false while the
/// Hugging Face repo has no credential, true once a key is set. This is the data
/// the hub UI uses to block + guide download BEFORE the click.
#[tokio::test]
async fn test_hub_models_source_auth_configured_reflects_repo_credential() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_source_auth",
        &[
            "hub::models::read",
            "llm_repositories::read",
            "llm_repositories::edit",
        ],
    )
    .await;

    // Initially: the Hugging Face repo has no key -> HF models report unconfigured.
    let before: serde_json::Value = reqwest::Client::new()
        .get(server.api_url("/hub/models?lang=en"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let mut hf_count = 0;
    for m in before.as_array().unwrap() {
        if m["repository_url"].as_str() == Some("https://huggingface.co") {
            hf_count += 1;
            assert_eq!(
                m["source_auth_configured"].as_bool(),
                Some(false),
                "model {} should be unconfigured while the HF repo is empty",
                m["id"]
            );
        }
    }
    assert!(hf_count > 0, "expected at least one Hugging Face model in the catalog");

    // Configure the Hugging Face repo with a dummy key.
    let repos: serde_json::Value = reqwest::Client::new()
        .get(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let hf_id = repos["repositories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"].as_str() == Some("Hugging Face Hub"))
        .expect("Hugging Face Hub repository should exist")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let update = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-repositories/{}", hf_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({ "auth_config": { "api_key": "dummy-token" } }))
        .send()
        .await
        .unwrap();
    assert_eq!(update.status(), 200);

    // Now HF models report configured.
    let after: serde_json::Value = reqwest::Client::new()
        .get(server.api_url("/hub/models?lang=en"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    for m in after.as_array().unwrap() {
        if m["repository_url"].as_str() == Some("https://huggingface.co") {
            assert_eq!(
                m["source_auth_configured"].as_bool(),
                Some(true),
                "model {} should be configured after setting a key",
                m["id"]
            );
        }
    }
}

/// A PARTIAL repository update (changing only a non-secret field, omitting the
/// secret) must NOT wipe the stored credential — the API/UI treat an omitted
/// secret as "keep existing". Regression guard for the merge-on-update fix.
#[tokio::test]
async fn test_partial_repo_update_preserves_stored_secret() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "repo_partial",
        &[
            "hub::models::read",
            "llm_repositories::read",
            "llm_repositories::edit",
        ],
    )
    .await;

    let repos: serde_json::Value = reqwest::Client::new()
        .get(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let hf_id = repos["repositories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"].as_str() == Some("Hugging Face Hub"))
        .expect("Hugging Face Hub repository")["id"]
        .as_str()
        .unwrap()
        .to_string();

    // 1) Set a credential.
    let set = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-repositories/{}", hf_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({ "auth_config": { "api_key": "dummy-key" } }))
        .send()
        .await
        .unwrap();
    assert_eq!(set.status(), 200);

    // 2) Partial update that OMITS the secret (changes only the test endpoint).
    let partial = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-repositories/{}", hf_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({
            "auth_config": { "auth_test_api_endpoint": "https://huggingface.co/api/whoami-v2" }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(partial.status(), 200, "partial update should succeed");

    // 3) The stored secret must survive — source_auth_configured stays true.
    let models: serde_json::Value = reqwest::Client::new()
        .get(server.api_url("/hub/models?lang=en"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let hf_model = models
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["repository_url"].as_str() == Some("https://huggingface.co"))
        .expect("an HF hub model");
    assert_eq!(
        hf_model["source_auth_configured"].as_bool(),
        Some(true),
        "a partial update must NOT wipe the stored credential"
    );
}

/// Configure the built-in Hugging Face repository with a dummy credential so the
/// hub pre-download gate (auth_required models require a configured source repo)
/// passes. Uses its own admin user so callers don't need repository-edit perms.
async fn configure_hf_repo_credential(server: &crate::common::TestServer) {
    let admin = crate::common::test_helpers::create_user_with_permissions(
        server,
        "hf_repo_admin",
        &["llm_repositories::read", "llm_repositories::edit"],
    )
    .await;
    let repos: serde_json::Value = reqwest::Client::new()
        .get(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("list repositories")
        .json()
        .await
        .expect("parse repositories");
    let hf_id = repos["repositories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"].as_str() == Some("Hugging Face Hub"))
        .expect("Hugging Face Hub repository should exist")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-repositories/{}", hf_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "auth_config": { "api_key": "dummy-token" } }))
        .send()
        .await
        .expect("configure HF repo");
    assert_eq!(
        resp.status(),
        200,
        "configuring the Hugging Face repo credential should succeed"
    );
}
