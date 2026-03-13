/// Test Helpers for LLM Local Runtime Integration Tests
///
/// Provides reusable helper functions for setting up binaries, providers, and models
/// for integration tests that require real infrastructure.

use crate::common::TestServer;
use reqwest::StatusCode;
use serde_json::json;
use std::path::PathBuf;
use uuid::Uuid;

/// Downloads and registers a test binary (llama.cpp or mistral.rs)
///
/// This helper:
/// 1. Downloads the binary using llm_runtime::binary_download::ensure_test_binary
/// 2. Registers it in the database via the API
/// 3. Returns the runtime version ID and binary path
///
/// # Arguments
/// * `server` - Test server instance
/// * `token` - Authentication token
/// * `engine` - Engine type ("llamacpp" or "mistralrs")
/// * `version` - Version to download (e.g., "b4359" or "latest")
///
/// # Returns
/// (runtime_version_id, binary_path)
pub async fn setup_test_binary(
    server: &TestServer,
    token: &str,
    engine: &str,
    version: &str,
) -> (Uuid, PathBuf) {
    // 1. Download binary using llm-runtime helper
    let engine_type = match engine {
        "llamacpp" => llm_runtime::config::EngineType::Llamacpp,
        "mistralrs" => llm_runtime::config::EngineType::Mistralrs,
        _ => panic!("Unknown engine: {}", engine),
    };

    println!("Downloading {} binary version {}...", engine, version);
    let binary_path = llm_runtime::binary_download::ensure_test_binary(engine_type, version)
        .await
        .expect("Failed to download test binary");
    println!("Binary downloaded to: {}", binary_path.display());

    // 2. Register in database via API
    let platform = if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        panic!("Unsupported platform")
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        panic!("Unsupported architecture")
    };

    let backend = if cfg!(target_os = "macos") {
        "metal"
    } else {
        "cpu"
    };

    let payload = json!({
        "engine": engine,
        "version": version,
        "platform": platform,
        "arch": arch,
        "backend": backend,
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/local-runtime/versions/download"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to register binary");

    // Handle both success and conflict (already exists)
    let status = response.status();
    if status != StatusCode::OK && status != StatusCode::CONFLICT {
        let error_body = response.text().await.unwrap();
        panic!(
            "Failed to register binary. Status: {}, Body: {}",
            status, error_body
        );
    }

    let body: serde_json::Value = response.json().await.unwrap();

    // Extract version ID - handle both new registrations and conflicts
    let version_id = if status == StatusCode::OK {
        // Response structure: { "version": { "id": "uuid", ... }, "downloaded": true, "message": "..." }
        Uuid::parse_str(body["version"]["id"].as_str().unwrap()).unwrap()
    } else {
        // CONFLICT - need to fetch the existing version
        let list_response = reqwest::Client::new()
            .get(&server.api_url(&format!("/local-runtime/versions?engine={}", engine)))
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .unwrap();

        let versions: serde_json::Value = list_response.json().await.unwrap();
        let found = versions
            .as_array()
            .unwrap()
            .iter()
            .find(|v| {
                v["engine"].as_str().unwrap() == engine
                    && v["version"].as_str().unwrap() == version
                    && v["platform"].as_str().unwrap() == platform
                    && v["arch"].as_str().unwrap() == arch
                    && v["backend"].as_str().unwrap() == backend
            })
            .expect("Should find the conflicting version");

        Uuid::parse_str(found["id"].as_str().unwrap()).unwrap()
    };

    println!("Binary registered with ID: {}", version_id);
    (version_id, binary_path)
}

