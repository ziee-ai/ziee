//! Tier 2 — crash-loop flap protection (P3 wiring of the health state
//! machine into the auto-start crash path).
//!
//! A model whose engine never becomes healthy is retried a few times
//! (each a 504 timeout), then the flap cap (5 crashes / 60s) trips and
//! the proxy fast-fails with 502 ("marked failed") instead of
//! re-spawning the engine on every request forever.

use crate::common::test_helpers::create_user_with_permissions;
use super::mock_release;
use super::test_helpers::{self as lrt, LOCAL_RUNTIME_ADMIN_PERMS};
use reqwest::StatusCode;
use serde_json::json;

#[tokio::test]
async fn crash_loop_trips_flap_cap_and_stops_respawning() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    // Short auto-start timeout so each failed attempt is ~1s, not 30s.
    let r = lrt::update_runtime_settings(&mock.server, &admin.token, json!({ "auto_start_timeout_secs": 1 })).await;
    assert_eq!(r.status(), StatusCode::OK);

    let version_id = lrt::download_engine_from_mock(&mock, &admin.token, "llamacpp").await;
    let pool = lrt::test_pool(&mock.server).await;
    let (provider_id, proxy_token, _p) =
        lrt::create_local_provider_with_token(&mock.server, &admin.token).await;
    // The `stub-unhealthy` sentinel in the model path makes /health 503
    // forever, so every auto-start attempt times out (counts as a crash).
    lrt::make_startable_model(
        &mock.server,
        &admin.token,
        &pool,
        provider_id,
        "flappy",
        version_id,
        "/tmp/ziee-stub-unhealthy-flappy.gguf",
    )
    .await;

    let mut saw_timeout = false;
    let mut saw_failed = false;
    for _ in 0..10 {
        let resp =
            lrt::proxy_chat(&mock.server, &proxy_token, json!({ "model": "flappy", "messages": [] })).await;
        match resp.status() {
            // Early attempts: engine never goes healthy → start timeout.
            StatusCode::GATEWAY_TIMEOUT => saw_timeout = true,
            // After the flap cap trips: fast-fail without re-spawning.
            StatusCode::BAD_GATEWAY => {
                saw_failed = true;
                break;
            }
            other => panic!("unexpected proxy status during crash loop: {other}"),
        }
    }

    assert!(saw_timeout, "early start attempts should time out (504)");
    assert!(
        saw_failed,
        "flap cap should eventually fast-fail with 502 instead of re-spawning forever"
    );
}
