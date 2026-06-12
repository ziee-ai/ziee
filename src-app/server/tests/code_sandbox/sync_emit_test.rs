//! Realtime-sync emission for the `code_sandbox_settings` singleton entity.
//!
//! The sandbox resource-limits surface is permission-scoped: a mutation fans
//! out only to connections whose snapshot satisfies
//! `code_sandbox::resource_limits::read` (admins always qualify). This asserts,
//! over the REAL path (handler → publish → registry → SSE), that an admin
//! updating the singleton produces a `code_sandbox_settings`/`update` frame,
//! and that a user lacking the read perm never observes it.
//!
//! NOTE on "sandbox disabled": code_sandbox is disabled in the test config,
//! but the resource-limits row is just a DB row — the PUT
//! `/code-sandbox/resource-limits` succeeds regardless (see
//! `tier3_resource_limits.rs`, which is NOT `#[ignore]`d and runs against the
//! same disabled-sandbox config). So this test exercises the real PUT path
//! without needing the sandbox enabled.

use std::time::Duration;

use serde_json::json;

use crate::common::sync_probe::SyncProbe;
use crate::common::{test_helpers, TestServer};

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);
const SILENCE_WINDOW: Duration = Duration::from_secs(1);

#[tokio::test]
async fn admin_update_delivers_code_sandbox_settings_event_other_user_silent() {
    let server = TestServer::start().await;
    // Actor holds the endpoint's manage perm + the audience read perm.
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "sync_cs_admin",
        &[
            "code_sandbox::resource_limits::read",
            "code_sandbox::resource_limits::manage",
        ],
    )
    .await;
    // Bob holds only the baseline (default group → profile::read); enough to
    // subscribe, but he lacks `code_sandbox::resource_limits::read` so he must
    // stay silent.
    let bob =
        test_helpers::create_user_with_permissions(&server, "sync_cs_bob", &[]).await;

    let mut admin_probe = SyncProbe::open(&server, &admin.token).await;
    let mut bob_probe = SyncProbe::open(&server, &bob.token).await;

    let resp = reqwest::Client::new()
        .put(server.api_url("/code-sandbox/resource-limits"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "pids_max": 128 }))
        .send()
        .await
        .expect("resource-limits PUT failed");
    assert_eq!(
        resp.status().as_u16(),
        200,
        "resource-limits PUT should return 200, got {:?}",
        resp.text().await
    );

    // Singleton → nil UUID id; assert entity + action only.
    admin_probe
        .expect_event("code_sandbox_settings", "update", EVENT_TIMEOUT)
        .await;

    bob_probe.expect_silence(SILENCE_WINDOW).await;
}
