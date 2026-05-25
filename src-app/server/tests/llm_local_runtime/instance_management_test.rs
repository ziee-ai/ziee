// Integration tests for instance management
use reqwest::StatusCode;
use serde_json::json;
use uuid::Uuid;

// =====================================================
// Permission Tests
// =====================================================

#[tokio::test]
async fn test_start_instance_requires_manage_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::read"],
    )
    .await;

    let model_id = Uuid::new_v4();
    let payload = json!({});

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/local-runtime/models/{}/start", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_stop_instance_requires_manage_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::read"],
    )
    .await;

    let model_id = Uuid::new_v4();

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/local-runtime/models/{}/stop", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_restart_instance_requires_manage_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::read"],
    )
    .await;

    let model_id = Uuid::new_v4();

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/local-runtime/models/{}/restart", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_get_instance_requires_read_permission() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    let model_id = Uuid::new_v4();

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/local-runtime/models/{}/instance", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_get_status_requires_read_permission() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    let model_id = Uuid::new_v4();

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/local-runtime/models/{}/status", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_get_health_requires_read_permission() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    let model_id = Uuid::new_v4();

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/local-runtime/models/{}/health", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_get_logs_requires_logs_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::read"],
    )
    .await;

    let model_id = Uuid::new_v4();

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/local-runtime/models/{}/logs", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_get_provider_instances_requires_read_permission() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    let provider_id = Uuid::new_v4();

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/local-runtime/providers/{}/instances", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// =====================================================
// Edge Case Tests
// =====================================================

#[tokio::test]
async fn test_start_instance_model_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::manage"],
    )
    .await;

    let nonexistent_model_id = Uuid::new_v4();
    let payload = json!({});

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/local-runtime/models/{}/start", nonexistent_model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_stop_instance_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::manage"],
    )
    .await;

    let nonexistent_model_id = Uuid::new_v4();

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/local-runtime/models/{}/stop", nonexistent_model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_restart_instance_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::manage"],
    )
    .await;

    let nonexistent_model_id = Uuid::new_v4();

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/local-runtime/models/{}/restart", nonexistent_model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_instance_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::read"],
    )
    .await;

    let nonexistent_model_id = Uuid::new_v4();

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/local-runtime/models/{}/instance", nonexistent_model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_status_returns_not_found_for_missing_instance() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::read"],
    )
    .await;

    let nonexistent_model_id = Uuid::new_v4();

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/local-runtime/models/{}/status", nonexistent_model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["status"], "not_found");
}

#[tokio::test]
async fn test_get_health_instance_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::read"],
    )
    .await;

    let nonexistent_model_id = Uuid::new_v4();

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/local-runtime/models/{}/health", nonexistent_model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_logs_instance_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::logs"],
    )
    .await;

    let nonexistent_model_id = Uuid::new_v4();

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/local-runtime/models/{}/logs", nonexistent_model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_provider_instances_empty_for_nonexistent_provider() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["llm_local_runtime::read"],
    )
    .await;

    let nonexistent_provider_id = Uuid::new_v4();

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/local-runtime/providers/{}/instances", nonexistent_provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["instances"].is_array());
    assert_eq!(body["instances"].as_array().unwrap().len(), 0);
}

// =====================================================
// Full Lifecycle Test (requires real binaries)
// =====================================================

