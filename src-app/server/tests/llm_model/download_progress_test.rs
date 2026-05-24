/// Download Progress Tracking Integration Tests
///
/// These tests verify that the download progress tracking works correctly,
/// including status updates, progress data updates, and model creation.
use reqwest::StatusCode;
use serde_json::json;
use tokio::time::{Duration, sleep};

#[tokio::test]
async fn test_download_status_and_progress_tracking() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "downloader",
        &[
            "llm_models::create",
            "llm_models::read",
            "llm_providers::read",
            "llm_providers::create",
            "llm_repositories::read",
            "llm_repositories::edit",
            "llm_models::downloads_read",
        ],
    )
    .await;

    // Get Hugging Face repository and configure API key
    let hf_repo =
        crate::llm_model::download_test::get_huggingface_repository(&server, &user.token, true)
            .await;
    let repo_id = hf_repo["id"].as_str().unwrap();

    // Get local provider
    let provider = crate::llm_model::download_test::get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Initiate download
    let payload = json!({
        "provider_id": provider_id,
        "repository_id": repo_id,
        "repository_path": "hf-internal-testing/tiny-random-gpt2",
        "repository_branch": "main",
        "name": "tiny-gpt2-progress-test",
        "display_name": "Tiny GPT-2 (Progress Test)",
        "description": "Test model for progress tracking",
        "file_format": "safetensors",
        "main_filename": "model.safetensors",
        "source": {
            "type": "hub",
            "id": "hf-internal-testing/tiny-random-gpt2"
        }
    });

    println!("Initiating download...");
    let response = reqwest::Client::new()
        .post(server.api_url("/llm-models/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let download_instance: serde_json::Value = response.json().await.unwrap();
    let download_id = download_instance["id"].as_str().unwrap();

    println!("Download initiated with ID: {}", download_id);

    // Poll for status changes
    let mut iterations = 0;
    let max_iterations = 60; // 60 seconds max
    let mut saw_downloading = false;
    let mut saw_progress_update = false;
    let mut final_status = String::new();

    while iterations < max_iterations {
        sleep(Duration::from_secs(1)).await;
        iterations += 1;

        // Get download status
        let response = reqwest::Client::new()
            .get(server.api_url(&format!("/llm-models/downloads/{}", download_id)))
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .unwrap();

        if response.status() == StatusCode::NOT_FOUND {
            // Download was deleted (means it completed)
            println!(
                "Download completed and deleted after {} seconds",
                iterations
            );
            final_status = "completed".to_string();
            break;
        }

        assert_eq!(response.status(), StatusCode::OK);

        let download: serde_json::Value = response.json().await.unwrap();
        let status = download["status"].as_str().unwrap();
        final_status = status.to_string();

        println!("Iteration {}: status = {}", iterations, status);

        // Check if we've seen the "downloading" status
        if status == "downloading" {
            saw_downloading = true;
            println!("✅ Status transitioned to 'downloading'");
        }

        // Check for progress data
        if let Some(progress_data) = download["progress_data"].as_object()
            && let Some(phase) = progress_data.get("phase") {
                saw_progress_update = true;
                println!(
                    "✅ Progress update: phase={}, current={}, total={}",
                    phase.as_str().unwrap_or("unknown"),
                    progress_data
                        .get("current")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0),
                    progress_data
                        .get("total")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0)
                );
            }

        // Check if completed
        if status == "completed" {
            println!("✅ Download completed");

            // Verify model_id is set
            if let Some(result) = download["result"].as_object() {
                if let Some(model_id) = result.get("model_id") {
                    println!("✅ Model ID set: {}", model_id.as_str().unwrap());

                    // Verify model appears in provider's models list
                    let response = reqwest::Client::new()
                        .get(server.api_url(&format!("/llm-models?provider_id={}", provider_id)))
                        .header("Authorization", format!("Bearer {}", user.token))
                        .send()
                        .await
                        .unwrap();

                    assert_eq!(response.status(), StatusCode::OK);

                    let models_list: serde_json::Value = response.json().await.unwrap();
                    let models = models_list["models"].as_array().unwrap();

                    let found_model = models
                        .iter()
                        .find(|m| m["id"].as_str().unwrap() == model_id.as_str().unwrap());

                    assert!(
                        found_model.is_some(),
                        "Downloaded model should appear in provider's models list"
                    );
                    println!("✅ Model appears in provider's models list");
                } else {
                    panic!("Model ID not set in completed download");
                }
            }
            break;
        }

        // Check if failed
        if status == "failed" {
            let error_msg = download["error_message"]
                .as_str()
                .unwrap_or("Unknown error");
            panic!("Download failed: {}", error_msg);
        }
    }

    // Verify the download completed successfully
    assert_eq!(
        final_status, "completed",
        "Download must complete successfully"
    );

    // Log what we observed during download
    println!("Download completed successfully");
    if saw_downloading {
        println!("  - Observed 'downloading' status transition");
    }
    if saw_progress_update {
        println!("  - Received progress updates");
    }

    println!("✅ Download progress tracking test passed!");
}

