use reqwest::StatusCode;
use reqwest::multipart::Form;
use reqwest::multipart::Part;
use serde_json::json;
use std::path::PathBuf;
use tokio::fs;

/// Test models to use for upload testing
/// These are selected for being very small (1-3MB each for testing)
/// Format: (repo_id, filename, file_format, display_name)
const TEST_MODELS: &[(&str, &str, &str, &str)] = &[
    // SafeTensors models (modern format, HF internal testing models are tiny)
    (
        "hf-internal-testing/tiny-random-gpt2",
        "model.safetensors",
        "safetensors",
        "Tiny Random GPT-2",
    ),
    (
        "hf-internal-testing/tiny-random-gpt2",
        "model.safetensors",
        "safetensors",
        "Tiny Random GPT-2 #2",
    ),
    // GGUF models (quantized format for llama.cpp) - using 2 different quantization levels
    (
        "tensorblock/tiny-mistral-test-GGUF",
        "tiny-mistral-test-Q2_K.gguf",
        "gguf",
        "Tiny Mistral Q2_K",
    ),
    (
        "tensorblock/tiny-mistral-test-GGUF",
        "tiny-mistral-test-Q4_0.gguf",
        "gguf",
        "Tiny Mistral Q4_0",
    ),
    // PyTorch models (legacy PyTorch .bin format)
    (
        "stas/tiny-wmt19-en-de",
        "pytorch_model.bin",
        "pytorch",
        "Tiny WMT19 EN-DE",
    ),
    (
        "prajjwal1/bert-tiny",
        "pytorch_model.bin",
        "pytorch",
        "BERT Tiny",
    ),
];

