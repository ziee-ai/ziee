//! Tier 2 — proves a model's nested `engine_settings` actually reach the
//! spawned engine's argv (regression guard for the unified settings
//! pipeline, which previously declared ~25/~37 fields in the API but
//! dropped all but a handful before spawn). The stub-engine echoes its
//! received argv to stdout; we read it back via the `/logs` snapshot.

use super::mock_release::{self, MockReleaseServer};
use super::test_helpers::{self as lrt, LOCAL_RUNTIME_ADMIN_PERMS};
use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;
use reqwest::StatusCode;
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;

/// Join the `/logs` snapshot into one string for substring assertions.
async fn read_logs(server: &TestServer, token: &str, model_id: Uuid) -> String {
    let body: serde_json::Value = reqwest::Client::new()
        .get(server.api_url(&format!("/local-runtime/models/{model_id}/logs")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("get logs")
        .json()
        .await
        .expect("logs json");
    body["logs"]
        .as_array()
        .expect("logs array")
        .iter()
        .filter_map(|l| l.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Start a llamacpp model carrying the given nested `engine_settings` and
/// return the captured argv line the stub echoed.
async fn argv_for_settings(
    mock: &MockReleaseServer,
    admin_token: &str,
    name: &str,
    settings: serde_json::Value,
) -> String {
    let version_id = lrt::download_engine_from_mock(mock, admin_token, "llamacpp").await;
    let pool = lrt::test_pool(&mock.server).await;
    let (provider_id, _t, _p) =
        lrt::create_local_provider_with_token(&mock.server, admin_token).await;
    let model_id = lrt::make_startable_model_with_settings(
        &mock.server,
        admin_token,
        &pool,
        provider_id,
        name,
        version_id,
        &format!("/tmp/ziee-args-{name}.gguf"),
        settings,
    )
    .await;

    let start = lrt::start_instance(&mock.server, admin_token, model_id).await;
    assert_eq!(start.status(), StatusCode::CREATED, "start should 201");

    // Let the capture loop drain the stub's stdout into the log ring.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let logs = read_logs(&mock.server, admin_token, model_id).await;
    let _ = lrt::stop_instance(&mock.server, admin_token, model_id).await;

    logs.lines()
        .find(|l| l.contains("argv:"))
        .unwrap_or_else(|| panic!("no argv line in stub logs:\n{logs}"))
        .to_string()
}

#[tokio::test]
async fn llamacpp_engine_settings_reach_argv() {
    let mock = mock_release::setup().await;
    let admin =
        create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;

    let argv = argv_for_settings(
        &mock,
        &admin.token,
        "lc-args",
        json!({ "llamacpp": {
            "ctx_size": 4096,
            "ubatch_size": 128,
            "flash_attn": true,
            "no_mmap": true,
            "cache_type_k": "q8_0",
            "n_gpu_layers": 0
        }}),
    )
    .await;

    // The newly-wired fields must appear in the real spawned argv.
    assert!(argv.contains("--ctx-size 4096"), "ctx-size missing: {argv}");
    assert!(argv.contains("--ubatch-size 128"), "ubatch-size missing: {argv}");
    assert!(argv.contains("--flash-attn on"), "flash-attn on missing: {argv}");
    assert!(argv.contains("--no-mmap"), "no-mmap missing: {argv}");
    assert!(argv.contains("--cache-type-k q8_0"), "cache-type-k missing: {argv}");
    // Hardening is always forced.
    assert!(argv.contains("--host 127.0.0.1"), "loopback host missing: {argv}");
    assert!(argv.contains("--api-key"), "api-key missing: {argv}");
}