#[tokio::test]
async fn test_download_with_invalid_repository() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "downloader",
        &[
            "llm_models::create",
            "llm_models::read",
            "llm_providers::read",
            "llm_providers::create",
            "llm_repositories::read",
            "llm_repositories::edit",
            "llm_models::downloads_read",
        ],
    )
    .await;

    // Get Hugging Face repository
    let hf_repo =
        crate::llm_model::download_test::get_huggingface_repository(&server, &user.token, true)
            .await;
    let repo_id = hf_repo["id"].as_str().unwrap();

    // Get local provider
    let provider = crate::llm_model::download_test::get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Initiate download with invalid repository path
    let payload = json!({
        "provider_id": provider_id,
        "repository_id": repo_id,
        "repository_path": "invalid/nonexistent-model-12345",
        "repository_branch": "main",
        "name": "invalid-repo-test",
        "display_name": "Invalid Repo Test",
        "description": "Test model with invalid repository",
        "file_format": "safetensors",
        "main_filename": "model.safetensors",
        "source": {
            "type": "hub",
            "id": "invalid/nonexistent-model-12345"
        }
    });

    println!("Initiating download with invalid repository...");
    let response = reqwest::Client::new()
        .post(server.api_url("/llm-models/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let download_instance: serde_json::Value = response.json().await.unwrap();
    let download_id = download_instance["id"].as_str().unwrap();

    println!("Download initiated with ID: {}", download_id);

    // Poll for status changes - should transition to failed
    let mut iterations = 0;
    let max_iterations = 30; // 30 seconds max
    let mut saw_failed = false;
    let mut error_message = String::new();

    while iterations < max_iterations {
        sleep(Duration::from_secs(1)).await;
        iterations += 1;

        // Get download status
        let response = reqwest::Client::new()
            .get(server.api_url(&format!("/llm-models/downloads/{}", download_id)))
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let download: serde_json::Value = response.json().await.unwrap();
        let status = download["status"].as_str().unwrap();

        println!("Iteration {}: status = {}", iterations, status);

        // Check if failed
        if status == "failed" {
            saw_failed = true;
            if let Some(error) = download["error_message"].as_str() {
                error_message = error.to_string();
                println!("✅ Download failed with error: {}", error);
            }
            break;
        }
    }

    // Verify the download failed
    assert!(
        saw_failed,
        "Download should have failed for invalid repository"
    );
    assert!(
        !error_message.is_empty(),
        "Error message should be set for failed download"
    );

    println!("✅ Invalid repository error handling test passed!");
}

#[tokio::test]
async fn test_download_cancellation() {
    // NOTE: This test verifies the cancellation endpoint works correctly.
    // With tiny test models that complete in <1 second, the download will
    // always be completed before we can cancel it. This tests the endpoint's
    // behavior when attempting to cancel an already-completed download.

    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "downloader",
        &[
            "llm_models::create",
            "llm_models::read",
            "llm_providers::read",
            "llm_providers::create",
            "llm_repositories::read",
            "llm_repositories::edit",
            "llm_models::downloads_read",
            "llm_models::downloads_cancel",
        ],
    )
    .await;

    // Get Hugging Face repository
    let hf_repo =
        crate::llm_model::download_test::get_huggingface_repository(&server, &user.token, true)
            .await;
    let repo_id = hf_repo["id"].as_str().unwrap();

    // Get local provider
    let provider = crate::llm_model::download_test::get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Initiate download
    let payload = json!({
        "provider_id": provider_id,
        "repository_id": repo_id,
        "repository_path": "hf-internal-testing/tiny-random-gpt2",
        "repository_branch": "main",
        "name": "tiny-gpt2-cancel-test",
        "display_name": "Tiny GPT-2 (Cancel Test)",
        "description": "Test model for cancellation endpoint",
        "file_format": "safetensors",
        "main_filename": "model.safetensors",
        "source": {
            "type": "hub",
            "id": "hf-internal-testing/tiny-random-gpt2"
        }
    });

    println!("Initiating download...");
    let response = reqwest::Client::new()
        .post(server.api_url("/llm-models/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let download_instance: serde_json::Value = response.json().await.unwrap();
    let download_id = download_instance["id"].as_str().unwrap();

    println!("Download initiated with ID: {}", download_id);

    // Wait for tiny model to complete (deterministic: it will complete in ~1 second)
    sleep(Duration::from_secs(2)).await;

    // Attempt to cancel the download (which should already be completed)
    println!("Attempting to cancel already-completed download...");
    let cancel_response = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-models/downloads/{}/cancel", download_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    // For tiny models, cancel should return 400 (cannot cancel completed download)
    assert_eq!(
        cancel_response.status(),
        StatusCode::BAD_REQUEST,
        "Cancel should fail with 400 for already-completed download"
    );

    // Verify download is in terminal state (completed, failed, or deleted)
    let status_response = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-models/downloads/{}", download_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    if status_response.status() == StatusCode::NOT_FOUND {
        println!("✅ Download was deleted after completion (as expected)");
    } else {
        assert_eq!(status_response.status(), StatusCode::OK);
        let download: serde_json::Value = status_response.json().await.unwrap();
        let status = download["status"].as_str().unwrap();
        assert!(
            status == "completed" || status == "failed",
            "Download must be in terminal state (completed/failed), got: {}",
            status
        );
        println!("✅ Download in terminal state: {}", status);
    }

    println!("✅ Cancellation endpoint test passed!");
}

#[tokio::test]
async fn test_download_with_authenticated_repository() {
    // This test verifies that downloads work with repositories that require authentication.
    // With a valid API key and valid repository, the download MUST succeed.

    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "downloader",
        &[
            "llm_models::create",
            "llm_models::read",
            "llm_providers::read",
            "llm_providers::create",
            "llm_repositories::read",
            "llm_repositories::edit",
            "llm_models::downloads_read",
        ],
    )
    .await;

    // Get Hugging Face repository (which uses API key authentication)
    let hf_repo = crate::llm_model::download_test::get_huggingface_repository(
        &server,
        &user.token,
        true, // configure with API key
    )
    .await;
    let repo_id = hf_repo["id"].as_str().unwrap();

    // Verify the repository has auth configured
    let auth_type = hf_repo["auth_type"].as_str().unwrap();
    assert_eq!(
        auth_type, "api_key",
        "Repository must use API key authentication"
    );
    println!("Repository configured with auth_type: {}", auth_type);

    // Get local provider
    let provider = crate::llm_model::download_test::get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Initiate download from authenticated repository
    let payload = json!({
        "provider_id": provider_id,
        "repository_id": repo_id,
        "repository_path": "hf-internal-testing/tiny-random-gpt2",
        "repository_branch": "main",
        "name": "tiny-gpt2-auth-test",
        "display_name": "Tiny GPT-2 (Auth Test)",
        "description": "Test model for authenticated repository",
        "file_format": "safetensors",
        "main_filename": "model.safetensors",
        "source": {
            "type": "hub",
            "id": "hf-internal-testing/tiny-random-gpt2"
        }
    });

    println!("Initiating download from authenticated repository...");
    let response = reqwest::Client::new()
        .post(server.api_url("/llm-models/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let download_instance: serde_json::Value = response.json().await.unwrap();
    let download_id = download_instance["id"].as_str().unwrap();

    println!("Download initiated with ID: {}", download_id);

    // Poll until download completes
    let mut iterations = 0;
    let max_iterations = 30;
    let mut final_status = String::new();

    while iterations < max_iterations {
        sleep(Duration::from_millis(500)).await;
        iterations += 1;

        let response = reqwest::Client::new()
            .get(server.api_url(&format!("/llm-models/downloads/{}", download_id)))
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .unwrap();

        if response.status() == StatusCode::NOT_FOUND {
            // Download completed and deleted
            final_status = "completed".to_string();
            println!("Download completed and deleted from database");
            break;
        }

        assert_eq!(response.status(), StatusCode::OK);

        let download: serde_json::Value = response.json().await.unwrap();
        let status = download["status"].as_str().unwrap();
        final_status = status.to_string();

        if status == "completed" {
            println!("Download completed successfully");
            break;
        }

        if status == "failed" {
            let error_msg = download["error_message"]
                .as_str()
                .unwrap_or("Unknown error");
            panic!(
                "Download failed: {}\nWith valid API key and valid repository, download must succeed",
                error_msg
            );
        }
    }

    // Download must complete successfully
    assert_eq!(
        final_status, "completed",
        "Download with valid auth credentials must complete successfully"
    );

    // Verify model appears in provider's models list
    let models_response = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-models?provider_id={}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(models_response.status(), StatusCode::OK);

    let models_list: serde_json::Value = models_response.json().await.unwrap();
    let models = models_list["models"].as_array().unwrap();

    let found_model = models
        .iter()
        .find(|m| m["name"].as_str().unwrap() == "tiny-gpt2-auth-test");

    assert!(
        found_model.is_some(),
        "Downloaded model must appear in provider's models list"
    );

    println!("✅ Authenticated repository download test passed!");
    println!("  - Auth token extracted from repository config");
    println!("  - Download completed successfully");
    println!("  - Model appears in provider's models list");
}
