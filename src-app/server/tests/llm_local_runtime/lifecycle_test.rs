//! Tier 2 — manual instance management (start/stop/restart/status +
//! provider instances + permission gating). Spawns the stub-engine.

use crate::common::test_helpers::{create_user_with_permissions, create_user_with_only_permissions};
use super::mock_release::{self, MockReleaseServer};
use super::test_helpers::{self as lrt, LOCAL_RUNTIME_ADMIN_PERMS};
use reqwest::StatusCode;
use uuid::Uuid;

/// Download+default the stub engine, create a startable model, return its id.
async fn startable(mock: &MockReleaseServer, admin_token: &str, name: &str) -> (Uuid, Uuid) {
    let version_id = lrt::download_engine_from_mock(mock, admin_token, "llamacpp").await;
    let pool = lrt::test_pool(&mock.server).await;
    let (provider_id, _proxy_token, _p) =
        lrt::create_local_provider_with_token(&mock.server, admin_token).await;
    let model_id = lrt::make_startable_model(
        &mock.server,
        admin_token,
        &pool,
        provider_id,
        name,
        version_id,
        "/tmp/ziee-lifecycle.gguf",
    )
    .await;
    (provider_id, model_id)
}

#[tokio::test]
async fn full_instance_lifecycle() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let (_provider_id, model_id) = startable(&mock, &admin.token, "lc-model").await;

    // start
    let start = lrt::start_instance(&mock.server, &admin.token, model_id).await;
    assert_eq!(start.status(), StatusCode::CREATED, "start should 201");

    // status → running
    let status = lrt::get_status(&mock.server, &admin.token, model_id).await;
    assert_eq!(status["status"].as_str(), Some("running"), "status after start");

    // restart
    let restart = lrt::restart_instance(&mock.server, &admin.token, model_id).await;
    assert_eq!(restart.status(), StatusCode::OK, "restart should 200");
    let status = lrt::get_status(&mock.server, &admin.token, model_id).await;
    assert_eq!(status["status"].as_str(), Some("running"), "status after restart");

    // stop
    let stop = lrt::stop_instance(&mock.server, &admin.token, model_id).await;
    assert_eq!(stop.status(), StatusCode::OK, "stop should 200");
    let status = lrt::get_status(&mock.server, &admin.token, model_id).await;
    assert_ne!(status["status"].as_str(), Some("running"), "stopped after stop");
}

/// Regression: starting a model that has a leftover non-running instance row
/// (left by a prior stop, or by validation's probe) must succeed, NOT 409
/// with "already running". Only a genuinely-running instance should block.
#[tokio::test]
async fn start_succeeds_after_stop() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let (_provider_id, model_id) = startable(&mock, &admin.token, "restart-after-stop").await;

    // start → stop leaves a status='stopped' instance row behind.
    assert_eq!(
        lrt::start_instance(&mock.server, &admin.token, model_id).await.status(),
        StatusCode::CREATED
    );
    assert_eq!(
        lrt::stop_instance(&mock.server, &admin.token, model_id).await.status(),
        StatusCode::OK
    );

    // Starting again must clear the stopped row and start fresh, not 409.
    let restart = lrt::start_instance(&mock.server, &admin.token, model_id).await;
    assert_eq!(
        restart.status(),
        StatusCode::CREATED,
        "start after stop must succeed, not 409 on the leftover stopped row"
    );
    let status = lrt::get_status(&mock.server, &admin.token, model_id).await;
    assert_eq!(status["status"].as_str(), Some("running"), "running after restart");
}

#[tokio::test]
async fn provider_instances_lists_running() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let (provider_id, model_id) = startable(&mock, &admin.token, "pi-model").await;

    lrt::start_instance(&mock.server, &admin.token, model_id).await;

    let resp = reqwest::Client::new()
        .get(mock.server.api_url(&format!("/local-runtime/providers/{provider_id}/instances")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    let instances = body["instances"].as_array().expect("instances array");
    assert!(
        instances.iter().any(|i| i["model_id"].as_str() == Some(model_id.to_string().as_str())),
        "started model should appear in provider instances: {body}"
    );
}

#[tokio::test]
async fn start_requires_manage_permission() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let (_provider_id, model_id) = startable(&mock, &admin.token, "perm-model").await;

    // A user with read but NOT manage.
    let reader =
        create_user_with_only_permissions(&mock.server, "reader", &["llm_local_runtime::read"]).await;
    let resp = lrt::start_instance(&mock.server, &reader.token, model_id).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "start needs manage");
}
