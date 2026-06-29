use reqwest::StatusCode;
use serde_json::json;

/// Helper to get the Hugging Face repository from the database and optionally configure it with API key
pub async fn get_huggingface_repository(
    server: &crate::common::TestServer,
    token: &str,
    configure_api_key: bool,
) -> serde_json::Value {
    let response = reqwest::Client::new()
        .get(server.api_url("/llm-repositories"))
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

    // Return repository without configuring if not needed (e.g., for permission tests)
    if !configure_api_key {
        return hf_repo;
    }

    // Get API key from environment variable
    let api_key = std::env::var("HUGGINGFACE_API_KEY")
        .expect("HUGGINGFACE_API_KEY not set. Please source tests/.env.test or set the environment variable.");

    // Update the repository with the API key
    let repo_id = hf_repo["id"].as_str().unwrap();
    let update_payload = json!({
        "auth_config": {
            "api_key": api_key,
            "auth_test_api_endpoint": "https://huggingface.co/api/whoami-v2"
        }
    });

    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-repositories/{}", repo_id)))
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

/// Helper to get or create a local provider
pub async fn get_local_provider(
    server: &crate::common::TestServer,
    token: &str,
) -> serde_json::Value {
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

#[tokio::test]
async fn test_initiate_download_from_huggingface() {
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
        ],
    )
    .await;

    // Get Hugging Face repository and configure API key
    let hf_repo = get_huggingface_repository(&server, &user.token, true).await;
    let repo_id = hf_repo["id"].as_str().unwrap();

    // Get local provider
    let provider = get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Initiate download of a tiny test model
    let payload = json!({
        "provider_id": provider_id,
        "repository_id": repo_id,
        "repository_path": "hf-internal-testing/tiny-random-gpt2",
        "repository_branch": "main",
        "name": "tiny-gpt2-download-test",
        "display_name": "Tiny GPT-2 (Download Test)",
        "description": "Test model downloaded from Hugging Face",
        "file_format": "safetensors",
        "main_filename": "model.safetensors",
        "source": {
            "type": "hub",
            "id": "hf-internal-testing/tiny-random-gpt2"
        }
    });

    println!("Initiating download request...");
    let response = reqwest::Client::new()
        .post(server.api_url("/llm-models/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    let status = response.status();
    println!("Download initiation response status: {}", status);

    if !status.is_success() {
        let error_body = response.text().await.unwrap();
        println!("Error response: {}", error_body);
        panic!("Download initiation failed with status {}", status);
    }

    assert_eq!(status, StatusCode::OK);

    let download_instance: serde_json::Value = response.json().await.unwrap();
    println!(
        "Download instance created: {}",
        serde_json::to_string_pretty(&download_instance).unwrap()
    );

    // Verify download instance fields
    assert!(download_instance["id"].as_str().is_some());
    assert_eq!(
        download_instance["provider_id"].as_str().unwrap(),
        provider_id
    );
    assert_eq!(
        download_instance["repository_id"].as_str().unwrap(),
        repo_id
    );

    let request_data = &download_instance["request_data"];
    assert_eq!(
        request_data["model_name"].as_str().unwrap(),
        "tiny-gpt2-download-test"
    );
    assert_eq!(request_data["revision"].as_str().unwrap(), "main");
    assert_eq!(
        request_data["repository_path"].as_str().unwrap(),
        "hf-internal-testing/tiny-random-gpt2"
    );

    // Status should be pending initially
    let status_str = download_instance["status"].as_str().unwrap();
    assert!(
        status_str == "pending" || status_str == "in_progress" || status_str == "completed",
        "Expected status to be pending, in_progress, or completed, got: {}",
        status_str
    );

    // If download completed, verify model appears in provider's models list
    if status_str == "completed" {
        // Get the model ID from download instance result
        if let Some(model_id) = download_instance["result"]["model_id"].as_str() {
            println!("Download completed, verifying model appears in provider's models list...");

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
                .find(|m| m["id"].as_str().unwrap() == model_id);

            assert!(
                found_model.is_some(),
                "Downloaded model should appear in provider's models list"
            );
            let found = found_model.unwrap();
            assert_eq!(found["name"].as_str().unwrap(), "tiny-gpt2-download-test");

            println!("✅ Model appears in provider's models list");
        }
    }

    println!("✅ Download initiation test passed!");
}

#[tokio::test]
async fn test_download_requires_create_permission() {
    let server = crate::common::TestServer::start().await;

    // User with only read permission
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "reader",
        &[
            "llm_models::read",
            "llm_providers::read",
            "llm_repositories::read",
        ],
    )
    .await;

    // Get Hugging Face repository (without configuring API key, user doesn't have permission)
    let hf_repo = get_huggingface_repository(&server, &user.token, false).await;
    let repo_id = hf_repo["id"].as_str().unwrap();

    // Try to create a provider (should fail)
    let payload = json!({
        "name": "Test Provider",
        "provider_type": "local",
        "display_name": "Test Provider",
        "enabled": true
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // This should fail with forbidden
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // Try to initiate download without create permission
    let download_payload = json!({
        "provider_id": "00000000-0000-0000-0000-000000000000",
        "repository_id": repo_id,
        "repository_path": "hf-internal-testing/tiny-random-gpt2",
        "name": "unauthorized-download",
        "display_name": "Unauthorized Download",
        "file_format": "safetensors",
        "main_filename": "model.safetensors",
        "source": {
            "type": "hub",
            "id": "hf-internal-testing/tiny-random-gpt2"
        }
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-models/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&download_payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_download_missing_required_fields() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "downloader",
        &[
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_repositories::read",
        ],
    )
    .await;

    // Get Hugging Face repository (no API key needed for validation test)
    let hf_repo = get_huggingface_repository(&server, &user.token, false).await;
    let repo_id = hf_repo["id"].as_str().unwrap();

    // Get provider
    let provider = get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Test missing repository_path
    let payload = json!({
        "provider_id": provider_id,
        "repository_id": repo_id,
        // Missing repository_path
        "name": "test-model",
        "display_name": "Test Model",
        "file_format": "safetensors",
        "main_filename": "model.safetensors",
        "source": {
            "type": "hub",
            "id": "test/model"
        }
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-models/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // Axum's Json extractor returns 422 for JSON deserialization errors (missing required fields)
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_download_invalid_repository() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "downloader",
        &[
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_repositories::read",
        ],
    )
    .await;

    // Note: No need to get HF repository for this test since we're testing invalid repository ID

    // Get provider
    let provider = get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Use a non-existent repository ID
    let payload = json!({
        "provider_id": provider_id,
        "repository_id": "00000000-0000-0000-0000-000000000000",
        "repository_path": "test/model",
        "name": "test-model",
        "display_name": "Test Model",
        "file_format": "safetensors",
        "main_filename": "model.safetensors",
        "source": {
            "type": "hub",
            "id": "test/model"
        }
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-models/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // API returns 404 NOT_FOUND when repository doesn't exist
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_download_multiple_models() {
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
        ],
    )
    .await;

    // Get Hugging Face repository and configure API key
    let hf_repo = get_huggingface_repository(&server, &user.token, true).await;
    let repo_id = hf_repo["id"].as_str().unwrap();

    // Get local provider
    let provider = get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Test models to download
    let test_models = vec![
        (
            "hf-internal-testing/tiny-random-gpt2",
            "tiny-gpt2-multi-1",
            "Tiny GPT-2 Model 1",
        ),
        (
            "hf-internal-testing/tiny-random-bert",
            "tiny-bert-multi-1",
            "Tiny BERT Model 1",
        ),
    ];

    for (repo_path, name, display_name) in test_models {
        let payload = json!({
            "provider_id": provider_id,
            "repository_id": repo_id,
            "repository_path": repo_path,
            "name": name,
            "display_name": display_name,
            "file_format": "safetensors",
            "main_filename": "model.safetensors",
            "source": {
                "type": "hub",
                "id": repo_path
            }
        });

        println!("Initiating download for {}...", name);
        let response = reqwest::Client::new()
            .post(server.api_url("/llm-models/download"))
            .header("Authorization", format!("Bearer {}", user.token))
            .json(&payload)
            .send()
            .await
            .unwrap();

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap();
            println!("Error for {}: {}", name, error_body);
            panic!("Failed to initiate download for {}: {}", name, status);
        }

        assert_eq!(
            status,
            StatusCode::OK,
            "Failed to initiate download for {}",
            name
        );

        let download_instance: serde_json::Value = response.json().await.unwrap();
        assert!(download_instance["id"].as_str().is_some());
        println!("✅ Download initiated for {}", name);
    }
}

#[tokio::test]
async fn test_download_with_specific_branch() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "downloader",
        &[
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_repositories::read",
            "llm_repositories::edit",
        ],
    )
    .await;

    // Get Hugging Face repository and configure API key
    let hf_repo = get_huggingface_repository(&server, &user.token, true).await;
    let repo_id = hf_repo["id"].as_str().unwrap();

    // Get local provider
    let provider = get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    // Download with specific branch
    let payload = json!({
        "provider_id": provider_id,
        "repository_id": repo_id,
        "repository_path": "hf-internal-testing/tiny-random-gpt2",
        "repository_branch": "main",
        "name": "tiny-gpt2-main-branch",
        "display_name": "Tiny GPT-2 (Main Branch)",
        "file_format": "safetensors",
        "main_filename": "model.safetensors",
        "source": {
            "type": "hub",
            "id": "hf-internal-testing/tiny-random-gpt2"
        }
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-models/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let download_instance: serde_json::Value = response.json().await.unwrap();
    let request_data = &download_instance["request_data"];
    assert_eq!(request_data["revision"].as_str().unwrap(), "main");
}

/// Helper: fetch the hub models catalog and return the first auth_required model
/// whose source is Hugging Face (the bundled catalog ships several).
async fn first_auth_required_hf_model(
    server: &crate::common::TestServer,
    token: &str,
) -> serde_json::Value {
    let models: serde_json::Value = reqwest::Client::new()
        .get(server.api_url("/hub/models?lang=en"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    // Names that the bundled hub seed deliberately pins to a future ziee
    // version (`min_ziee_version: 99.0.0`) — they exist to exercise the
    // HUB_INCOMPATIBLE gate, but THIS test wants a "normal" model so it
    // can reach the disabled-repo / auth-not-configured gates the test
    // is actually about. The `/hub/models` response shape does not
    // surface `min_ziee_version`, so the skip list lives here as a
    // hard-coded fixture exclusion. Update when the seed changes.
    // Matches the reverse-DNS `name` field on the catalog item.
    const INCOMPATIBLE_FIXTURE_NAMES: &[&str] = &["io.github.phibya/deepseek-r1-70b"];

    // Auth + source-registry live on `sources[].environmentVariables[]`
    // (isRequired+isSecret) and `sources[].registryType` — not on
    // model-wide flat fields.
    models
        .as_array()
        .unwrap()
        .iter()
        .find(|m| {
            let name = m["name"].as_str().unwrap_or("");
            if INCOMPATIBLE_FIXTURE_NAMES.contains(&name) {
                return false;
            }
            let sources = match m.get("sources").and_then(|s| s.as_array()) {
                Some(s) => s,
                None => return false,
            };
            sources.iter().any(|src| {
                let is_hf = src.get("registryType").and_then(|v| v.as_str())
                    == Some("huggingface");
                let needs_auth = src
                    .get("environmentVariables")
                    .and_then(|e| e.as_array())
                    .map(|envs| {
                        envs.iter().any(|ev| {
                            ev.get("isRequired").and_then(|v| v.as_bool()) == Some(true)
                                && ev.get("isSecret").and_then(|v| v.as_bool()) == Some(true)
                        })
                    })
                    .unwrap_or(false);
                is_hf && needs_auth
            })
        })
        .cloned()
        .expect("bundled catalog should contain an HF model with a required+secret env var not in INCOMPATIBLE_FIXTURE_NAMES")
}

/// The hub download endpoint must BLOCK with a 422 + actionable guidance when the
/// model needs auth but its source repository has no credential configured —
/// instead of spawning a download that fails later with an opaque git auth error.
#[tokio::test]
async fn test_hub_download_blocked_when_repo_auth_not_configured() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_gate_blocked",
        &[
            "hub::models::download",
            "hub::models::read",
            "llm_models::create",
            "llm_models::downloads_read",
            "llm_providers::read",
            "llm_providers::create",
            "llm_repositories::read",
            "llm_repositories::edit",
        ],
    )
    .await;

    // Leave the Hugging Face repo credential EMPTY (the seeded default).
    let _hf = get_huggingface_repository(&server, &user.token, false).await;

    let provider = get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    let model = first_auth_required_hf_model(&server, &user.token).await;
    let hub_id = model["name"].as_str().unwrap();

    // The computed flag should agree that auth is NOT configured.
    assert_eq!(
        model["source_auth_configured"].as_bool(),
        Some(false),
        "source_auth_configured should be false while the HF repo has no key"
    );

    let response = reqwest::Client::new()
        .post(server.api_url("/hub/models/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "hub_id": hub_id, "provider_id": provider_id }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body["error_code"].as_str(),
        Some("HUB_REPOSITORY_AUTH_NOT_CONFIGURED"),
        "unexpected error body: {body}"
    );
    assert_eq!(
        body["details"]["settings_path"].as_str(),
        Some("/settings/llm-repositories")
    );

    // No download instance should have been created.
    let downloads: serde_json::Value = reqwest::Client::new()
        .get(server.api_url("/llm-models/downloads"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        downloads["total"].as_i64(),
        Some(0),
        "the blocked request must not create a download instance"
    );
}

/// REPOSITORY_DISABLED gate (sibling of the auth gate above). Verifies
/// that POST /hub/models/download bounces with 422 +
/// HUB_REPOSITORY_DISABLED when the source repository is disabled,
/// even if a credential is configured. Defense in depth: the
/// frontend's `ModelHubCard::handleDownload` flow gates this before
/// firing the request, but a direct API call (or a stale UI snapshot)
/// must also bounce cleanly.
#[tokio::test]
async fn test_hub_download_blocked_when_repo_is_disabled() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_gate_disabled",
        &[
            "hub::models::download",
            "hub::models::read",
            "llm_models::create",
            "llm_models::downloads_read",
            "llm_providers::read",
            "llm_providers::create",
            "llm_repositories::read",
            "llm_repositories::edit",
        ],
    )
    .await;

    let hf = get_huggingface_repository(&server, &user.token, false).await;
    let repo_id = hf["id"].as_str().unwrap();
    // Disable the repo via the public API (so the test exercises the
    // same wire path the UI uses — not a direct SQL UPDATE).
    let disable_resp = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-repositories/{}", repo_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(disable_resp.status(), StatusCode::OK);

    let provider = get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();
    // Any hub model that lives on the HF repo works — the disable
    // gate runs BEFORE the auth gate, so an auth-required model
    // still trips this first.
    let model = first_auth_required_hf_model(&server, &user.token).await;
    let hub_id = model["name"].as_str().unwrap();

    let response = reqwest::Client::new()
        .post(server.api_url("/hub/models/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "hub_id": hub_id, "provider_id": provider_id }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "disabled-repo download must 422",
    );
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body["error_code"].as_str(),
        Some("HUB_REPOSITORY_DISABLED"),
        "unexpected error body: {body}"
    );
    // The error details carry the repo id + settings path so the UI
    // can route the user to the right page if they hit this gate via
    // a direct API call.
    assert_eq!(
        body["details"]["repository_id"].as_str(),
        Some(repo_id),
        "details.repository_id should match the disabled repo: {body}"
    );
    assert_eq!(
        body["details"]["settings_path"].as_str(),
        Some("/settings/llm-repositories"),
    );

    // No download instance should have been created.
    let downloads: serde_json::Value = reqwest::Client::new()
        .get(server.api_url("/llm-models/downloads"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        downloads["total"].as_i64(),
        Some(0),
        "the disabled-repo request must not create a download instance"
    );
}

/// Once the source repository has a credential configured, the same hub download
/// passes the gate and creates a download instance. A dummy (non-empty) key is
/// enough to satisfy the presence gate — we don't await the background clone.
///
/// NOTE: this is a NETWORK-TOUCHING test — once the gate passes, the background
/// task makes a real (immediately-failing, dummy-token) git clone to
/// huggingface.co. The assertion reads only the synchronously-created download
/// row, so it is not flaky, but the test belongs to the network-dependent tier.
#[tokio::test]
async fn test_hub_download_proceeds_when_repo_auth_configured() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hub_gate_ok",
        &[
            "hub::models::download",
            "hub::models::read",
            "llm_models::create",
            "llm_models::downloads_read",
            "llm_providers::read",
            "llm_providers::create",
            "llm_repositories::read",
            "llm_repositories::edit",
        ],
    )
    .await;

    // Configure the Hugging Face repo with a DUMMY non-empty key.
    let hf = get_huggingface_repository(&server, &user.token, false).await;
    let hf_id = hf["id"].as_str().unwrap();
    let update = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-repositories/{}", hf_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "auth_config": {
                "api_key": "dummy-token-for-gate-test",
                "auth_test_api_endpoint": "https://huggingface.co/api/whoami-v2"
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(update.status(), StatusCode::OK);

    let provider = get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap();

    let model = first_auth_required_hf_model(&server, &user.token).await;
    let hub_id = model["name"].as_str().unwrap();
    // The flag now reports configured.
    assert_eq!(model["source_auth_configured"].as_bool(), Some(true));

    let response = reqwest::Client::new()
        .post(server.api_url("/hub/models/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "hub_id": hub_id, "provider_id": provider_id }))
        .send()
        .await
        .unwrap();

    assert!(
        response.status().is_success(),
        "gate should pass once a credential is configured, got {}",
        response.status()
    );

    // A download instance was created (it may immediately fail in the background
    // with the dummy key — we only assert the gate let it through).
    let downloads: serde_json::Value = reqwest::Client::new()
        .get(server.api_url("/llm-models/downloads"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        downloads["total"].as_i64().unwrap_or(0) >= 1,
        "a download instance should have been created"
    );
}

// audit id all-0d1edf0f339b — concurrent (not sequential) download initiation.
// uploads.rs spawns a background task per download; test_download_multiple_models
// initiates two SEQUENTIALLY. This fires two initiations CONCURRENTLY and asserts
// both succeed with DISTINCT download-instance ids — proving the per-download
// spawn path has no shared-state race on concurrent initiation. (Real-HF tier,
// like the sibling tests: needs HUGGINGFACE_API_KEY from tests/.env.test.)
#[tokio::test]
async fn test_concurrent_download_initiations_are_distinct() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "cc_downloader",
        &[
            "llm_models::create",
            "llm_models::read",
            "llm_providers::read",
            "llm_providers::create",
            "llm_repositories::read",
            "llm_repositories::edit",
        ],
    )
    .await;

    let hf_repo = get_huggingface_repository(&server, &user.token, true).await;
    let repo_id = hf_repo["id"].as_str().unwrap().to_string();
    let provider = get_local_provider(&server, &user.token).await;
    let provider_id = provider["id"].as_str().unwrap().to_string();

    let initiate = |repo_path: &'static str, name: &'static str| {
        let url = server.api_url("/llm-models/download");
        let token = user.token.clone();
        let body = json!({
            "provider_id": provider_id,
            "repository_id": repo_id,
            "repository_path": repo_path,
            "name": name,
            "display_name": name,
            "file_format": "safetensors",
            "main_filename": "model.safetensors",
            "source": { "type": "hub", "id": repo_path }
        });
        async move {
            reqwest::Client::new()
                .post(&url)
                .header("Authorization", format!("Bearer {token}"))
                .json(&body)
                .send()
                .await
                .unwrap()
        }
    };

    // Fire BOTH initiations concurrently.
    let (r1, r2) = tokio::join!(
        initiate("hf-internal-testing/tiny-random-gpt2", "cc-dl-gpt2"),
        initiate("hf-internal-testing/tiny-random-bert", "cc-dl-bert"),
    );
    assert_eq!(r1.status(), StatusCode::OK, "first concurrent initiation");
    assert_eq!(r2.status(), StatusCode::OK, "second concurrent initiation");
    let id1 = r1.json::<serde_json::Value>().await.unwrap()["id"].as_str().unwrap().to_string();
    let id2 = r2.json::<serde_json::Value>().await.unwrap()["id"].as_str().unwrap().to_string();
    assert_ne!(id1, id2, "concurrent initiations must yield distinct download instances");
}

// audit id all-00827f174278 — download CANCELLATION exercised on the REAL path
// (the E2E spec deliberately mocks the HF download because a real ~350MB
// download raced flakily). Here we initiate a REAL HuggingFace download and
// immediately hit the REAL cancel endpoint: cancel must either succeed
// (200 → status cancelled) while the download is still pending/downloading, or
// be cleanly rejected (4xx) if the tiny model already finished — never a 500.
// Env-keyed like the sibling real-HF tests (source tests/.env.test).
#[tokio::test]
async fn test_cancel_real_download() {
    let _api_key = std::env::var("HUGGINGFACE_API_KEY")
        .expect("HUGGINGFACE_API_KEY not set. Please source tests/.env.test or set the environment variable.");
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "cancel_downloader",
        &[
            "llm_models::create",
            "llm_models::read",
            "llm_models::downloads_read",
            "llm_providers::read",
            "llm_providers::create",
            "llm_repositories::read",
            "llm_repositories::edit",
        ],
    )
    .await;
    let hf_repo = get_huggingface_repository(&server, &user.token, true).await;
    let provider = get_local_provider(&server, &user.token).await;

    // Initiate a real download.
    let init = reqwest::Client::new()
        .post(server.api_url("/llm-models/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "provider_id": provider["id"].as_str().unwrap(),
            "repository_id": hf_repo["id"].as_str().unwrap(),
            "repository_path": "hf-internal-testing/tiny-random-gpt2",
            "repository_branch": "main",
            "name": "cancel-real-test",
            "display_name": "Cancel Real Test",
            "file_format": "safetensors",
            "main_filename": "model.safetensors",
            "source": { "type": "hub", "id": "hf-internal-testing/tiny-random-gpt2" }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(init.status(), StatusCode::OK, "initiate should 200");
    let download_id = init.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Immediately cancel the REAL download.
    let cancel = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-models/downloads/{download_id}/cancel")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    let cancel_status = cancel.status();
    assert!(
        cancel_status.is_success() || cancel_status.is_client_error(),
        "cancel of a real download must be 2xx (cancelled in-flight) or 4xx (already terminal), never 5xx; got {cancel_status}"
    );

    // If the cancel was accepted, the download must be reported cancelled.
    if cancel_status.is_success() {
        let list: serde_json::Value = reqwest::Client::new()
            .get(server.api_url("/llm-models/downloads"))
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let row = list["downloads"]
            .as_array()
            .and_then(|a| a.iter().find(|d| d["id"].as_str() == Some(download_id.as_str())));
        if let Some(r) = row {
            assert_eq!(
                r["status"].as_str(),
                Some("cancelled"),
                "an accepted cancel must leave the download in 'cancelled' status: {r}"
            );
        }
    }
}

/// Concurrency-safety: the partial unique index `uq_download_instances_in_progress`
/// (migration 119) is the TOCTOU guard for two simultaneous identical downloads.
/// Two concurrent requests can both pass the handler's find-existing check and
/// both attempt an insert; the index makes the loser fail at the DB level so the
/// handler can return the in-flight winner instead of spawning a duplicate task.
/// This exercises that guard directly (no network): a second in-progress row for
/// the same (repo, provider, path, filename) must be rejected, while a fresh
/// in-progress insert is allowed once the prior row reaches a terminal status.
#[tokio::test]
async fn concurrent_identical_downloads_dedup_via_unique_index() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "dl_dedup",
        &[
            "llm_models::create",
            "llm_models::read",
            "llm_providers::read",
            "llm_providers::create",
            "llm_repositories::read",
            "llm_repositories::edit",
        ],
    )
    .await;

    let hf_repo = get_huggingface_repository(&server, &user.token, false).await;
    let repo_id =
        uuid::Uuid::parse_str(hf_repo["id"].as_str().unwrap()).unwrap();
    let provider = get_local_provider(&server, &user.token).await;
    let provider_id =
        uuid::Uuid::parse_str(provider["id"].as_str().unwrap()).unwrap();

    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let request_data = serde_json::json!({
        "repository_path": "org/model",
        "main_filename": "model.safetensors"
    });

    let insert_in_progress = |status: &'static str| {
        let pool = pool.clone();
        let rd = request_data.clone();
        async move {
            sqlx::query(
                r#"INSERT INTO download_instances
                       (provider_id, repository_id, request_data, status)
                   VALUES ($1, $2, $3, $4)"#,
            )
            .bind(provider_id)
            .bind(repo_id)
            .bind(&rd)
            .bind(status)
            .execute(&pool)
            .await
        }
    };

    // First in-progress download row inserts cleanly.
    insert_in_progress("downloading")
        .await
        .expect("first in-progress download should insert");

    // A second identical in-progress row violates the partial unique index —
    // this is exactly what makes the concurrent-duplicate insert lose the race.
    let dup = insert_in_progress("pending").await;
    let err = dup.expect_err("second identical in-progress download must be rejected");
    match err {
        sqlx::Error::Database(db) => assert!(
            db.is_unique_violation(),
            "expected a unique-violation, got: {db}"
        ),
        other => panic!("expected a database unique violation, got: {other}"),
    }

    // Once the in-flight row reaches a terminal status, the partial index no
    // longer covers it, so a brand-new download of the same file is allowed.
    sqlx::query("UPDATE download_instances SET status = 'completed' WHERE provider_id = $1")
        .bind(provider_id)
        .execute(&pool)
        .await
        .unwrap();
    insert_in_progress("downloading")
        .await
        .expect("a fresh download is allowed once the prior one is terminal");

    pool.close().await;
}

