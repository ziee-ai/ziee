//! Phase 8 H wave 2 — skill access isolation + entry_point safety.
//!
//! - H1: cross-user list isolation — user B's GET /skills omits user A's
//!   user-scope rows.
//! - SEC-2: a hub manifest carrying a traversal `entry_point`
//!   (`../../../etc/passwd`) is REJECTED at install time, before any
//!   join/read.

use serde_json::Value as Json;

use super::{
    FIXTURE_REFERENCE_MD, FIXTURE_SKILL_MD, install_fixture_skill, refresh_catalog,
    server_with_skill_catalog,
};
use crate::common::TestServer;
use crate::common::test_helpers::{TestUser, create_user_with_permissions};
use crate::hub::mock_release_server::{MockItem, MockVersion, spawn_mock_hub};

async fn skill_user(server: &TestServer, name: &str) -> TestUser {
    create_user_with_permissions(
        server,
        name,
        &[
            "hub::catalog::read",
            "hub::catalog::manage",
            "skills::read",
            "skills::install",
            "skills::manage",
        ],
    )
    .await
}

#[tokio::test]
async fn cross_user_skill_list_isolation() {
    // H1: a user-scope skill installed by A must not surface in B's list.
    let (server, _mock) = server_with_skill_catalog().await;
    let user_a = skill_user(&server, "skill_iso_a").await;
    let user_b = skill_user(&server, "skill_iso_b").await;
    refresh_catalog(&server, &user_a.token).await;

    let body = install_fixture_skill(&server, &user_a.token).await;
    let a_skill_id = body["skill"]["id"].clone();

    let list_b: Json = reqwest::Client::new()
        .get(server.api_url("/skills"))
        .header("Authorization", format!("Bearer {}", user_b.token))
        .send()
        .await
        .expect("list b")
        .json()
        .await
        .expect("parse");
    let leaked = list_b["skills"]
        .as_array()
        .expect("skills array")
        .iter()
        .any(|s| s["id"] == a_skill_id);
    assert!(
        !leaked,
        "user B must NOT see user A's user-scope skill: {list_b}"
    );

    // Sanity: A sees their own.
    let list_a: Json = reqwest::Client::new()
        .get(server.api_url("/skills"))
        .header("Authorization", format!("Bearer {}", user_a.token))
        .send()
        .await
        .expect("list a")
        .json()
        .await
        .expect("parse");
    assert!(
        list_a["skills"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["id"] == a_skill_id),
        "user A sees their own skill"
    );
}

#[tokio::test]
async fn install_rejects_traversal_entry_point() {
    // SEC-2: a manifest with a traversal entry_point is rejected at
    // install time (before the entry_point is joined to extracted_path
    // and read). The bundle bytes themselves are well-formed.
    const NAME: &str = "io.github.test/evil-entry";
    let mock = spawn_mock_hub(vec![MockVersion {
        version: "9.9.1-test",
        prerelease: true,
        items: vec![MockItem {
            category: "skill",
            name: NAME,
            min_ziee_version: None,
            extra_json: None,
            mcp_http: false,
            bundle_files: Some(vec![
                ("SKILL.md", FIXTURE_SKILL_MD),
                ("references/x.md", FIXTURE_REFERENCE_MD),
            ]),
            // Attacker-controlled traversal entry_point.
            bundle_entry_point: Some("../../../etc/passwd"),
        }],
    }])
    .await;
    let server = TestServer::start_with_options(crate::common::TestServerOptions {
        extra_env: mock.test_env(),
        ..Default::default()
    })
    .await;
    let user = skill_user(&server, "skill_sec2").await;
    refresh_catalog(&server, &user.token).await;

    let resp = reqwest::Client::new()
        .post(server.api_url("/skills/install-from-hub"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({ "hub_id": NAME }))
        .send()
        .await
        .expect("install evil-entry");
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    assert!(
        status.is_client_error() || status.is_server_error(),
        "traversal entry_point install must fail; got {status}: {text}"
    );
    assert!(
        text.contains("entry_point") || text.to_lowercase().contains("unsafe"),
        "rejection should cite entry_point safety (SEC-2); got: {text}"
    );

    // And no skill row was created.
    let list: Json = reqwest::Client::new()
        .get(server.api_url("/skills"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("list")
        .json()
        .await
        .expect("parse");
    assert!(
        !list["skills"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["name"] == NAME),
        "no skill row should exist for the rejected install: {list}"
    );
}
