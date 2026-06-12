//! Realtime-sync emission for the local-runtime `runtime_settings` singleton
//! and the `runtime_version` entity.
//!
//! Both surfaces are permission-scoped: a mutation fans out only to
//! connections whose snapshot satisfies the entity's audience permission
//! (admins always qualify) — `runtime_settings` →
//! `llm_local_runtime::settings_read`, `runtime_version` →
//! `llm_local_runtime::versions_read` (see `modules/sync/event.rs`).
//! These assert, over the REAL path (handler → publish → registry → SSE),
//! that the mutation produces the right frame to the right audience and that a
//! user lacking the read perm never observes it.

use std::time::Duration;

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::test_helpers::{self as lrt, LOCAL_RUNTIME_ADMIN_PERMS};
use crate::common::TestServer;
use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::create_user_with_permissions;

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);
const SILENCE_WINDOW: Duration = Duration::from_secs(1);

#[tokio::test]
async fn admin_update_delivers_runtime_settings_event_other_user_silent() {
    let server = crate::common::TestServer::start().await;
    // Actor holds the endpoint's manage perm + the audience read perm
    // (both are in LOCAL_RUNTIME_ADMIN_PERMS).
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_rt_admin",
        LOCAL_RUNTIME_ADMIN_PERMS,
    )
    .await;
    // Bob holds only the baseline (default group → profile::read); enough to
    // subscribe, but he lacks `llm_local_runtime::settings_read` so he must
    // stay silent.
    let bob = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_rt_bob",
        &[],
    )
    .await;

    let mut admin_probe = SyncProbe::open(&server, &admin.token).await;
    let mut bob_probe = SyncProbe::open(&server, &bob.token).await;

    let resp = lrt::update_runtime_settings(
        &server,
        &admin.token,
        json!({ "idle_unload_secs": 120 }),
    )
    .await;
    assert_eq!(resp.status(), 200, "runtime settings PUT should return 200");

    // Singleton → nil UUID id; assert entity + action only.
    admin_probe
        .expect_event("runtime_settings", "update", EVENT_TIMEOUT)
        .await;

    bob_probe.expect_silence(SILENCE_WINDOW).await;
}

// ============================================================================
// Entity: `runtime_version` (audience perm `llm_local_runtime::versions_read`)
//
// Trigger choice — cheapest real path:
//   * `download_task.rs` emits `runtime_version`/`create` only on a real
//     engine-download COMPLETION (needs MockReleaseServer + extract/cache).
//   * `sync_cache` emits `update` only when it discovers an UNregistered
//     binary on disk (`BinaryManager::list_binaries()` over a specific
//     cache-dir layout) — a filesystem fixture.
//   * `set_system_default` (POST .../set-default) emits `update`, and
//     `delete_runtime_version` (DELETE ...) emits `delete`, off nothing more
//     than a directly-seeded `llm_runtime_versions` row — NO download, NO
//     binary on disk. This is the path `version_usage_test.rs` already drives
//     (it asserts set-default → 200 and delete → 204 on a seeded row), so we
//     reuse its exact `seed_version` SQL fixture here.
// ============================================================================

/// Open a pool against the per-test database (mirrors
/// `test_helpers::test_pool`, inlined so this file needs no extra surface).
async fn test_pool(server: &TestServer) -> PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect to test database")
}

/// Insert a non-default `llm_runtime_versions` row directly. The binary path
/// is a deliberately non-existent stub — these tests never start an instance,
/// they only flip default / delete the row, which is what
/// `version_usage_test.rs::seed_version` proves is enough.
async fn seed_version(pool: &PgPool, engine: &str, version: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO llm_runtime_versions
            (id, engine, version, platform, arch, backend, binary_path, is_system_default)
         VALUES ($1, $2, $3, 'linux', 'x86_64', 'cpu', '/tmp/ziee-sync-emit-noexist', FALSE)",
    )
    .bind(id)
    .bind(engine)
    .bind(version)
    .execute(pool)
    .await
    .expect("seed runtime version");
    id
}

/// Set-default on a seeded engine version emits `runtime_version`/`update`
/// to a `versions_read` holder; a user without `versions_read` is silent.
#[tokio::test]
async fn set_default_delivers_runtime_version_update_other_user_silent() {
    let server = TestServer::start().await;

    // Actor: audience perm (`versions_read`) + the mutate perm
    // (`update`, required by the set-default handler).
    let admin = create_user_with_permissions(
        &server,
        "sync_rv_admin",
        &[
            "llm_local_runtime::versions_read",
            "llm_local_runtime::update",
        ],
    )
    .await;
    // Outsider: only the baseline default group — no `versions_read`.
    let outsider = create_user_with_permissions(&server, "sync_rv_outsider", &[]).await;

    let pool = test_pool(&server).await;
    let version_id = seed_version(&pool, "llamacpp", "v0.0.0-sync-emit-1").await;

    let mut admin_probe = SyncProbe::open(&server, &admin.token).await;
    let mut outsider_probe = SyncProbe::open(&server, &outsider.token).await;

    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/local-runtime/versions/{version_id}/set-default")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("set-default request");
    assert_eq!(resp.status(), 200, "set-default should 200 on a seeded row");

    let frame = admin_probe
        .expect_event("runtime_version", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(
        frame.id,
        version_id.to_string(),
        "the update frame carries the version's id"
    );

    outsider_probe.expect_silence(SILENCE_WINDOW).await;
}

/// Deleting a seeded (non-default, unused) engine version emits
/// `runtime_version`/`delete` to a `versions_read` holder; a user without
/// `versions_read` is silent.
#[tokio::test]
async fn delete_delivers_runtime_version_delete_other_user_silent() {
    let server = TestServer::start().await;

    let admin = create_user_with_permissions(
        &server,
        "sync_rv_del_admin",
        &[
            "llm_local_runtime::versions_read",
            "llm_local_runtime::delete",
        ],
    )
    .await;
    let outsider = create_user_with_permissions(&server, "sync_rv_del_outsider", &[]).await;

    let pool = test_pool(&server).await;
    let version_id = seed_version(&pool, "llamacpp", "v0.0.0-sync-emit-2").await;

    let mut admin_probe = SyncProbe::open(&server, &admin.token).await;
    let mut outsider_probe = SyncProbe::open(&server, &outsider.token).await;

    let resp = reqwest::Client::new()
        .delete(server.api_url(&format!("/local-runtime/versions/{version_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("delete request");
    assert_eq!(
        resp.status(),
        204,
        "delete should 204 on a non-default, unused, seeded row"
    );

    let frame = admin_probe
        .expect_event("runtime_version", "delete", EVENT_TIMEOUT)
        .await;
    assert_eq!(
        frame.id,
        version_id.to_string(),
        "the delete frame carries the version's id"
    );

    outsider_probe.expect_silence(SILENCE_WINDOW).await;
}