/// Helper to download a model from HuggingFace using hf CLI
async fn download_test_model(repo_id: &str, filename: &str) -> Result<PathBuf, String> {
    let cache_dir = std::env::temp_dir().join("ziee-test-models");
    fs::create_dir_all(&cache_dir)
        .await
        .map_err(|e| e.to_string())?;

    let model_dir = cache_dir.join(repo_id.replace("/", "_"));

    // Check if already downloaded
    let model_path = model_dir.join(filename);
    if model_path.exists() {
        println!("Model already cached at: {}", model_path.display());
        return Ok(model_path);
    }

    fs::create_dir_all(&model_dir)
        .await
        .map_err(|e| e.to_string())?;

    println!("Downloading model {} from {}...", filename, repo_id);

    // Use hf download command
    let output = tokio::process::Command::new("hf")
        .args([
            "download",
            repo_id,
            filename,
            "--local-dir",
            &model_dir.to_string_lossy(),
            "--quiet",
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to execute hf command: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "hf download failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    if !model_path.exists() {
        return Err(format!(
            "Model file not found after download: {}",
            model_path.display()
        ));
    }

    println!("Model downloaded successfully to: {}", model_path.display());
    Ok(model_path)
}

/// Helper to get or create a local provider
async fn get_local_provider(server: &crate::common::TestServer, token: &str) -> serde_json::Value {
    // Try to get existing local provider
    let response = reqwest::Client::new()
        .get(server.api_url("/llm-providers"))
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
    let payload = json!({
        "name": "Local Models",
        "provider_type": "local",
        "display_name": "Local Models",
        "enabled": true
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    response.json().await.unwrap()
}

/// Helper function to test uploading a model
async fn test_upload_model_helper(
    server: &crate::common::TestServer,
    user_token: &str,
    provider_id: &str,
    repo_id: &str,
    filename: &str,
    file_format: &str,
    display_name: &str,
    model_name: &str,
) {
    // Download test model
    let model_path = match download_test_model(repo_id, filename).await {
        Ok(path) => path,
        Err(e) => {
            println!("⚠ Skipping upload test for {}: {}", filename, e);
            println!("Make sure 'hf' CLI is installed and you have internet connection");
            return;
        }
    };

    // Read the model file
    let file_data = fs::read(&model_path).await.unwrap();
    println!("Model file size: {} bytes", file_data.len());

    // Create multipart form
    let file_part = Part::bytes(file_data)
        .file_name(filename.to_string())
        .mime_str("application/octet-stream")
        .unwrap();

    let form = Form::new()
        .text("provider_id", provider_id.to_string())
        .text("name", model_name.to_string())
        .text("display_name", display_name.to_string())
        .text(
            "description",
            format!("Test {} model uploaded via integration test", file_format),
        )
        .text("file_format", file_format.to_string())
        .text("main_filename", filename.to_string())
        .part("files", file_part);

    // Upload the model
    println!("Uploading {} model to server...", file_format);
    let response = reqwest::Client::new()
        .post(server.api_url("/llm-models/upload"))
        .header("Authorization", format!("Bearer {}", user_token))
        .multipart(form)
        .send()
        .await
        .unwrap();

    let status = response.status();
    println!("Upload response status: {}", status);

    if !status.is_success() {
        let error_body = response.text().await.unwrap();
        println!("Error response: {}", error_body);
        panic!("Upload failed with status {}", status);
    }

    assert_eq!(status, StatusCode::OK);

    let model: serde_json::Value = response.json().await.unwrap();
    println!(
        "Model created: {}",
        serde_json::to_string_pretty(&model).unwrap()
    );

    // Verify model was created correctly
    assert_eq!(model["name"].as_str().unwrap(), model_name);
    assert_eq!(model["display_name"].as_str().unwrap(), display_name);
    assert_eq!(model["file_format"].as_str().unwrap(), file_format);
    assert_eq!(model["provider_id"].as_str().unwrap(), provider_id);
    assert!(model["enabled"].as_bool().unwrap());

    // Verify we can retrieve the model
    let model_id = model["id"].as_str().unwrap();
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-models/{}", model_id)))
        .header("Authorization", format!("Bearer {}", user_token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let retrieved_model: serde_json::Value = response.json().await.unwrap();
    assert_eq!(retrieved_model["id"].as_str().unwrap(), model_id);
    assert_eq!(retrieved_model["name"].as_str().unwrap(), model_name);

    // Verify model appears in provider's models list
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-models?provider_id={}", provider_id)))
        .header("Authorization", format!("Bearer {}", user_token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let models_list: serde_json::Value = response.json().await.unwrap();
    let models = models_list["models"].as_array().unwrap();

    let found_model = models
        .iter()
        .find(|m| m["id"].as_str().unwrap() == model_id);

    assert!(
        found_model.is_some(),
        "Uploaded model should appear in provider's models list"
    );
    let found = found_model.unwrap();
    assert_eq!(found["name"].as_str().unwrap(), model_name);
    assert_eq!(found["display_name"].as_str().unwrap(), display_name);

    println!("✅ {} model upload test passed!", file_format);
}

#[tokio::test]
async fn test_upload_gguf_models() {
    // Setup test server and user with appropriate permissions
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "uploader",
        &[
            "llm_models::create",
            "llm_models::read",
            "llm_providers::read",
            "llm_providers::create",
        ],
    )
    .await;

    // Get or create local provider
    let provider = get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Test all GGUF models
    let gguf_models: Vec<_> = TEST_MODELS
        .iter()
        .filter(|(_, _, format, _)| *format == "gguf")
        .collect();

    for (idx, (repo_id, filename, file_format, display_name)) in gguf_models.iter().enumerate() {
        let model_name = format!("gguf-test-{}", idx);
        test_upload_model_helper(
            &server,
            &user.token,
            provider_id,
            repo_id,
            filename,
            file_format,
            display_name,
            &model_name,
        )
        .await;
    }
}

#[tokio::test]
async fn test_upload_safetensors_models() {
    // Setup test server and user with appropriate permissions
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "uploader",
        &[
            "llm_models::create",
            "llm_models::read",
            "llm_providers::read",
            "llm_providers::create",
        ],
    )
    .await;

    // Get or create local provider
    let provider = get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Test all SafeTensors models
    let safetensors_models: Vec<_> = TEST_MODELS
        .iter()
        .filter(|(_, _, format, _)| *format == "safetensors")
        .collect();

    for (idx, (repo_id, filename, file_format, display_name)) in
        safetensors_models.iter().enumerate()
    {
        let model_name = format!("safetensors-test-{}", idx);
        test_upload_model_helper(
            &server,
            &user.token,
            provider_id,
            repo_id,
            filename,
            file_format,
            display_name,
            &model_name,
        )
        .await;
    }
}

#[tokio::test]
async fn test_upload_pytorch_models() {
    // Setup test server and user with appropriate permissions
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "uploader",
        &[
            "llm_models::create",
            "llm_models::read",
            "llm_providers::read",
            "llm_providers::create",
        ],
    )
    .await;

    // Get or create local provider
    let provider = get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Test all PyTorch models
    let pytorch_models: Vec<_> = TEST_MODELS
        .iter()
        .filter(|(_, _, format, _)| *format == "pytorch")
        .collect();

    for (idx, (repo_id, filename, file_format, display_name)) in pytorch_models.iter().enumerate() {
        let model_name = format!("pytorch-test-{}", idx);
        test_upload_model_helper(
            &server,
            &user.token,
            provider_id,
            repo_id,
            filename,
            file_format,
            display_name,
            &model_name,
        )
        .await;
    }
}

#[tokio::test]
async fn test_upload_requires_create_permission() {
    let server = crate::common::TestServer::start().await;

    // User with only read permission
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "reader",
        &["llm_models::read", "llm_providers::read"],
    )
    .await;

    // Get provider
    let provider = get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Try to upload without create permission
    let dummy_data = b"dummy model data";
    let file_part = Part::bytes(dummy_data.to_vec())
        .file_name("test.gguf")
        .mime_str("application/octet-stream")
        .unwrap();

    let form = Form::new()
        .text("provider_id", provider_id.to_string())
        .text("name", "unauthorized-model")
        .text("display_name", "Unauthorized Model")
        .text("file_format", "gguf")
        .text("main_filename", "test.gguf")
        .part("files", file_part);

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-models/upload"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_upload_duplicate_name_fails() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "uploader",
        &[
            "llm_models::create",
            "llm_models::read",
            "llm_providers::read",
            "llm_providers::create",
        ],
    )
    .await;

    let provider = get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Upload first model. 07-llm-model F-09 (Medium) closure made
    // validate_file_content actually enforce: a weight file <1024 bytes
    // is rejected as "suspiciously small". Use 2 KiB of zero-padding
    // so the test exercises the duplicate-name path, not the
    // size-validation path.
    let dummy_data = vec![0u8; 2048];
    let dummy_data = dummy_data.as_slice();
    let file_part = Part::bytes(dummy_data.to_vec())
        .file_name("model1.gguf")
        .mime_str("application/octet-stream")
        .unwrap();

    let form = Form::new()
        .text("provider_id", provider_id.to_string())
        .text("name", "duplicate-test")
        .text("display_name", "First Model")
        .text("file_format", "gguf")
        .text("main_filename", "model1.gguf")
        .part("files", file_part);

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-models/upload"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Try to upload second model with same name
    let file_part2 = Part::bytes(dummy_data.to_vec())
        .file_name("model2.gguf")
        .mime_str("application/octet-stream")
        .unwrap();

    let form2 = Form::new()
        .text("provider_id", provider_id.to_string())
        .text("name", "duplicate-test") // Same name
        .text("display_name", "Second Model")
        .text("file_format", "gguf")
        .text("main_filename", "model2.gguf")
        .part("files", file_part2);

    let response2 = reqwest::Client::new()
        .post(server.api_url("/llm-models/upload"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form2)
        .send()
        .await
        .unwrap();

    // Should fail with bad request
    assert_eq!(response2.status(), StatusCode::BAD_REQUEST);

    let error: serde_json::Value = response2.json().await.unwrap();
    // The API returns error in the "error" field, not "message"
    let error_msg = error["error"].as_str().unwrap().to_lowercase();
    assert!(
        error_msg.contains("duplicate"),
        "Error message should contain 'duplicate', got: {}",
        error_msg
    );
    assert_eq!(error["error_code"].as_str().unwrap(), "DUPLICATE_ENTRY");
}

#[tokio::test]
async fn test_upload_missing_fields_fails() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "uploader",
        &[
            "llm_models::create",
            "llm_models::read",
            "llm_providers::read",
        ],
    )
    .await;

    let provider = get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Test missing name
    let dummy_data = b"test data";
    let file_part = Part::bytes(dummy_data.to_vec())
        .file_name("test.gguf")
        .mime_str("application/octet-stream")
        .unwrap();

    let form = Form::new()
        .text("provider_id", provider_id.to_string())
        // Missing name field
        .text("display_name", "Test Model")
        .text("file_format", "gguf")
        .text("main_filename", "test.gguf")
        .part("files", file_part);

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-models/upload"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// audit id all-07fc905ac56b — file-CONTENT validation on upload. The handler
// runs validate_file_content (uploads.rs:752) and refuses the upload with 400
// INVALID_MODEL_FILE when a file (a) is empty, or (b) looks like an HTML error
// page (a stand-in for a download that actually saved an HTTP error body as a
// ".gguf"). The existing tests cover only missing-field / duplicate-name, not
// the content checks. Driven through the real HTTP upload path.
async fn upload_single_file(
    server: &crate::common::TestServer,
    token: &str,
    provider_id: &str,
    filename: &str,
    bytes: Vec<u8>,
) -> reqwest::Response {
    let file_part = Part::bytes(bytes)
        .file_name(filename.to_string())
        .mime_str("application/octet-stream")
        .unwrap();
    let form = Form::new()
        .text("provider_id", provider_id.to_string())
        .text("name", format!("content-val-{filename}"))
        .text("display_name", "Content Validation")
        .text("file_format", "gguf")
        .text("main_filename", filename.to_string())
        .part("files", file_part);
    reqwest::Client::new()
        .post(server.api_url("/llm-models/upload"))
        .header("Authorization", format!("Bearer {token}"))
        .multipart(form)
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn test_upload_rejects_empty_and_html_content() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "uploader",
        &[
            "llm_models::create",
            "llm_models::read",
            "llm_providers::read",
            "llm_providers::create",
        ],
    )
    .await;
    let provider = get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // (a) Empty file → rejected.
    let empty = upload_single_file(&server, &user.token, provider_id, "empty.gguf", vec![]).await;
    assert_eq!(
        empty.status(),
        StatusCode::BAD_REQUEST,
        "an empty model file must be rejected"
    );
    let empty_body: serde_json::Value = empty.json().await.unwrap();
    assert_eq!(empty_body["error_code"].as_str().unwrap(), "INVALID_MODEL_FILE");
    assert!(
        empty_body["error"].as_str().unwrap().to_lowercase().contains("empty"),
        "empty-file error should mention 'empty': {empty_body}"
    );

    // (b) HTML error-page body saved as a .gguf → rejected. Pad past 1KB so the
    // "suspiciously small" weight check isn't what fires — the HTML sniff must.
    let mut html = b"<!DOCTYPE html><html><body>404 Not Found</body></html>".to_vec();
    html.resize(2048, b' ');
    let html_res =
        upload_single_file(&server, &user.token, provider_id, "model.gguf", html).await;
    assert_eq!(
        html_res.status(),
        StatusCode::BAD_REQUEST,
        "an HTML error-page masquerading as a weight file must be rejected"
    );
    let html_body: serde_json::Value = html_res.json().await.unwrap();
    assert_eq!(html_body["error_code"].as_str().unwrap(), "INVALID_MODEL_FILE");
    assert!(
        html_body["error"].as_str().unwrap().to_uppercase().contains("HTML"),
        "html-content error should mention HTML: {html_body}"
    );
}

/// Gap-A regression (the headline of the mistral.rs-parity hardening): a
/// SHARDED safetensors model with NO `*.index.json` must keep EVERY shard.
/// determine_files_to_copy -> model_files::select_download_files now grabs
/// the whole safetensors set regardless of an index. Synthetic shards, so no
/// network is needed and the test always runs.
#[tokio::test]
async fn test_upload_sharded_safetensors_without_index_keeps_all_shards() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "uploader",
        &[
            "llm_models::create",
            "llm_models::read",
            "llm_providers::read",
            "llm_providers::create",
        ],
    )
    .await;
    let provider = get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    let part = |name: &str, bytes: Vec<u8>| {
        Part::bytes(bytes)
            .file_name(name.to_string())
            .mime_str("application/octet-stream")
            .unwrap()
    };

    // Two distinct-size shards (>=1KB each to pass the weight-size check),
    // config + tokenizer, and crucially NO `*.index.json`.
    let form = Form::new()
        .text("provider_id", provider_id.to_string())
        .text("name", "sharded-noindex-test")
        .text("display_name", "Sharded (no index)")
        .text("file_format", "safetensors")
        .text("main_filename", "model-00001-of-00002.safetensors")
        .part("files", part("model-00001-of-00002.safetensors", vec![1u8; 2048]))
        .part("files", part("model-00002-of-00002.safetensors", vec![2u8; 3072]))
        .part("files", part("config.json", b"{}".to_vec()))
        .part("files", part("tokenizer.json", b"{}".to_vec()));

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-models/upload"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .unwrap();

    let status = response.status();
    let model: serde_json::Value = response.json().await.unwrap();
    assert_eq!(status, StatusCode::OK, "upload failed: {model}");
    let model_id = model["id"].as_str().unwrap();

    // Verify BOTH shards landed on disk. (`file_size_bytes` on the response is
    // None at create time — it's populated later by Tier-2 validation — so we
    // check the storage dir directly.) If only the single main shard were kept
    // (the OLD index-dependent behavior), shard 2 would be missing.
    let model_dir = crate::common::shared_test_app_data_dir()
        .join("models")
        .join(provider_id)
        .join(model_id);
    let shard_size = |name: &str| {
        std::fs::metadata(model_dir.join(name))
            .map(|m| m.len())
            .unwrap_or(0)
    };
    let s1 = shard_size("model-00001-of-00002.safetensors");
    let s2 = shard_size("model-00002-of-00002.safetensors");
    assert!(
        s1 >= 2048 && s2 >= 3072,
        "both shards must be copied (gap-A); shard1={s1} shard2={s2} dir={model_dir:?}"
    );
}