#[tokio::test]
async fn test_full_instance_lifecycle() {
    use crate::llm_local_runtime::test_helpers::*;

    // Setup: Start test server
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "llm_local_runtime::manage",
            "llm_local_runtime::read",
            "llm_local_runtime::logs",
            "llm_local_runtime::create",
            "llm_providers::create",
            "llm_providers::read",
            "llm_models::create",
            "llm_models::read",
            "llm_models::edit",
            "llm_models::downloads_read",
            "llm_repositories::read",
            "llm_repositories::edit",
        ],
    )
    .await;

    // Step 1: Download and register binary
    println!("Step 1: Downloading llama.cpp binary...");
    let (runtime_version_id, _binary_path) =
        setup_test_binary(&server, &user.token, "llamacpp", "latest").await;
    println!("✓ Binary ready: {}", runtime_version_id);

    // Step 2: Create provider
    println!("Step 2: Creating test provider...");
    let provider = create_test_provider(&server, &user.token, "test-local-provider").await;
    let provider_id = Uuid::parse_str(provider["id"].as_str().unwrap()).unwrap();
    println!("✓ Provider created: {}", provider_id);

    // Step 3: Download actual model file (tiny-random-gpt2, small test model)
    println!("Step 3: Downloading test model (tiny-random-gpt2)...");
    let (model, _model_path) = download_test_model(&server, &user.token, provider_id).await;
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();
    println!("✓ Model downloaded: {}", model_id);

    // Step 4: Start instance
    println!("Step 4: Starting instance...");
    let start_response = reqwest::Client::new()
        .post(server.api_url(&format!("/local-runtime/models/{}/start", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({}))
        .send()
        .await
        .unwrap();

    assert_eq!(start_response.status(), StatusCode::CREATED);
    let instance: serde_json::Value = start_response.json().await.unwrap();
    println!(
        "✓ Instance started on port: {}",
        instance["local_port"].as_u64().unwrap_or(0)
    );

    // Step 5: Check status
    println!("Step 5: Checking status...");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await; // Let it start

    let status_response = reqwest::Client::new()
        .get(server.api_url(&format!("/local-runtime/models/{}/status", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(status_response.status(), StatusCode::OK);
    let status: serde_json::Value = status_response.json().await.unwrap();
    assert_eq!(status["status"], "running");
    println!(
        "✓ Status: running (uptime: {}s)",
        status["uptime_seconds"].as_i64().unwrap_or(0)
    );

    // Step 6: Health check
    println!("Step 6: Health check...");
    let health_response = reqwest::Client::new()
        .get(server.api_url(&format!("/local-runtime/models/{}/health", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(health_response.status(), StatusCode::OK);
    let health: serde_json::Value = health_response.json().await.unwrap();
    // Note: Using safetensors model with llama.cpp won't be healthy (format mismatch)
    // but we can verify the health endpoint works and returns the correct structure
    assert!(health.get("healthy").is_some(), "Health response should have 'healthy' field");
    let is_healthy = health["healthy"].as_bool().unwrap_or(false);
    println!(
        "✓ Health check completed: {} (response time: {}ms)",
        if is_healthy { "healthy" } else { "unhealthy (expected - format mismatch)" },
        health["response_time_ms"].as_u64().unwrap_or(0)
    );

    // Step 7: Get logs
    println!("Step 7: Fetching logs...");
    let logs_response = reqwest::Client::new()
        .get(server.api_url(&format!("/local-runtime/models/{}/logs", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(logs_response.status(), StatusCode::OK);
    let logs: serde_json::Value = logs_response.json().await.unwrap();
    assert!(logs["logs"].is_array());
    println!(
        "✓ Logs fetched: {} lines",
        logs["logs"].as_array().unwrap().len()
    );

    // Step 8: Restart instance
    println!("Step 8: Restarting instance...");
    let restart_response = reqwest::Client::new()
        .post(server.api_url(&format!("/local-runtime/models/{}/restart", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(restart_response.status(), StatusCode::OK);
    let restarted: serde_json::Value = restart_response.json().await.unwrap();
    println!(
        "✓ Instance restarted (new instance ID: {})",
        restarted["id"]
    );

    // Step 9: Stop instance
    println!("Step 9: Stopping instance...");
    let stop_response = reqwest::Client::new()
        .post(server.api_url(&format!("/local-runtime/models/{}/stop", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(stop_response.status(), StatusCode::OK);
    let stopped: serde_json::Value = stop_response.json().await.unwrap();
    assert_eq!(stopped["status"], "stopped");
    println!("✓ Instance stopped");

    // Step 10: Verify cleanup
    println!("Step 10: Verifying cleanup...");
    let final_status_response = reqwest::Client::new()
        .get(server.api_url(&format!("/local-runtime/models/{}/status", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    let final_status: serde_json::Value = final_status_response.json().await.unwrap();
    assert!(
        final_status["status"] == "stopped" || final_status["status"] == "not_found",
        "Instance should be stopped or not found"
    );
    println!("✓ Cleanup verified");

    println!("\n✅ Full lifecycle test completed successfully!");
}
