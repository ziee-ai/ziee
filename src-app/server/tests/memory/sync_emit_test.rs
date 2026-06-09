// ============================================================================
// Realtime-sync emission for memory's two singleton-settings entities.
//
// `tests/sync/delivery_test.rs` already proves the end-to-end mechanism and
// the cross-cutting guarantees (isolation + self-echo suppression) using the
// owner-scoped `memory` entity. This file instead pins, per-entity, that the
// REST mutation for each SETTINGS endpoint emits the right `(entity, action)`
// to the right AUDIENCE:
//
//   - PUT /memory/settings        → memory_settings / update      (OWNER)
//   - PUT /memory/admin-settings  → memory_admin_settings / update (PERMISSION
//                                    "memory::admin::read"; id is the nil UUID)
//
// Endpoints, JSON bodies and required permissions are copied verbatim from
// the existing memory tests (retention_test.rs / onboarding_settings_init_test.rs).
// ============================================================================

use std::time::Duration;

use serde_json::json;

use crate::common::sync_probe::SyncProbe;

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);
const SILENCE_WINDOW: Duration = Duration::from_secs(1);

// ── memory_settings (OWNER-scoped) ──────────────────────────────────────────
//
// The per-user memory settings update is owner-scoped: only the user who owns
// the settings row receives the event. A different baseline user must stay
// silent.
#[tokio::test]
async fn memory_settings_update_delivers_to_owner_not_other_users() {
    let server = crate::common::TestServer::start().await;
    let alice = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_memset_alice",
        &["memory::read", "memory::write"],
    )
    .await;
    // Bob holds only the baseline (default group → profile::read); enough to
    // subscribe, but he must NEVER see Alice's owner-scoped settings event.
    let bob = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_memset_bob",
        &[],
    )
    .await;

    let mut alice_probe = SyncProbe::open(&server, &alice.token).await;
    let mut bob_probe = SyncProbe::open(&server, &bob.token).await;

    // Mutate as Alice WITHOUT echoing a connection id, so her own subscription
    // receives the event (no self-echo suppression).
    let res = reqwest::Client::new()
        .put(server.api_url("/memory/settings"))
        .header("Authorization", format!("Bearer {}", alice.token))
        .json(&json!({ "max_memories": 10 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "memory settings update should return 200");

    alice_probe
        .expect_event("memory_settings", "update", EVENT_TIMEOUT)
        .await;

    bob_probe.expect_silence(SILENCE_WINDOW).await;
}

// ── memory_admin_settings (PERMISSION-scoped: "memory::admin::read") ─────────
//
// The admin memory-settings update fans out to every holder of
// `memory::admin::read`. The actor holds the manage perm (to perform the PUT)
// plus the read perm (to receive the event); a baseline user lacking
// `memory::admin::read` must stay silent. The entity is a singleton, so its id
// is the nil UUID — we assert only entity + action, not a specific id.
#[tokio::test]
async fn memory_admin_settings_update_delivers_to_admin_read_holders_only() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_memadmin_actor",
        &["memory::admin::read", "memory::admin::manage"],
    )
    .await;
    // Bystander holds only the baseline — no memory::admin::read, so the
    // admin-settings event must not reach them.
    let bystander = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_memadmin_bystander",
        &[],
    )
    .await;

    let mut admin_probe = SyncProbe::open(&server, &admin.token).await;
    let mut bystander_probe = SyncProbe::open(&server, &bystander.token).await;

    // Mutate as the admin WITHOUT echoing a connection id, so the actor's own
    // subscription (a holder of memory::admin::read) receives the event.
    let res = reqwest::Client::new()
        .put(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        200,
        "memory admin-settings update should return 200"
    );

    // Singleton entity → id is the nil UUID; assert only entity + action.
    admin_probe
        .expect_event("memory_admin_settings", "update", EVENT_TIMEOUT)
        .await;

    bystander_probe.expect_silence(SILENCE_WINDOW).await;
}
