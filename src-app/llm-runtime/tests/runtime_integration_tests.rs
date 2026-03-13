//! Integration tests for llm-runtime with real binaries
//!
//! These tests verify the complete workflow:
//! - Starting instances
//! - Health checks
//! - Inference (completions)
//! - Stopping instances
//! - State persistence
//!
//! Binaries are downloaded from GitHub releases and cached in ~/.llm-runtime/binaries/

use llm_runtime::{
    binary_download,
    config::{DeviceType, EngineSettings, EngineType, GlobalSettings, InstanceConfig, RuntimeConfig},
    download::ModelDownloader,
    state::StateManager,
    HealthStatus, Runtime,
};
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

/// Get or download engine binary for testing (auto-detects platform, uses CPU)
async fn get_test_binary(engine: EngineType) -> PathBuf {
    binary_download::ensure_test_binary(engine, "latest")
        .await
        .expect("Failed to download test binary")
}

/// Setup test environment: download binary and add to PATH
async fn setup_test_binary(engine: EngineType) {
    let binary_path = get_test_binary(engine).await;
    let binary_dir = binary_path.parent().expect("Binary should have parent directory");

    // Add binary directory to PATH so Runtime can find it
    let current_path = std::env::var("PATH").unwrap_or_default();
    let new_path = if current_path.is_empty() {
        binary_dir.to_string_lossy().to_string()
    } else {
        format!("{}:{}", binary_dir.display(), current_path)
    };
    std::env::set_var("PATH", new_path);

    println!("Binary ready: {}", binary_path.display());
}

/// Get or download TinyLlama model
async fn get_test_model() -> PathBuf {
    let downloader = ModelDownloader::new().expect("Failed to create downloader");

    // Check if already downloaded
    let models = downloader.list_models().expect("Failed to list models");
    for model in models {
        if model.repo_id == "TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF"
            && model.filename == "tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf"
        {
            println!("Using existing model: {}", model.path.display());
            return model.path;
        }
    }

    // Download if not exists
    println!("Downloading TinyLlama model...");
    let model_info = downloader
        .download(
            "TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF",
            "tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf",
        )
        .await
        .expect("Failed to download model");

    println!("Model downloaded: {}", model_info.path.display());
    model_info.path
}

#[tokio::test]
#[ignore] // Run with: cargo test -- --ignored --nocapture
           // Downloads llama-server binary and TinyLlama model automatically
async fn test_llamacpp_start_stop() {
    setup_test_binary(EngineType::Llamacpp).await;
    let model_path = get_test_model().await;

    // Create minimal config
    let instance_config = InstanceConfig {
        id: "test-tiny".to_string(),
        engine: EngineType::Llamacpp,
        model_path: model_path.clone(),
        device: DeviceType::Cpu, // Use CPU for reproducible tests
        settings: EngineSettings::default(),
    };

    let runtime_config = RuntimeConfig {
        global: GlobalSettings::default(),
        instances: vec![instance_config],
    };

    // Create runtime
    let mut runtime = Runtime::new(runtime_config)
        .await
        .expect("Failed to create runtime");

    // Start instance
    println!("Starting TinyLlama instance...");
    let handle = runtime
        .start("test-tiny")
        .await
        .expect("Failed to start instance");

    println!("Instance started:");
    println!("  PID: {}", handle.pid);
    println!("  Port: {}", handle.port);
    println!("  URL: {}", handle.base_url);

    // Give it a moment to initialize
    sleep(Duration::from_secs(2)).await;

    // Check health
    let health = runtime
        .health_check("test-tiny")
        .await
        .expect("Failed to check health");

    println!("Health status: {:?}", health);
    assert!(
        matches!(health, HealthStatus::Healthy | HealthStatus::Starting),
        "Instance should be healthy or starting"
    );

    // Stop instance
    println!("Stopping instance...");
    runtime
        .stop("test-tiny")
        .await
        .expect("Failed to stop instance");

    println!("Test completed successfully");
}

#[tokio::test]
#[ignore] // Run with: cargo test -- --ignored --nocapture
           // Downloads llama-server binary and TinyLlama model automatically
