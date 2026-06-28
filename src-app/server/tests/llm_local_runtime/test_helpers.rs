/// Test Helpers for LLM Local Runtime Integration Tests
///
/// Provides reusable helper functions for setting up binaries, providers, and models
/// for integration tests that require real infrastructure.

use crate::common::TestServer;
use reqwest::StatusCode;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::path::PathBuf;
use uuid::Uuid;

use super::mock_release::MockReleaseServer;

/// Full admin permission set for the local-runtime + model surface.
/// Use with `create_user_with_permissions` for happy-path tests; use
/// `create_user_with_only_permissions` with a narrower slice for the
/// 403 permission-gating tests.
pub const LOCAL_RUNTIME_ADMIN_PERMS: &[&str] = &[
    "llm_local_runtime::read",
    "llm_local_runtime::manage",
    "llm_local_runtime::logs",
    "llm_local_runtime::versions_read",
    "llm_local_runtime::create",
    "llm_local_runtime::update",
    "llm_local_runtime::delete",
    "llm_local_runtime::settings_read",
    "llm_local_runtime::settings_manage",
    "llm_providers::create",
    "llm_providers::read",
    "llm_providers::edit",
    "llm_models::create",
    "llm_models::read",
    "llm_models::edit",
    "llm_models::delete",
    "llm_models::downloads_read",
    "llm_repositories::read",
    "llm_repositories::edit",
];

/// Open a pool against the per-test database. Tests use this to seed
/// rows (model files, instance state) and to assert on persisted state
/// directly — the server's in-memory caches (token cache, in-flight
/// counters, drain flags) are NOT reachable from the test process, so
/// anything the proxy reads from memory must be driven through the real
/// server behaviour, not seeded here.
pub async fn test_pool(server: &TestServer) -> PgPool {
    PgPoolOptions::new()
        .max_connections(4)
        .connect(&server.database_url)
        .await
        .expect("connect to test database")
}

/// Create a local provider and return `(provider_id, proxy_token,
/// provider_json)`. The proxy token is the one-time `plaintext_api_key`
/// from the create response — the ONLY way a test can authenticate to
/// the same-port proxy (the in-memory token cache can't be read from
/// the test process).
pub async fn create_local_provider_with_token(
    server: &TestServer,
    token: &str,
) -> (Uuid, String, serde_json::Value) {
    let name = format!("local-{}", &Uuid::new_v4().to_string()[..8]);
    let response = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({ "name": name, "provider_type": "local", "enabled": true }))
        .send()
        .await
        .expect("create local provider");

    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("provider create body");
    assert_eq!(
        status,
        StatusCode::CREATED,
        "provider create failed: {body}"
    );

    // CreateLlmProviderResponse `#[serde(flatten)]`s the provider, so its
    // fields (id, name, provider_type, …) sit at the TOP LEVEL alongside
    // plaintext_api_key — there is no nested "provider" object.
    let provider_id = Uuid::parse_str(body["id"].as_str().expect("provider id"))
        .expect("provider id uuid");
    let proxy_token = body["plaintext_api_key"]
        .as_str()
        .expect("local provider create must return plaintext_api_key")
        .to_string();

    (provider_id, proxy_token, body)
}

