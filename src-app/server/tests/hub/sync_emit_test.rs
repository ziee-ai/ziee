//! Realtime-sync emission coverage for the `hub_settings` singleton entity.
//!
//! `hub_settings` is permission-scoped: a catalog mutation fans out only to
//! connections whose snapshot satisfies `hub::catalog::read` (admins always
//! qualify — see `modules/sync/event.rs::audience_kind`). This asserts, over
//! the REAL path (handler → publish → registry → SSE), that activating a
//! catalog version emits a `hub_settings`/`update` frame to a
//! `hub::catalog::read` holder and that a user lacking it stays silent.
//!
//! Trigger choice — cheapest real path:
//!   * Both `activate_hub_version` (POST /hub/activate) and
//!     `refresh_hub_catalog` (POST /hub/refresh) emit `hub_settings`/`update`,
//!     but each first calls `HubManager::refresh()`, which fetches + verifies
//!     a catalog bundle from GitHub. The `sync_publish` runs only AFTER that
//!     `?`-propagated fetch succeeds, so a network/cosign failure aborts the
//!     handler before any event.
//!   * The hermetic `mock_release_server` fixture (already used by
//!     `catalog_hermetic.rs`) serves a tiny tar.gz catalog over loopback with
//!     cosign skipped (`ZIEE_HUB_ALLOW_UNSIGNED=1`), so `/hub/activate`
//!     returns 200 against it with NO real download. We reuse that exact
//!     fixture + the activate flow `catalog_hermetic::activate_then_switch...`
//!     already proves returns 200.
//! The settings row is a singleton, so the wire id is the nil UUID — assert
//! entity + action only.

use std::time::Duration;

use serde_json::json;

use super::mock_release_server::{spawn_mock_hub, MockItem, MockVersion};
use crate::common::TestServer;
use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::create_user_with_permissions;

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);
const SILENCE_WINDOW: Duration = Duration::from_secs(1);

/// A single cheap mock catalog version (one model item).
fn one_version() -> Vec<MockVersion> {
    vec![MockVersion {
        version: "9.9.1-test",
        prerelease: true,
        items: vec![MockItem {
            category: "model",
            id: "mock-model-a",
            min_ziee_version: None,
            extra_yaml: None,
            mcp_http: false,
        }],
    }]
}

/// Activating a catalog version emits `hub_settings`/`update` to a
/// `hub::catalog::read` holder; a user lacking that perm is silent.
#[tokio::test]
async fn activate_delivers_hub_settings_update_other_user_silent() {
    let mock = spawn_mock_hub(one_version()).await;
    let server = TestServer::start_with_options(crate::common::TestServerOptions {
        extra_env: mock.test_env(),
        ..Default::default()
    })
    .await;

    // Actor: audience perm (`hub::catalog::read`) + the manage perm
    // (`hub::catalog::manage`, required by the activate handler).
    let admin = create_user_with_permissions(
        &server,
        "sync_hub_admin",
        &["hub::catalog::read", "hub::catalog::manage"],
    )
    .await;
    // Outsider: only the baseline default group — it grants `profile::read`
    // (enough to subscribe) but NOT `hub::catalog::read`, so the
    // permission-scoped frame must never reach them.
    let outsider = create_user_with_permissions(&server, "sync_hub_outsider", &[]).await;

    let mut admin_probe = SyncProbe::open(&server, &admin.token).await;
    let mut outsider_probe = SyncProbe::open(&server, &outsider.token).await;

    let resp = reqwest::Client::new()
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": "9.9.1-test" }))
        .send()
        .await
        .expect("activate request");
    assert_eq!(
        resp.status(),
        200,
        "activate (unsigned mock) should 200: {}",
        resp.text().await.unwrap_or_default()
    );

    // Singleton → nil UUID id; assert entity + action only.
    admin_probe
        .expect_event("hub_settings", "update", EVENT_TIMEOUT)
        .await;

    outsider_probe.expect_silence(SILENCE_WINDOW).await;
}