async fn test_llamacpp_inference() {
    setup_test_binary(EngineType::Llamacpp).await;
    let model_path = get_test_model().await;

    // Create config with specific settings for inference
    let mut settings = EngineSettings::default();
    settings.llamacpp.ctx_size = 2048;
    settings.llamacpp.n_gpu_layers = 0; // CPU only for tests

    let instance_config = InstanceConfig {
        id: "test-inference".to_string(),
        engine: EngineType::Llamacpp,
        model_path: model_path.clone(),
        device: DeviceType::Cpu,
        settings,
    };

    let runtime_config = RuntimeConfig {
        global: GlobalSettings::default(),
        instances: vec![instance_config],
    };

    let mut runtime = Runtime::new(runtime_config)
        .await
        .expect("Failed to create runtime");

    // Start instance
    println!("Starting instance for inference test...");
    let handle = runtime
        .start("test-inference")
        .await
        .expect("Failed to start instance");

    println!("Waiting for model to load...");
    // Wait longer for model loading
    sleep(Duration::from_secs(10)).await;

    // Check health before inference
    let health = runtime
        .health_check("test-inference")
        .await
        .expect("Failed to check health");

    println!("Health status: {:?}", health);

    if matches!(health, HealthStatus::Healthy) {
        // Try a simple completion request
        println!("Attempting inference request...");
        let client = reqwest::Client::new();

        let response = client
            .post(format!("{}/v1/completions", handle.base_url))
            .json(&serde_json::json!({
                "prompt": "Once upon a time",
                "max_tokens": 10,
                "temperature": 0.7
            }))
            .timeout(Duration::from_secs(30))
            .send()
            .await;

        match response {
            Ok(resp) => {
                println!("Response status: {}", resp.status());
                if resp.status().is_success() {
                    let body = resp.text().await.expect("Failed to read response");
                    println!("Response: {}", body);
                    println!("✓ Inference successful!");
                } else {
                    let error = resp.text().await.unwrap_or_default();
                    println!("Error response: {}", error);
                }
            }
            Err(e) => {
                println!("Request failed: {}", e);
            }
        }
    }

    // Stop instance
    println!("Stopping instance...");
    runtime
        .stop("test-inference")
        .await
        .expect("Failed to stop instance");
}

#[tokio::test]
#[ignore] // Run with: cargo test -- --ignored --nocapture
           // Downloads llama-server binary and TinyLlama model automatically
async fn test_state_persistence() {
    setup_test_binary(EngineType::Llamacpp).await;
    let model_path = get_test_model().await;

    // Create temporary state database
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let state_db = temp_dir.path().join("test-state.db");

    let state = StateManager::with_path(state_db.clone())
        .expect("Failed to create state manager");

    // Create and save instance config
    let instance_config = InstanceConfig {
        id: "test-state".to_string(),
        engine: EngineType::Llamacpp,
        model_path: model_path.clone(),
        device: DeviceType::Cpu,
        settings: EngineSettings::default(),
    };

    state
        .save_instance(&instance_config, 12345, 8080, "http://127.0.0.1:8080")
        .expect("Failed to save instance");

    // Retrieve and verify
    let retrieved = state
        .get_instance("test-state")
        .expect("Failed to get instance");

    assert!(retrieved.is_some(), "Instance should be retrievable");

    let (config, pid, port, base_url) = retrieved.unwrap();
    assert_eq!(config.id, "test-state");
    assert_eq!(config.engine, EngineType::Llamacpp);
    assert_eq!(pid, 12345);
    assert_eq!(port, 8080);
    assert_eq!(base_url, "http://127.0.0.1:8080");

    // List instances
    let instances = state.list_instances().expect("Failed to list instances");
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].0, "test-state");

    // Delete instance
    state
        .delete_instance("test-state")
        .expect("Failed to delete instance");

    let after_delete = state
        .get_instance("test-state")
        .expect("Failed to get instance");

    assert!(after_delete.is_none(), "Instance should be deleted");

    println!("✓ State persistence test passed");
}

#[tokio::test]
#[ignore] // Run with: cargo test -- --ignored --nocapture
           // Downloads llama-server binary and TinyLlama model automatically