/// PUT the runtime-settings singleton. Pass only the fields to change;
/// each is an `UpdateRuntimeSettingsRequest` Option.
pub async fn update_runtime_settings(
    server: &TestServer,
    token: &str,
    patch: serde_json::Value,
) -> reqwest::Response {
    reqwest::Client::new()
        .put(server.api_url("/local-runtime/settings"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&patch)
        .send()
        .await
        .expect("PUT runtime settings")
}

/// Poll a download task's snapshot endpoint until it reaches a
/// terminal status (Completed or Failed). Returns the
/// `result_version_id` on Completed; panics with the snapshot body
/// on Failed or on timeout. The POST endpoint is fire-and-forget
/// now (detached, page-reload-safe), so tests that need the
/// resulting version row poll via this helper.
pub async fn wait_for_download(
    server: &TestServer,
    token: &str,
    key: &str,
) -> Uuid {
    use std::time::{Duration, Instant};
    let deadline = Instant::now() + Duration::from_secs(300); // 5 min cap for real-network tests
    let client = reqwest::Client::new();
    loop {
        let resp = client
            .get(server.api_url(&format!("/local-runtime/versions/downloads/{key}")))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .expect("snapshot fetch");
        assert_eq!(resp.status(), StatusCode::OK, "snapshot endpoint must exist");
        let snap: serde_json::Value = resp.json().await.expect("snapshot body");
        match snap["status"].as_str() {
            Some("completed") => {
                let id = snap["result_version_id"].as_str().unwrap_or_else(|| {
                    panic!("completed snapshot missing result_version_id: {snap}")
                });
                return Uuid::parse_str(id).expect("uuid");
            }
            Some("failed") => panic!("download failed: {snap}"),
            _ => {}
        }
        if Instant::now() > deadline {
            panic!("wait_for_download timeout for {key}: last snapshot {snap}");
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Download (register) an engine version from the mock release
/// server. Returns the new runtime_version_id. The
/// `allow_unsigned_downloads` opt-in has been removed — downloads now
/// proceed unconditionally and the mock can serve an unsigned
/// artifact without any setup PUT. The POST is detached, so we poll
/// the snapshot endpoint until terminal.
pub async fn download_engine_from_mock(
    mock: &MockReleaseServer,
    token: &str,
    engine: &str,
) -> Uuid {
    let payload = json!({
        "engine": engine,
        "version": mock.version,
        "platform": mock.platform,
        "arch": mock.arch,
        "backend": "cpu",
    });
    let response = reqwest::Client::new()
        .post(mock.server.api_url("/local-runtime/versions/download"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .expect("download engine from mock");

    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("download body");
    assert_eq!(status, StatusCode::OK, "engine download failed: {body}");
    let key = body["key"].as_str().expect("key in response").to_string();
    let version_id = wait_for_download(&mock.server, token, &key).await;

    // `LocalDeployment::start` resolves the engine binary via the SYSTEM
    // DEFAULT for the engine (not the model's required_runtime_version_id),
    // so a freshly-downloaded version must be made default before any
    // instance can spawn it.
    let set_default = reqwest::Client::new()
        .post(
            mock.server
                .api_url(&format!("/local-runtime/versions/{version_id}/set-default")),
        )
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("set system default");
    assert_eq!(
        set_default.status(),
        StatusCode::OK,
        "set-default after mock download"
    );

    version_id
}

/// Download a real engine binary from the published `ziee-ai` fork
/// release (hits real github.com — NO mock mirror), extracts it via
/// the production path, registers it, and makes it the system
/// default. Returns the runtime_version_id. The
/// `allow_unsigned_downloads` opt-in has been removed; the runtime
/// always accepts unverified downloads (cosign verify is logged but
/// no longer blocks).
pub async fn download_engine_release(
    server: &TestServer,
    token: &str,
    engine: &str,
    version: &str,
) -> Uuid {
    let platform = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x86_64" };
    let backend = if cfg!(target_os = "macos") { "metal" } else { "cpu" };

    let payload = json!({
        "engine": engine,
        "version": version,
        "platform": platform,
        "arch": arch,
        "backend": backend,
    });
    let response = reqwest::Client::new()
        .post(server.api_url("/local-runtime/versions/download"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&payload)
        .send()
        .await
        .expect("download engine from release");
    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("download body");
    assert_eq!(status, StatusCode::OK, "engine release download failed: {body}");
    let key = body["key"].as_str().expect("key in response").to_string();
    let version_id = wait_for_download(server, token, &key).await;

    let set_default = reqwest::Client::new()
        .post(server.api_url(&format!("/local-runtime/versions/{version_id}/set-default")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("set system default");
    assert_eq!(set_default.status(), StatusCode::OK, "set-default after release download");

    version_id
}

/// Create a local model row (no files yet). Returns the model_id.
///
/// `runtime_version_id` is accepted for caller readability but NOT sent:
/// `CreateLlmModelRequest` has no such field (and `deny_unknown_fields`),
/// and `LocalDeployment::start` resolves the engine binary from the
/// SYSTEM DEFAULT version, not a per-model pin.
pub async fn create_local_model(
    server: &TestServer,
    token: &str,
    provider_id: Uuid,
    name: &str,
    engine_type: &str,
    runtime_version_id: Option<Uuid>,
) -> Uuid {
    // Nested per-engine shape (`ModelEngineSettings`), the single source of
    // truth the spawn path now reads.
    let settings = if engine_type == "llamacpp" {
        json!({ "llamacpp": { "ctx_size": 512, "n_gpu_layers": 0 } })
    } else {
        json!({ "mistralrs": { "max_seqs": 16 } })
    };
    create_local_model_with_settings(
        server,
        token,
        provider_id,
        name,
        engine_type,
        runtime_version_id,
        settings,
    )
    .await
}

/// Like [`create_local_model`] but with caller-supplied nested
/// `engine_settings` (`{ "llamacpp": { … } }` / `{ "mistralrs": { … } }`).
pub async fn create_local_model_with_settings(
    server: &TestServer,
    token: &str,
    provider_id: Uuid,
    name: &str,
    engine_type: &str,
    _runtime_version_id: Option<Uuid>,
    engine_settings: serde_json::Value,
) -> Uuid {
    let payload = json!({
        "provider_id": provider_id.to_string(),
        "name": name,
        "display_name": format!("Test Model {name}"),
        "engine_type": engine_type,
        "engine_settings": engine_settings,
        "file_format": if engine_type == "llamacpp" { "gguf" } else { "safetensors" },
        "enabled": true,
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .expect("create model");
    let status = response.status();
    let text = response.text().await.expect("model body text");
    assert_eq!(status, StatusCode::CREATED, "model create failed ({status}): {text}");
    let body: serde_json::Value = serde_json::from_str(&text).expect("model json");
    Uuid::parse_str(body["id"].as_str().expect("model id")).expect("model id uuid")
}

/// Insert a completed `llm_model_files` row so `resolve_model_inputs`
/// can find a path. The file at `file_path` need not exist on disk for
/// the stub-engine (it ignores `--model`); the row just has to resolve.
/// Use a `file_path` containing the substring `stub-unhealthy` to make
/// the spawned stub report `/health` 503 forever (drives the 504 test).
pub async fn seed_model_file(pool: &PgPool, model_id: Uuid, file_path: &str) {
    let filename = std::path::Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("model.gguf")
        .to_string();
    sqlx::query(
        "INSERT INTO llm_model_files
            (model_id, filename, file_path, file_size_bytes, file_type, upload_status)
         VALUES ($1, $2, $3, 1, 'gguf', 'completed')
         ON CONFLICT (model_id, filename) DO UPDATE SET file_path = EXCLUDED.file_path",
    )
    .bind(model_id)
    .bind(&filename)
    .bind(file_path)
    .execute(pool)
    .await
    .expect("seed llm_model_files row");
}

/// Force a model's validation_status to `valid` so the proxy will
/// forward to it (the proxy rejects `failed` / `invalid`). Sidesteps the
/// async validation queue for tests that aren't exercising validation.
pub async fn mark_model_valid(pool: &PgPool, model_id: Uuid) {
    sqlx::query("UPDATE llm_models SET validation_status = 'valid' WHERE id = $1")
        .bind(model_id)
        .execute(pool)
        .await
        .expect("mark model valid");
}

/// Create a fully startable llamacpp model: row + seeded `.gguf` file +
/// validation_status=valid. Returns the model_id. The stub-engine will
/// be spawned with `--model <gguf_path>` (and ignore it).
pub async fn make_startable_model(
    server: &TestServer,
    token: &str,
    pool: &PgPool,
    provider_id: Uuid,
    name: &str,
    runtime_version_id: Uuid,
    gguf_path: &str,
) -> Uuid {
    let model_id =
        create_local_model(server, token, provider_id, name, "llamacpp", Some(runtime_version_id))
            .await;
    seed_model_file(pool, model_id, gguf_path).await;
    mark_model_valid(pool, model_id).await;
    model_id
}

/// Like [`make_startable_model`] but with caller-supplied nested
/// `engine_settings`, so a test can assert specific flags reach the argv.
#[allow(clippy::too_many_arguments)]
pub async fn make_startable_model_with_settings(
    server: &TestServer,
    token: &str,
    pool: &PgPool,
    provider_id: Uuid,
    name: &str,
    runtime_version_id: Uuid,
    gguf_path: &str,
    engine_settings: serde_json::Value,
) -> Uuid {
    let model_id = create_local_model_with_settings(
        server,
        token,
        provider_id,
        name,
        "llamacpp",
        Some(runtime_version_id),
        engine_settings,
    )
    .await;
    seed_model_file(pool, model_id, gguf_path).await;
    mark_model_valid(pool, model_id).await;
    model_id
}

// ── instance lifecycle wrappers ─────────────────────────────────────────

pub async fn start_instance(server: &TestServer, token: &str, model_id: Uuid) -> reqwest::Response {
    reqwest::Client::new()
        .post(server.api_url(&format!("/local-runtime/models/{model_id}/start")))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({}))
        .send()
        .await
        .expect("start instance")
}

pub async fn stop_instance(server: &TestServer, token: &str, model_id: Uuid) -> reqwest::Response {
    reqwest::Client::new()
        .post(server.api_url(&format!("/local-runtime/models/{model_id}/stop")))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("stop instance")
}

pub async fn restart_instance(server: &TestServer, token: &str, model_id: Uuid) -> reqwest::Response {
    reqwest::Client::new()
        .post(server.api_url(&format!("/local-runtime/models/{model_id}/restart")))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("restart instance")
}

pub async fn get_status(server: &TestServer, token: &str, model_id: Uuid) -> serde_json::Value {
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/local-runtime/models/{model_id}/status")))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("get status");
    assert_eq!(resp.status(), StatusCode::OK, "status endpoint");
    resp.json().await.expect("status body")
}

// ── proxy front-door wrappers ───────────────────────────────────────────

/// POST /api/local-llm/v1/chat/completions with a bearer + JSON body.
pub async fn proxy_chat(
    server: &TestServer,
    bearer: &str,
    body: serde_json::Value,
) -> reqwest::Response {
    reqwest::Client::new()
        .post(server.api_url("/local-llm/v1/chat/completions"))
        .header("Authorization", format!("Bearer {bearer}"))
        .json(&body)
        .send()
        .await
        .expect("proxy chat")
}

/// POST /api/local-llm/v1/embeddings with a bearer + JSON body.
pub async fn proxy_embeddings(
    server: &TestServer,
    bearer: &str,
    body: serde_json::Value,
) -> reqwest::Response {
    reqwest::Client::new()
        .post(server.api_url("/local-llm/v1/embeddings"))
        .header("Authorization", format!("Bearer {bearer}"))
        .json(&body)
        .send()
        .await
        .expect("proxy embeddings")
}

// ── GGUF fixtures ───────────────────────────────────────────────────────

/// Minimal valid GGUF v3 header: architecture=llama, context_length=4096.
/// Matches the synthetic bytes the metadata parser accepts, so capability
/// extraction succeeds on it.
pub fn tiny_gguf_bytes() -> Vec<u8> {
    const GGUF_TYPE_U32: u32 = 4;
    const GGUF_TYPE_STRING: u32 = 8;
    let mut buf = Vec::new();
    buf.extend_from_slice(b"GGUF");
    buf.extend_from_slice(&3u32.to_le_bytes()); // version
    buf.extend_from_slice(&0u64.to_le_bytes()); // tensor_count
    buf.extend_from_slice(&2u64.to_le_bytes()); // kv_count

    let key1 = b"general.architecture";
    buf.extend_from_slice(&(key1.len() as u64).to_le_bytes());
    buf.extend_from_slice(key1);
    buf.extend_from_slice(&GGUF_TYPE_STRING.to_le_bytes());
    let val1 = b"llama";
    buf.extend_from_slice(&(val1.len() as u64).to_le_bytes());
    buf.extend_from_slice(val1);

    let key2 = b"llama.context_length";
    buf.extend_from_slice(&(key2.len() as u64).to_le_bytes());
    buf.extend_from_slice(key2);
    buf.extend_from_slice(&GGUF_TYPE_U32.to_le_bytes());
    buf.extend_from_slice(&4096u32.to_le_bytes());

    buf
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
        .post(server.api_url("/llm-providers"))
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

    // POST /llm-providers returns `CreateLlmProviderResponse { provider,
    // plaintext_api_key }`. Callers that only want the provider object
    // (and read `["id"]` directly) get the inner object; the fallback
    // keeps this robust if the response were ever unwrapped.
    let body: serde_json::Value = response.json().await.unwrap();
    body.get("provider").cloned().unwrap_or(body)
}

/// Gets or creates a local provider
///
/// # Arguments
/// * `server` - Test server instance
/// * `token` - Authentication token
///
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
        "engine_settings": { "llamacpp": { "ctx_size": 2048, "n_gpu_layers": 0 } },
        "enabled": true,
    });

    let model = run_model_download(server, token, payload).await;
    (model, PathBuf::new())
}

/// POST a model-download request and poll until it completes; returns the
/// committed model JSON. Panics on failure/timeout.
async fn run_model_download(
    server: &TestServer,
    token: &str,
    payload: serde_json::Value,
) -> serde_json::Value {
    let response = reqwest::Client::new()
        .post(server.api_url("/llm-models/download"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to start model download");

    // Model download returns 200 OK with a download instance, not 201.
    if response.status() != StatusCode::OK && response.status() != StatusCode::CREATED {
        let status = response.status();
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "Could not read response body".to_string());
        panic!("Failed to initiate model download. Status: {status}, Body: {error_body}");
    }

    let download_instance: serde_json::Value = response.json().await.unwrap();
    let download_id = Uuid::parse_str(download_instance["id"].as_str().unwrap()).unwrap();
    println!("Model download initiated. Download ID: {download_id}");

    // Poll status (large GGUFs can take minutes).
    let max_wait_seconds = 600;
    let poll_interval = 5;
    let max_attempts = max_wait_seconds / poll_interval;

    for attempt in 0..max_attempts {
        tokio::time::sleep(tokio::time::Duration::from_secs(poll_interval)).await;

        let status_response = reqwest::Client::new()
            .get(server.api_url(&format!("/llm-models/downloads/{download_id}")))
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .unwrap();

        if !status_response.status().is_success() {
            continue;
        }

        let download_data: serde_json::Value = status_response.json().await.unwrap();
        let status = download_data["status"].as_str().unwrap_or("Unknown");
        println!("Download status: {status} (attempt {}/{max_attempts})", attempt + 1);

        if status == "completed" {
            let model_id = Uuid::parse_str(
                download_data["model_id"]
                    .as_str()
                    .expect("Download instance should have model_id when completed"),
            )
            .expect("Invalid model_id in download instance");

            let model_response = reqwest::Client::new()
                .get(server.api_url(&format!("/llm-models/{model_id}")))
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await
                .unwrap();
            println!("Model download completed! Model ID: {model_id}");
            return model_response.json().await.unwrap();
        } else if status == "failed" {
            panic!("Model download failed: {download_data:?}");
        }
    }

    panic!("Model download timed out after {max_wait_seconds} seconds");
}

/// Download a small REAL chat GGUF (TinyLlama-1.1B-Chat Q4_K_M, ~670 MB)
/// from HuggingFace into `provider_id`, returning the committed model JSON.
/// Needs `HUGGINGFACE_API_KEY`. Used by the real-engine end-to-end test.
pub async fn download_test_gguf_model(
    server: &TestServer,
    token: &str,
    provider_id: Uuid,
) -> serde_json::Value {
    let repository = get_huggingface_repository(server, token).await;
    let repository_id = Uuid::parse_str(repository["id"].as_str().unwrap()).unwrap();

    let repo = "TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF";
    let filename = "tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf";
    let payload = json!({
        "provider_id": provider_id.to_string(),
        "repository_id": repository_id.to_string(),
        "repository_path": repo,
        "repository_branch": "main",
        "name": "tinyllama-gguf",
        "display_name": "TinyLlama 1.1B Chat (GGUF)",
        "file_format": "gguf",
        "main_filename": filename,
        "source": { "type": "hub", "id": repo },
        "engine_type": "llamacpp",
        // A richer (but long-stable, value-form) flag set so the gold smoke
        // proves the REAL llama-server accepts a multi-flag argv built from
        // the unified engine_settings — not just ctx_size.
        "engine_settings": { "llamacpp": {
            "ctx_size": 2048,
            "n_gpu_layers": 0,
            "ubatch_size": 256,
            "parallel": 2,
            "seed": 42
        } },
        "enabled": true,
    });

    run_model_download(server, token, payload).await
}
