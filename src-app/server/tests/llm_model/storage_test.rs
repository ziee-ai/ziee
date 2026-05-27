/// File System Operations Integration Tests
///
/// These tests verify that file operations (storage, cleanup, validation)
/// work correctly for model uploads and downloads.
use crate::common::{TestServer, test_helpers};
use std::fs;
use std::path::Path;

#[tokio::test]
async fn test_delete_downloaded_model_removes_files() {
    // This test verifies that deleting a downloaded model removes its files from disk
    use tokio::time::{Duration, sleep};

    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "llm_models::create",
            "llm_models::read",
            "llm_models::delete",
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

    // Download a model
    let payload = serde_json::json!({
        "provider_id": provider_id,
        "repository_id": repo_id,
        "repository_path": "hf-internal-testing/tiny-random-gpt2",
        "repository_branch": "main",
        "name": "tiny-gpt2-delete-test",
        "display_name": "Tiny GPT-2 (Delete Test)",
        "description": "Test model for file deletion",
        "file_format": "safetensors",
        "main_filename": "model.safetensors",
        "source": {
            "type": "hub",
            "id": "hf-internal-testing/tiny-random-gpt2"
        }
    });

    let download_response = reqwest::Client::new()
        .post(server.api_url("/llm-models/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Download request failed");

    assert_eq!(download_response.status(), 200);

    let download_instance: serde_json::Value = download_response
        .json()
        .await
        .expect("Failed to parse download response");
    let download_id = download_instance["id"].as_str().unwrap();

    println!("Download initiated with ID: {}", download_id);

    // Wait for download to complete
    let mut iterations = 0;
    let max_iterations = 30;
    let mut model_id: Option<String> = None;

    while iterations < max_iterations {
        sleep(Duration::from_secs(1)).await;
        iterations += 1;

        let response = reqwest::Client::new()
            .get(server.api_url(&format!("/llm-models/downloads/{}", download_id)))
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .expect("Get download request failed");

        if response.status() == 404 {
            // Download completed and deleted
            println!("Download completed and deleted");
            break;
        }

        assert_eq!(response.status(), 200);

        let download: serde_json::Value = response.json().await.expect("Failed to parse download");
        let status = download["status"].as_str().unwrap();

        if status == "completed" {
            model_id = Some(download["model_id"].as_str().unwrap().to_string());
            println!(
                "✅ Download completed, model_id: {}",
                model_id.as_ref().unwrap()
            );
            break;
        }

        if status == "failed" {
            let error = download["error_message"]
                .as_str()
                .unwrap_or("Unknown error");
            panic!("Download failed: {}", error);
        }
    }

    // Get the created model
    let model_id = model_id.expect("Download should have created a model");

    let get_response = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-models?provider_id={}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Get models request failed");

    assert_eq!(get_response.status(), 200);

    let models_list: serde_json::Value = get_response
        .json()
        .await
        .expect("Failed to parse models list");
    let models = models_list["models"]
        .as_array()
        .expect("Should have models array");
    let _model = models
        .iter()
        .find(|m| m["id"].as_str().unwrap() == model_id)
        .expect("Downloaded model should appear in list");

    // Construct the model path manually since API doesn't return file paths
    // Model storage path is: {app_data_dir}/models/{provider_id}/{model_id}
    // App data dir defaults to home directory
    let app_data_dir = std::env::var("APP_DATA_DIR").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        format!("{}/.ziee", home)
    });
    let model_path = Path::new(&app_data_dir)
        .join("models")
        .join(provider_id)
        .join(&model_id);

    println!("Model stored at: {}", model_path.display());

    // Verify files exist on disk BEFORE deletion
    assert!(
        model_path.exists(),
        "Model directory should exist before deletion: {}",
        model_path.display()
    );

    // Count files in the model directory
    let file_count_before = fs::read_dir(&model_path)
        .expect("Should be able to read model directory")
        .count();

    println!(
        "Files in model directory before deletion: {}",
        file_count_before
    );
    assert!(
        file_count_before > 0,
        "Model directory should contain files"
    );

    // Delete the model via API
    let delete_response = reqwest::Client::new()
        .delete(server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Delete request failed");

    let status = delete_response.status();
    assert!(
        status == 200 || status == 204,
        "Delete should succeed (200 or 204): got {}",
        status
    );

    println!("✅ Model deleted via API");

    // Verify files are removed from disk AFTER deletion
    assert!(
        !model_path.exists(),
        "Model directory should be removed after deletion: {}",
        model_path.display()
    );

    println!("✅ Model directory removed from disk");
    println!("✅ File cleanup test passed!");
}