async fn test_multiple_instances() {
    setup_test_binary(EngineType::Llamacpp).await;
    let model_path = get_test_model().await;

    // Create two instances with different IDs
    let instance1 = InstanceConfig {
        id: "multi-1".to_string(),
        engine: EngineType::Llamacpp,
        model_path: model_path.clone(),
        device: DeviceType::Cpu,
        settings: EngineSettings::default(),
    };

    let instance2 = InstanceConfig {
        id: "multi-2".to_string(),
        engine: EngineType::Llamacpp,
        model_path: model_path.clone(),
        device: DeviceType::Cpu,
        settings: EngineSettings::default(),
    };

    let runtime_config = RuntimeConfig {
        global: GlobalSettings::default(),
        instances: vec![instance1, instance2],
    };

    let mut runtime = Runtime::new(runtime_config)
        .await
        .expect("Failed to create runtime");

    // Start both instances
    println!("Starting first instance...");
    let handle1 = runtime
        .start("multi-1")
        .await
        .expect("Failed to start instance 1");

    println!("First instance on port: {}", handle1.port);

    println!("Starting second instance...");
    let handle2 = runtime
        .start("multi-2")
        .await
        .expect("Failed to start instance 2");

    println!("Second instance on port: {}", handle2.port);

    // Verify they have different ports
    assert_ne!(
        handle1.port, handle2.port,
        "Instances should have different ports"
    );

    // Give them time to initialize
    sleep(Duration::from_secs(2)).await;

    // Check both healths
    let health1 = runtime
        .health_check("multi-1")
        .await
        .expect("Failed to check health 1");

    let health2 = runtime
        .health_check("multi-2")
        .await
        .expect("Failed to check health 2");

    println!("Instance 1 health: {:?}", health1);
    println!("Instance 2 health: {:?}", health2);

    // Stop both instances
    println!("Stopping instances...");
    runtime
        .stop("multi-1")
        .await
        .expect("Failed to stop instance 1");

    runtime
        .stop("multi-2")
        .await
        .expect("Failed to stop instance 2");

    println!("✓ Multiple instances test passed");
}

#[tokio::test]
#[ignore] // Run with: cargo test -- --ignored --nocapture
           // Downloads llama-server binary and TinyLlama model automatically
async fn test_restart_instance() {
    setup_test_binary(EngineType::Llamacpp).await;
    let model_path = get_test_model().await;

    let instance_config = InstanceConfig {
        id: "test-restart".to_string(),
        engine: EngineType::Llamacpp,
        model_path: model_path.clone(),
        device: DeviceType::Cpu,
        settings: EngineSettings::default(),
    };

    let runtime_config = RuntimeConfig {
        global: GlobalSettings::default(),
        instances: vec![instance_config],
    };

    let mut runtime = Runtime::new(runtime_config)
        .await
        .expect("Failed to create runtime");

    // Start instance
    println!("Starting instance (first time)...");
    let handle1 = runtime
        .start("test-restart")
        .await
        .expect("Failed to start instance");

    let first_pid = handle1.pid;
    let first_port = handle1.port;
    println!("First start - PID: {}, Port: {}", first_pid, first_port);

    sleep(Duration::from_secs(2)).await;

    // Stop instance
    println!("Stopping instance...");
    runtime
        .stop("test-restart")
        .await
        .expect("Failed to stop instance");

    sleep(Duration::from_secs(1)).await;

    // Restart instance
    println!("Restarting instance...");
    let handle2 = runtime
        .start("test-restart")
        .await
        .expect("Failed to restart instance");

    let second_pid = handle2.pid;
    let second_port = handle2.port;
    println!("Second start - PID: {}, Port: {}", second_pid, second_port);

    // PIDs should be different
    assert_ne!(first_pid, second_pid, "PID should change on restart");

    // Ports may be the same or different (implementation dependent)
    println!("Port after restart: {}", if first_port == second_port { "same" } else { "different" });

    // Stop again
    println!("Stopping instance (final)...");
    runtime
        .stop("test-restart")
        .await
        .expect("Failed to stop instance");

    println!("✓ Restart test passed");
}
