//! Hermetic hub catalog tests — no network, no real cosign.
//!
//! Uses the in-test `mock_release_server` + the debug-only fetch
//! overrides so the full activate → fetch → sha256 → unpack → rotate
//! path is exercised against a local server. Replaces the
//! network-dependent assertions the plan flagged.

use serde_json::{json, Value as Json};

use super::mock_release_server::{spawn_mock_hub, MockItem, MockVersion};
use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;

fn two_versions() -> Vec<MockVersion> {
    vec![
        // Newest-first (GitHub order). v9.9.2 adds an incompatible
        // assistant (min_ziee_version 99.0.0).
        MockVersion {
            version: "9.9.2-test",
            prerelease: true,
            items: vec![
                MockItem { category: "model", id: "mock-model-a", min_ziee_version: None },
                MockItem { category: "assistant", id: "mock-asst-a", min_ziee_version: None },
                MockItem {
                    category: "assistant",
                    id: "mock-asst-future",
                    min_ziee_version: Some("99.0.0"),
                },
            ],
        },
        MockVersion {
            version: "9.9.1-test",
            prerelease: true,
            items: vec![
                MockItem { category: "model", id: "mock-model-a", min_ziee_version: None },
                MockItem { category: "assistant", id: "mock-asst-a", min_ziee_version: None },
            ],
        },
    ]
}

#[tokio::test]
async fn activate_then_switch_versions_against_mock() {
    let mock = spawn_mock_hub(two_versions()).await;
    let server = TestServer::start_with_options(crate::common::TestServerOptions {
        extra_env: mock.test_env(),
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(
        &server,
        "admin",
        &["hub::catalog::read", "hub::catalog::manage", "hub::models::read"],
    )
    .await;
    let client = reqwest::Client::new();

    // Activate the older mock version (2 items).
    let resp = client
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": "9.9.1-test" }))
        .send()
        .await
        .expect("activate 9.9.1");
    assert_eq!(
        resp.status(),
        200,
        "activate 9.9.1-test (unsigned mock) should succeed: {}",
        resp.text().await.unwrap_or_default()
    );

    let idx: Json = client
        .get(server.api_url("/hub/index"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("index")
        .json()
        .await
        .expect("parse index");
    assert_eq!(idx["hub_version"], "9.9.1-test");
    assert_eq!(idx["items"].as_array().map(|a| a.len()), Some(2));

    // /version reports source=github (a fetch replaced the seed) +
    // cosign was skipped (unsigned mock).
    let ver: Json = client
        .get(server.api_url("/hub/version"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("version")
        .json()
        .await
        .expect("parse version");
    assert_eq!(ver["source"], "github");
    assert_eq!(ver["hub_version"], "9.9.1-test");

    // Switch to the newer version (3 items).
    let resp = client
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": "9.9.2-test" }))
        .send()
        .await
        .expect("activate 9.9.2");
    assert_eq!(resp.status(), 200);
    let idx: Json = client
        .get(server.api_url("/hub/index"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("index 2")
        .json()
        .await
        .expect("parse index 2");
    assert_eq!(idx["hub_version"], "9.9.2-test");
    assert_eq!(idx["items"].as_array().map(|a| a.len()), Some(3));
}

#[tokio::test]
async fn install_rejects_incompatible_item() {
    let mock = spawn_mock_hub(two_versions()).await;
    let server = TestServer::start_with_options(crate::common::TestServerOptions {
        extra_env: mock.test_env(),
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(
        &server,
        "admin",
        &["hub::catalog::read", "hub::catalog::manage", "hub::assistants::create"],
    )
    .await;
    let client = reqwest::Client::new();

    // Activate v9.9.2 which contains the future-pinned assistant.
    let resp = client
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": "9.9.2-test" }))
        .send()
        .await
        .expect("activate");
    assert_eq!(resp.status(), 200);

    // A compatible assistant installs fine (201).
    let ok = client
        .post(server.api_url("/hub/assistants/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-asst-a" }))
        .send()
        .await
        .expect("create compatible");
    assert_eq!(
        ok.status(),
        201,
        "compatible assistant should install: {}",
        ok.text().await.unwrap_or_default()
    );

    // The future-pinned assistant is rejected by the compat gate (422),
    // even though the API client bypasses the UI hiding.
    let blocked = client
        .post(server.api_url("/hub/assistants/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-asst-future" }))
        .send()
        .await
        .expect("create incompatible");
    assert_eq!(
        blocked.status(),
        422,
        "incompatible assistant should be 422'd, got {}",
        blocked.status()
    );
    let body: Json = blocked.json().await.expect("parse 422 body");
    assert_eq!(body["error_code"], "HUB_INCOMPATIBLE");
}

#[tokio::test]
async fn install_stamps_current_version_so_updates_stays_empty() {
    // Directly exercises the hub_version write-back: installing from the
    // active catalog must stamp hub_entities.hub_version with the current
    // version, so /hub/updates does NOT immediately flag the fresh install
    // as "behind". (Regression guard for the bug where every install was
    // recorded with NULL and showed as needing an update.)
    let mock = spawn_mock_hub(two_versions()).await;
    let server = TestServer::start_with_options(crate::common::TestServerOptions {
        extra_env: mock.test_env(),
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(
        &server,
        "admin",
        &["hub::catalog::read", "hub::catalog::manage", "hub::assistants::create"],
    )
    .await;
    let client = reqwest::Client::new();

    // Activate v9.9.1-test, then install a compatible assistant from it.
    client
        .post(server.api_url("/hub/activate"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": "9.9.1-test" }))
        .send()
        .await
        .expect("activate")
        .error_for_status()
        .expect("activate ok");
    client
        .post(server.api_url("/hub/assistants/create"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "hub_id": "mock-asst-a" }))
        .send()
        .await
        .expect("install")
        .error_for_status()
        .expect("install ok");

    // The fresh install was stamped 9.9.1-test == current → not behind.
    let updates: Json = client
        .get(server.api_url("/hub/updates"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("updates")
        .json()
        .await
        .expect("parse updates");
    let rows = updates["updates"].as_array().expect("updates array");
    assert!(
        rows.iter().all(|r| r["hub_id"] != "mock-asst-a"),
        "freshly-installed item must NOT appear in updates: {updates}"
    );
}