/// Creates a test provider
///
/// # Arguments
/// * `server` - Test server instance
/// * `token` - Authentication token
/// * `name` - Provider name
///
/// # Returns
/// Provider JSON object
pub async fn create_test_provider(
    server: &TestServer,
    token: &str,
    name: &str,
) -> serde_json::Value {
    let payload = json!({
        "name": name,
        "provider_type": "local",
        "enabled": true,
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to create provider");

    if response.status() != StatusCode::CREATED {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_else(|_| "Could not read response body".to_string());
        panic!(
            "Failed to create provider. Status: {}, Body: {}",
            status, error_body
        );
    }

    response.json().await.unwrap()
}

/// Gets or creates a local provider
///
/// # Arguments
/// * `server` - Test server instance
/// * `token` - Authentication token
///
/// # Returns
/// Provider JSON object
pub async fn get_or_create_local_provider(
    server: &TestServer,
    token: &str,
) -> serde_json::Value {
    let response = reqwest::Client::new()
        .get(&server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = response.json().await.unwrap();

    // Look for a local provider
    if let Some(providers) = body["providers"].as_array() {
        for provider in providers {
            if provider["provider_type"].as_str() == Some("local") {
                return provider.clone();
            }
        }
    }

    // Create a local provider if none exists
    create_test_provider(server, token, "Local Models").await
}

/// Creates a test model (no actual file - for testing instance management)
///
/// # Arguments
/// * `server` - Test server instance
/// * `token` - Authentication token
/// * `provider_id` - Provider UUID
/// * `runtime_version_id` - Optional runtime version requirement
/// * `name` - Model name
///
/// # Returns
/// Model JSON object
pub async fn create_test_model(
    server: &TestServer,
    token: &str,
    provider_id: Uuid,
    runtime_version_id: Option<Uuid>,
    name: &str,
) -> serde_json::Value {
    let mut payload = json!({
        "provider_id": provider_id.to_string(),
        "name": name,
        "display_name": format!("Test Model: {}", name),
        "engine_type": "llamacpp",
        "engine_settings": {
            "ctx_size": 2048,
            "n_gpu_layers": 0,
        },
        "file_format": "gguf",
        "enabled": true,
    });

    if let Some(version_id) = runtime_version_id {
        payload["required_runtime_version_id"] = json!(version_id.to_string());
    }

    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to create model");

    assert_eq!(response.status(), StatusCode::CREATED);
    response.json().await.unwrap()
}

/// Gets the Hugging Face repository and configures API key
///
/// # Arguments
/// * `server` - Test server instance
/// * `token` - Authentication token
///
/// # Returns
/// Repository JSON object
pub async fn get_huggingface_repository(
    server: &TestServer,
    token: &str,
) -> serde_json::Value {
    let response = reqwest::Client::new()
        .get(&server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();

    // Find the Hugging Face repository
    let repositories = body["repositories"].as_array().unwrap();
    let mut hf_repo = None;
    for repo in repositories {
        if repo["name"].as_str() == Some("Hugging Face Hub") {
            hf_repo = Some(repo.clone());
            break;
        }
    }

    let hf_repo = hf_repo.expect("Hugging Face Hub repository not found in database");

    // Get API key from environment variable
    let api_key = std::env::var("HUGGINGFACE_API_KEY").expect(
        "HUGGINGFACE_API_KEY not set. Please source tests/.env.test or set the environment variable.",
    );

    // Update the repository with the API key
    let repo_id = hf_repo["id"].as_str().unwrap();
    let update_payload = json!({
        "auth_config": {
            "api_key": api_key,
            "auth_test_api_endpoint": "https://huggingface.co/api/whoami-v2"
        }
    });

    let response = reqwest::Client::new()
        .post(&server.api_url(&format!("/llm-repositories/{}", repo_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&update_payload)
        .send()
        .await
        .unwrap();

    let status = response.status();
    if status != StatusCode::OK {
        let error_body = response.text().await.unwrap();
        panic!(
            "Failed to update Hugging Face repository with API key. Status: {}, Body: {}",
            status, error_body
        );
    }

    // Return the updated repository
    response.json().await.unwrap()
}

/// Downloads a small test model file (tiny-random-gpt2)
///
/// This downloads the actual model file from Hugging Face and waits for completion.
/// The model is very small (~few MB) and should complete quickly.
///
/// # Arguments
/// * `server` - Test server instance
/// * `token` - Authentication token
/// * `provider_id` - Provider UUID
///
/// # Returns
/// (model JSON object, model file path)
pub async fn download_test_model(
    server: &TestServer,
    token: &str,
    provider_id: Uuid,
) -> (serde_json::Value, PathBuf) {
    // Get Hugging Face repository
    let repository = get_huggingface_repository(server, token).await;
    let repository_id = Uuid::parse_str(repository["id"].as_str().unwrap()).unwrap();

    // Create download request for tiny-random-gpt2 (small test model)
    let payload = json!({
        "provider_id": provider_id.to_string(),
        "repository_id": repository_id.to_string(),
        "repository_path": "hf-internal-testing/tiny-random-gpt2",
        "repository_branch": "main",
        "name": "tiny-gpt2-test",
        "display_name": "Tiny GPT-2 Test Model",
        "file_format": "safetensors",
        "main_filename": "model.safetensors",
        "source": {
            "type": "hub",
            "id": "hf-internal-testing/tiny-random-gpt2"
        },
        "engine_type": "llamacpp",
        "engine_settings": {
            "ctx_size": 2048,
            "n_gpu_layers": 0,
        },
        "enabled": true,
    });

    println!("Starting tiny-random-gpt2 model download...");
    let response = reqwest::Client::new()
        .post(&server.api_url("/llm-models/download"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to start model download");

    // Model download returns 200 OK with download instance, not 201 CREATED
    if response.status() != StatusCode::OK && response.status() != StatusCode::CREATED {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_else(|_| "Could not read response body".to_string());
        panic!(
            "Failed to initiate model download. Status: {}, Body: {}",
            status, error_body
        );
    }

    let download_instance: serde_json::Value = response.json().await.unwrap();
    let download_id = Uuid::parse_str(download_instance["id"].as_str().unwrap()).unwrap();
    println!("Model download initiated. Download ID: {}", download_id);

    // Wait for download to complete (poll status)
    // TinyLlama is ~600MB, can take 5-10 minutes on slow connections
    let max_wait_seconds = 600; // 10 minutes
    let poll_interval = 5; // 5 seconds
    let max_attempts = max_wait_seconds / poll_interval;

    for attempt in 0..max_attempts {
        tokio::time::sleep(tokio::time::Duration::from_secs(poll_interval)).await;

        let status_response = reqwest::Client::new()
            .get(&server.api_url(&format!("/llm-models/downloads/{}", download_id)))
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .unwrap();

        // Check response status code
        if !status_response.status().is_success() {
            println!("ERROR: Download status endpoint returned {}: {}",
                status_response.status(),
                status_response.text().await.unwrap_or_else(|_| "Could not read response".to_string()));
            tokio::time::sleep(tokio::time::Duration::from_secs(poll_interval)).await;
            continue;
        }

        let download_data: serde_json::Value = status_response.json().await.unwrap();

        // Debug: Print response structure on first attempt
        if attempt == 1 {
            println!("DEBUG: Download response: {}", serde_json::to_string_pretty(&download_data).unwrap_or_else(|_| format!("{:?}", download_data)));
        }

        let status = download_data["status"].as_str().unwrap_or("Unknown");

        println!(
            "Download status: {} (attempt {}/{})",
            status,
            attempt + 1,
            max_attempts
        );

        if status == "completed" {
            // Get model_id from download instance
            let model_id = Uuid::parse_str(download_data["model_id"].as_str()
                .expect("Download instance should have model_id when completed"))
                .expect("Invalid model_id in download instance");

            // Fetch the actual model data
            let model_response = reqwest::Client::new()
                .get(&server.api_url(&format!("/llm-models/{}", model_id)))
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await
                .unwrap();

            let model_data: serde_json::Value = model_response.json().await.unwrap();

            println!("Model download completed! Model ID: {}", model_id);
            // Note: The API response doesn't include storage_path, but that's internal server data
            // For testing purposes, we just need the model metadata to proceed with instance management
            return (model_data, PathBuf::new());
        } else if status == "failed" {
            panic!("Model download failed: {:?}", download_data);
        }
    }

    panic!(
        "Model download timed out after {} seconds",
        max_wait_seconds
    );
}