#[tokio::test]
async fn test_download_creates_correct_file_structure() {
    // This test verifies that downloading a model creates the correct file structure

    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
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

    // Initiate download
    let payload = serde_json::json!({
        "provider_id": provider_id,
        "repository_id": repo_id,
        "repository_path": "hf-internal-testing/tiny-random-gpt2",
        "repository_branch": "main",
        "name": "tiny-gpt2-storage-test",
        "display_name": "Tiny GPT-2 (Storage Test)",
        "description": "Test model for file storage verification",
        "file_format": "safetensors",
        "main_filename": "model.safetensors",
        "source": {
            "type": "hub",
            "id": "hf-internal-testing/tiny-random-gpt2"
        }
    });

    let download_response = reqwest::Client::new()
        .post(server.api_url("/llm-models/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Download request failed");

    assert_eq!(download_response.status(), 200);

    let download_instance: serde_json::Value = download_response
        .json()
        .await
        .expect("Failed to parse download response");
    let download_id = download_instance["id"].as_str().unwrap();

    println!("Download initiated with ID: {}", download_id);

    // Wait for download to complete
    use tokio::time::{Duration, sleep};
    let mut iterations = 0;
    let max_iterations = 30;
    let mut model_id: Option<String> = None;

    while iterations < max_iterations {
        sleep(Duration::from_secs(1)).await;
        iterations += 1;

        let response = reqwest::Client::new()
            .get(server.api_url(&format!("/llm-models/downloads/{}", download_id)))
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .expect("Get download request failed");

        if response.status() == 404 {
            // Download completed and deleted
            println!("Download completed and deleted");
            break;
        }

        assert_eq!(response.status(), 200);

        let download: serde_json::Value = response.json().await.expect("Failed to parse download");
        let status = download["status"].as_str().unwrap();

        if status == "completed" {
            model_id = Some(download["model_id"].as_str().unwrap().to_string());
            println!(
                "✅ Download completed, model_id: {}",
                model_id.as_ref().unwrap()
            );
            break;
        }

        if status == "failed" {
            let error = download["error_message"]
                .as_str()
                .unwrap_or("Unknown error");
            panic!("Download failed: {}", error);
        }
    }

    // Get the created model
    let model_id = model_id.expect("Download should have created a model");

    let get_response = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-models?provider_id={}", provider_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Get models request failed");

    assert_eq!(get_response.status(), 200);

    let models_list: serde_json::Value = get_response
        .json()
        .await
        .expect("Failed to parse models list");
    let models = models_list["models"]
        .as_array()
        .expect("Should have models array");
    let _model = models
        .iter()
        .find(|m| m["id"].as_str().unwrap() == model_id)
        .expect("Downloaded model should appear in list");

    // Construct the model path manually since API doesn't return file paths
    // Model storage path is: {app_data_dir}/models/{provider_id}/{model_id}
    // App data dir defaults to home directory
    let app_data_dir = std::env::var("APP_DATA_DIR").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        format!("{}/.ziee", home)
    });
    let model_path = Path::new(&app_data_dir)
        .join("models")
        .join(provider_id)
        .join(&model_id);

    println!("Model stored at: {}", model_path.display());

    // Verify directory exists
    assert!(
        model_path.exists(),
        "Model directory should exist: {}",
        model_path.display()
    );
    assert!(
        model_path.is_dir(),
        "Model path should be a directory: {}",
        model_path.display()
    );

    // Verify expected files exist
    let expected_files = vec![
        "model.safetensors",
        "config.json",
        "tokenizer.json",
        "vocab.json",
        "merges.txt",
    ];

    for filename in expected_files {
        let file_path = model_path.join(filename);
        assert!(
            file_path.exists(),
            "Expected file should exist: {}",
            file_path.display()
        );

        // Verify file has content
        let metadata = fs::metadata(&file_path).expect("Should be able to read file metadata");
        assert!(
            metadata.len() > 0,
            "File should have content: {}",
            file_path.display()
        );
    }

    println!("✅ All expected files exist and have content");

    // Verify main model file is the largest (safetensors file)
    let main_file_path = model_path.join("model.safetensors");
    let main_file_size = fs::metadata(&main_file_path)
        .expect("Should be able to read main file metadata")
        .len();

    assert!(
        main_file_size > 1000,
        "Main model file should be substantial in size"
    );

    println!("✅ Main model file size: {} bytes", main_file_size);
    println!("✅ File structure verification test passed!");
}
