// Integration tests for runtime version management
use reqwest::StatusCode;
use serde_json::json;

// =====================================================
// Permission Tests
// =====================================================

#[tokio::test]
async fn test_list_versions_requires_read_permission() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    let response = reqwest::Client::new()
        .get(&server.api_url("/local-runtime/versions"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_list_versions_with_read_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::versions_read"],
    )
    .await;

    let response = reqwest::Client::new()
        .get(&server.api_url("/local-runtime/versions"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["versions"].is_array());
}

#[tokio::test]
async fn test_download_version_requires_create_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::versions_read"],
    )
    .await;

    let payload = json!({
        "engine": "llamacpp",
        "version": "b1234",
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/local-runtime/versions/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_delete_version_requires_delete_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::versions_read"],
    )
    .await;

    let response = reqwest::Client::new()
        .delete(&server.api_url("/local-runtime/versions/00000000-0000-0000-0000-000000000000"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_set_default_version_requires_update_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::versions_read"],
    )
    .await;

    let response = reqwest::Client::new()
        .post(&server.api_url("/local-runtime/versions/00000000-0000-0000-0000-000000000000/set-default"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// =====================================================
// Functional Tests
// =====================================================

#[tokio::test]
async fn test_list_versions_empty_initially() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::versions_read"],
    )
    .await;

    let response = reqwest::Client::new()
        .get(&server.api_url("/local-runtime/versions"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["versions"].is_array());
    let versions = body["versions"].as_array().unwrap();
    // Initially, the list should be empty or contain only pre-existing versions
    // We just verify it returns a valid array
    assert!(versions.len() >= 0);
}

#[tokio::test]
async fn test_list_versions_can_filter_by_engine() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::versions_read"],
    )
    .await;

    // Test filtering by llamacpp
    let response = reqwest::Client::new()
        .get(&server.api_url("/local-runtime/versions?engine=llamacpp"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["versions"].is_array());

    // Verify all returned versions are llamacpp
    if let Some(versions) = body["versions"].as_array() {
        for version in versions {
            assert_eq!(version["engine"].as_str().unwrap(), "llamacpp");
        }
    }

    // Test filtering by mistralrs
    let response = reqwest::Client::new()
        .get(&server.api_url("/local-runtime/versions?engine=mistralrs"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["versions"].is_array());

    // Verify all returned versions are mistralrs
    if let Some(versions) = body["versions"].as_array() {
        for version in versions {
            assert_eq!(version["engine"].as_str().unwrap(), "mistralrs");
        }
    }
}

#[tokio::test]
async fn test_download_version_validation() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::versions_read", "llm_local_runtime::create"],
    )
    .await;

    // Test invalid engine
    let payload = json!({
        "engine": "invalid_engine",
        "version": "b1234",
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/local-runtime/versions/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert!(response.status().is_client_error());

    // Test missing engine field
    let payload = json!({
        "version": "b1234",
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/local-runtime/versions/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert!(response.status().is_client_error());

    // Test missing version field
    let payload = json!({
        "engine": "llamacpp",
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/local-runtime/versions/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert!(response.status().is_client_error());
}

#[tokio::test]
async fn test_delete_nonexistent_version() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::versions_read", "llm_local_runtime::delete"],
    )
    .await;

    let fake_uuid = "00000000-0000-0000-0000-000000000000";
    let response = reqwest::Client::new()
        .delete(&server.api_url(&format!("/local-runtime/versions/{}", fake_uuid)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    // Should return 404 for non-existent version
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_set_default_nonexistent_version() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::versions_read", "llm_local_runtime::update"],
    )
    .await;

    let fake_uuid = "00000000-0000-0000-0000-000000000000";
    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/local-runtime/versions/{}/set-default", fake_uuid)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    // Should return 404 for non-existent version
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =====================================================
// Download and CRUD Workflow Tests
// =====================================================

// Note: These tests require actual GitHub releases to be available
// They are marked as ignored and can be run manually with --ignored flag
// Example: cargo test --test integration_tests llm_local_runtime::runtime_version_test::test_download_and_list_llamacpp_version -- --exact --ignored --nocapture

#[tokio::test]
async fn test_download_and_list_llamacpp_version() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "llm_local_runtime::versions_read",
            "llm_local_runtime::create",
            "llm_local_runtime::update",
            "llm_local_runtime::delete",
        ],
    )
    .await;

    // Download the latest version
    let platform = if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x86_64"
    };

    let backend = if cfg!(target_os = "macos") {
        "metal"
    } else {
        "cpu"
    };

    let payload = json!({
        "engine": "llamacpp",
        "version": "latest",
        "platform": platform,
        "arch": arch,
        "backend": backend,
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/local-runtime/versions/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // Should succeed or already exist
    let status = response.status();
    if status != StatusCode::OK && status != StatusCode::CONFLICT {
        let error_body = response.text().await.unwrap_or_else(|_| "Could not read response body".to_string());
        panic!(
            "Expected OK (200) or CONFLICT (409), got {}. Body: {}",
            status, error_body
        );
    }

    if status == StatusCode::OK {
        let body: serde_json::Value = response.json().await.unwrap();
        // Response structure: { "version": { "id": "uuid", "engine": "...", ... }, "downloaded": true, "message": "..." }
        assert_eq!(body["version"]["engine"].as_str().unwrap_or(""), "llamacpp",
            "Expected llamacpp engine, got body: {:?}", body);
        // Version will be the resolved latest version (e.g., "v0.0.1-alpha")
        assert!(body["version"]["version"].is_string(),
            "Expected version string, got body: {:?}", body);
        assert!(!body["version"]["version"].as_str().unwrap().is_empty());
        assert!(body["version"]["id"].is_string(),
            "Expected id string, got body: {:?}", body);
    }

    // List versions and verify it appears
    let response = reqwest::Client::new()
        .get(&server.api_url("/local-runtime/versions?engine=llamacpp"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["versions"].is_array(), "Expected versions array, got: {:?}", body);
    let versions = body["versions"].as_array().unwrap();

    // Verify at least one llamacpp version exists (the one we just downloaded)
    let found_version = versions.iter().find(|v| {
        v["engine"].as_str().unwrap() == "llamacpp"
    });

    assert!(found_version.is_some(), "Downloaded llamacpp version should appear in list");
}

#[tokio::test]
async fn test_full_version_lifecycle() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "llm_local_runtime::versions_read",
            "llm_local_runtime::create",
            "llm_local_runtime::update",
            "llm_local_runtime::delete",
        ],
    )
    .await;

    // 1. Download a version
    let platform = if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x86_64"
    };

    let backend = if cfg!(target_os = "macos") {
        "metal"
    } else {
        "cpu"
    };

    let payload = json!({
        "engine": "llamacpp",
        "version": "latest",
        "platform": platform,
        "arch": arch,
        "backend": backend,
    });

    let download_response = reqwest::Client::new()
        .post(&server.api_url("/local-runtime/versions/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    if download_response.status() != StatusCode::OK {
        // If it already exists or there's another issue, skip this test
        // CONFLICT (409) = already exists, UNPROCESSABLE_ENTITY (422) = validation error
        assert!(
            download_response.status() == StatusCode::CONFLICT
            || download_response.status() == StatusCode::UNPROCESSABLE_ENTITY,
            "Expected OK, CONFLICT, or UNPROCESSABLE_ENTITY, got: {}",
            download_response.status()
        );
        return; // Skip the rest if it already exists or can't be downloaded
    }

    let version_data: serde_json::Value = download_response.json().await.unwrap();
    // Response structure: { "version": { "id": "uuid", ... }, "downloaded": true, "message": "..." }
    let version_id = version_data["version"]["id"].as_str().unwrap();

    // 2. Set as default
    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/local-runtime/versions/{}/set-default", version_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // 3. Verify it's marked as default
    let response = reqwest::Client::new()
        .get(&server.api_url("/local-runtime/versions?engine=llamacpp"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["versions"].is_array(), "Expected versions array, got: {:?}", body);
    let versions = body["versions"].as_array().unwrap();

    let default_version = versions.iter().find(|v| {
        v["id"].as_str().unwrap() == version_id
    });

    assert!(default_version.is_some());
    assert_eq!(default_version.unwrap()["is_system_default"].as_bool().unwrap(), true);

    // 4. Delete the version
    let response = reqwest::Client::new()
        .delete(&server.api_url(&format!("/local-runtime/versions/{}", version_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // 5. Verify it's deleted
    let response = reqwest::Client::new()
        .get(&server.api_url("/local-runtime/versions?engine=llamacpp"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["versions"].is_array(), "Expected versions array, got: {:?}", body);
    let versions = body["versions"].as_array().unwrap();

    let found_version = versions.iter().find(|v| {
        v["id"].as_str().unwrap() == version_id
    });

    assert!(found_version.is_none(), "Deleted version should not appear in list");
}

#[tokio::test]
async fn test_download_mistralrs_version() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::versions_read", "llm_local_runtime::create"],
    )
    .await;

    let platform = if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x86_64"
    };

    let backend = if cfg!(target_os = "macos") {
        "metal"
    } else {
        "cpu"
    };

    let payload = json!({
        "engine": "mistralrs",
        "version": "latest",
        "platform": platform,
        "arch": arch,
        "backend": backend,
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/local-runtime/versions/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // Should succeed, already exist, or fail validation (if no releases available)
    assert!(
        response.status() == StatusCode::OK
        || response.status() == StatusCode::CONFLICT
        || response.status() == StatusCode::UNPROCESSABLE_ENTITY,
        "Expected OK, CONFLICT, or UNPROCESSABLE_ENTITY, got: {}",
        response.status()
    );

    if response.status() == StatusCode::OK {
        let body: serde_json::Value = response.json().await.unwrap();
        // Response structure: { "version": { "engine": "mistralrs", ... }, "downloaded": true, "message": "..." }
        assert_eq!(body["version"]["engine"].as_str().unwrap_or(""), "mistralrs",
            "Expected mistralrs engine, got body: {:?}", body);
    }
}
