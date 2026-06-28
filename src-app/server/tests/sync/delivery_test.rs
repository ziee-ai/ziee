//! Core realtime-sync delivery over the REAL path: a real REST mutation in
//! one connection produces a real `sync` frame on a subscribed stream. These
//! assert the mechanism end-to-end (handler → publish → registry → SSE) and
//! the two cross-cutting guarantees — cross-user isolation and origin (self-
//! echo) suppression — using `memory` as an owner-scoped vehicle. Per-entity
//! coverage lives in each owning module's own integration tests.

use std::time::Duration;

use serde_json::{Value, json};

use crate::common::sync_probe::SyncProbe;

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);
const SILENCE_WINDOW: Duration = Duration::from_secs(1);

/// POST /memories as `token`, returning the new memory id.
async fn create_memory(
    server: &crate::common::TestServer,
    token: &str,
    content: &str,
    origin_conn: Option<&str>,
) -> String {
    let mut req = reqwest::Client::new()
        .post(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "content": content, "kind": "preference" }));
    if let Some(conn) = origin_conn {
        req = req.header("X-Sync-Connection-Id", conn);
    }
    let res = req.send().await.unwrap();
    assert_eq!(res.status(), 201, "memory create should return 201");
    let row: Value = res.json().await.unwrap();
    row["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn owner_mutation_is_delivered_to_owner_not_to_other_users() {
    let server = crate::common::TestServer::start().await;
    let alice = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_owner_alice",
        &["memory::read", "memory::write"],
    )
    .await;
    // Bob holds only the baseline (default group → profile::read); enough to
    // subscribe, but he must NEVER see Alice's owner-scoped event.
    let bob =
        crate::common::test_helpers::create_user_with_permissions(&server, "sync_owner_bob", &[])
            .await;

    let mut alice_probe = SyncProbe::open(&server, &alice.token).await;
    let mut bob_probe = SyncProbe::open(&server, &bob.token).await;

    let id = create_memory(&server, &alice.token, "alice prefers vim", None).await;

    let frame = alice_probe
        .expect_event("memory", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(frame.id, id, "the frame must carry the new memory's id");

    bob_probe.expect_silence(SILENCE_WINDOW).await;
}

#[tokio::test]
async fn create_update_delete_each_deliver_to_the_owner() {
    let server = crate::common::TestServer::start().await;
    let alice = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_cud_alice",
        &["memory::read", "memory::write"],
    )
    .await;
    let mut probe = SyncProbe::open(&server, &alice.token).await;
    let client = reqwest::Client::new();

    let id = create_memory(&server, &alice.token, "first", None).await;
    let created = probe.expect_event("memory", "create", EVENT_TIMEOUT).await;
    assert_eq!(created.id, id);

    client
        .patch(server.api_url(&format!("/memories/{id}")))
        .header("Authorization", format!("Bearer {}", alice.token))
        .json(&json!({ "content": "second" }))
        .send()
        .await
        .unwrap();
    let updated = probe.expect_event("memory", "update", EVENT_TIMEOUT).await;
    assert_eq!(updated.id, id);

    client
        .delete(server.api_url(&format!("/memories/{id}")))
        .header("Authorization", format!("Bearer {}", alice.token))
        .send()
        .await
        .unwrap();
    let deleted = probe.expect_event("memory", "delete", EVENT_TIMEOUT).await;
    assert_eq!(deleted.id, id);
}

#[tokio::test]
async fn originating_connection_is_skipped_but_the_users_other_tab_updates() {
    let server = crate::common::TestServer::start().await;
    let alice = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_origin_alice",
        &["memory::read", "memory::write"],
    )
    .await;

    // Two tabs for the SAME user.
    let mut origin_tab = SyncProbe::open(&server, &alice.token).await;
    let mut other_tab = SyncProbe::open(&server, &alice.token).await;

    // The mutation originates on `origin_tab` (echo its connection id back).
    let conn = origin_tab.connection_id().to_string();
    let _id = create_memory(&server, &alice.token, "from tab 1", Some(&conn)).await;

    // The originating tab is suppressed; the other tab still updates live.
    other_tab.expect_event("memory", "create", EVENT_TIMEOUT).await;
    origin_tab.expect_silence(SILENCE_WINDOW).await;
}

// ── Mid-stream deactivation tears down the sync stream (gap c71588ff52a2) ────

/// The subscribe loop's periodic re-check (handlers.rs:111-131) must tear down
/// an OPEN stream once the account is deactivated mid-stream. Uses the
/// debug-only `SYNC_RECHECK_TICK_MS` seam to shorten the 60s cadence so the
/// teardown is observable in the test window.
#[tokio::test]
async fn deactivating_a_user_mid_stream_closes_their_sync_stream() {
    let server = crate::common::TestServer::start_with_options(
        crate::common::TestServerOptions {
            extra_env: vec![("SYNC_RECHECK_TICK_MS".to_string(), "200".to_string())],
            ..Default::default()
        },
    )
    .await;

    // Admin who can deactivate; victim is a plain user (baseline profile::read).
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_deact_admin",
        &["users::create", "users::toggle_status"],
    )
    .await;
    let victim =
        crate::common::test_helpers::create_user_with_permissions(&server, "sync_deact_victim", &[])
            .await;

    // Victim opens a sync stream.
    let mut victim_probe = SyncProbe::open(&server, &victim.token).await;

    // Admin deactivates the victim mid-stream.
    let res = reqwest::Client::new()
        .post(server.api_url(&format!("/users/{}/toggle-active", victim.user_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("toggle-active");
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["is_active"], false, "victim must be deactivated");

    // Within a few re-check ticks the server tears the victim's stream down.
    victim_probe.expect_closed(Duration::from_secs(5)).await;
}
