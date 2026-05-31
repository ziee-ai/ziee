//! Tier 2 — idle eviction + drain coordination (spawns the stub-engine).
//!
//! Uses the debug-only `LLM_RUNTIME_REAPER_TICK_MS` override so the
//! reaper ticks in milliseconds instead of the production 60s. The
//! drain flag + in-flight counter live in-memory in the server, so
//! these behaviours are driven through real server state (a held
//! in-flight request), never seeded.

use crate::common::test_helpers::create_user_with_permissions;
use super::mock_release::{self, MockReleaseServer};
use super::test_helpers::{self as lrt, LOCAL_RUNTIME_ADMIN_PERMS};
use reqwest::StatusCode;
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;

async fn started_model(mock: &MockReleaseServer, admin_token: &str, name: &str) -> (Uuid, String) {
    let version_id = lrt::download_engine_from_mock(mock, admin_token, "llamacpp").await;
    let pool = lrt::test_pool(&mock.server).await;
    let (provider_id, proxy_token, _p) =
        lrt::create_local_provider_with_token(&mock.server, admin_token).await;
    let model_id = lrt::make_startable_model(
        &mock.server,
        admin_token,
        &pool,
        provider_id,
        name,
        version_id,
        "/tmp/ziee-reaper.gguf",
    )
    .await;
    let start = lrt::start_instance(&mock.server, admin_token, model_id).await;
    assert_eq!(start.status(), StatusCode::CREATED);
    (model_id, proxy_token)
}

/// idle_unload_secs=1 + fast reaper tick → an untouched engine is
/// drained and stopped within a couple seconds.
#[tokio::test]
async fn idle_eviction_stops_engine() {
    let mock = mock_release::setup_with_env(vec![(
        "LLM_RUNTIME_REAPER_TICK_MS".to_string(),
        "300".to_string(),
    )])
    .await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let r = lrt::update_runtime_settings(&mock.server, &admin.token, json!({ "idle_unload_secs": 1, "drain_timeout_secs": 2 })).await;
    assert_eq!(r.status(), StatusCode::OK);

    let (model_id, _tok) = started_model(&mock, &admin.token, "idle-model").await;

    // Wait through several idle ticks.
    tokio::time::sleep(Duration::from_secs(4)).await;

    let status = lrt::get_status(&mock.server, &admin.token, model_id).await;
    assert_ne!(
        status["status"].as_str(),
        Some("running"),
        "idle engine should have been evicted: {status}"
    );
}

/// idle_unload_secs=0 disables eviction — the engine stays running.
#[tokio::test]
async fn idle_eviction_disabled_when_zero() {
    let mock = mock_release::setup_with_env(vec![(
        "LLM_RUNTIME_REAPER_TICK_MS".to_string(),
        "300".to_string(),
    )])
    .await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let r = lrt::update_runtime_settings(&mock.server, &admin.token, json!({ "idle_unload_secs": 0 })).await;
    assert_eq!(r.status(), StatusCode::OK);

    let (model_id, _tok) = started_model(&mock, &admin.token, "noevict-model").await;
    tokio::time::sleep(Duration::from_secs(3)).await;

    let status = lrt::get_status(&mock.server, &admin.token, model_id).await;
    assert_eq!(status["status"].as_str(), Some("running"), "eviction disabled");
}

/// The reaper must DRAIN (wait for in-flight requests) rather than chop a
/// live stream: an engine that goes idle while a request is still in
/// flight is not stopped until that request finishes (or drain_timeout).
///
/// We assert the in-flight request completes with 200 — if the reaper had
/// killed the engine mid-flight, the proxy's upstream call would error.
/// This is the deterministic core of the drain guarantee. (The companion
/// "a NEW request during the drain window gets 503" depends on the
/// in-process Draining flag being observed within a sub-second window and
/// is too timing-sensitive to assert reliably from outside the process.)
#[tokio::test]
async fn inflight_request_survives_reaper_drain() {
    let mock = mock_release::setup_with_env(vec![(
        "LLM_RUNTIME_REAPER_TICK_MS".to_string(),
        "300".to_string(),
    )])
    .await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    // Idle quickly so the engine becomes evictable while the request below
    // is mid-flight; a long drain window so the reaper waits for it.
    let r = lrt::update_runtime_settings(&mock.server, &admin.token, json!({ "idle_unload_secs": 1, "drain_timeout_secs": 15 })).await;
    assert_eq!(r.status(), StatusCode::OK);

    // Auto-start via the proxy (no manual start to avoid a start/auto-start
    // race) and hold the request in-flight ~5s — well past the idle
    // threshold, so the reaper will try to evict while we're still
    // streaming.
    let version_id = lrt::download_engine_from_mock(&mock, &admin.token, "llamacpp").await;
    let pool = lrt::test_pool(&mock.server).await;
    let (provider_id, proxy_token, _p) =
        lrt::create_local_provider_with_token(&mock.server, &admin.token).await;
    lrt::make_startable_model(
        &mock.server, &admin.token, &pool, provider_id, "drain-model", version_id, "/tmp/ziee-drain.gguf",
    )
    .await;

    let resp = lrt::proxy_chat(
        &mock.server,
        &proxy_token,
        json!({ "model": "drain-model", "messages": [], "stub_hang_ms": 5000 }),
    )
    .await;

    // If the reaper had chopped the engine mid-flight, this would be a 502
    // (upstream error). A 200 proves drain waited for the in-flight request.
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "reaper must not chop an in-flight request"
    );
}