/// File-content validation rejects an EMPTY file and an HTML error-page blob
/// (magic-byte sniff) with 400 INVALID_MODEL_FILE — the previously-untested
/// content-validation branch of the upload handler (existing tests cover
/// missing-fields + duplicate-name only). No model download needed: the bytes
/// are synthetic and must be refused before any storage write.
#[tokio::test]
async fn test_upload_rejects_empty_and_html_content_v2() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "uploader_validation",
        &[
            "llm_models::create",
            "llm_models::read",
            "llm_providers::read",
            "llm_providers::create",
        ],
    )
    .await;
    let provider = get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap().to_string();

    let upload = |bytes: Vec<u8>, model_name: &str| {
        let provider_id = provider_id.clone();
        let token = user.token.clone();
        let url = server.api_url("/llm-models/upload");
        let model_name = model_name.to_string();
        async move {
            let part = Part::bytes(bytes)
                .file_name("model.gguf")
                .mime_str("application/octet-stream")
                .unwrap();
            let form = Form::new()
                .text("provider_id", provider_id)
                .text("name", model_name)
                .text("display_name", "Bad Upload")
                .text("description", "validation test")
                .text("file_format", "gguf")
                .text("main_filename", "model.gguf")
                .part("files", part);
            reqwest::Client::new()
                .post(&url)
                .header("Authorization", format!("Bearer {token}"))
                .multipart(form)
                .send()
                .await
                .unwrap()
        }
    };

    // Empty file → 400.
    let empty = upload(Vec::new(), "empty-model").await;
    assert_eq!(empty.status(), 400, "empty file must be rejected");
    let body = empty.text().await.unwrap();
    assert!(
        body.contains("INVALID_MODEL_FILE") || body.to_lowercase().contains("empty"),
        "empty-file rejection must name the validation failure: {body}"
    );

    // HTML error-page content (starts with `<htm`) → 400.
    let html = upload(b"<html><body>502 Bad Gateway</body></html>".to_vec(), "html-model").await;
    assert_eq!(html.status(), 400, "HTML content must be rejected");
    let body = html.text().await.unwrap();
    assert!(
        body.contains("INVALID_MODEL_FILE") || body.to_uppercase().contains("HTML"),
        "HTML rejection must name the validation failure: {body}"
    );
}

