//! Realtime-sync emission coverage for the `llm_provider` module.
//!
//! Proves that a real REST mutation through the production handler emits the
//! right `sync` frame(s) to the right audience, end-to-end (handler → publish
//! → registry → SSE), via `SyncProbe`. Per the routing table in
//! `modules/sync/event.rs`, `SyncEntity`/`SyncAction` serialize `snake_case`,
//! so the wire entity strings are `llm_provider` / `user_llm_provider` /
//! `api_key` and the actions are `create` / `update` / `delete`.
//!
//! Three entities, three audience shapes (matching `audience_kind`):
//!
//! - `llm_provider` — `Permission("llm_providers::read")`. An admin
//!   create/update of a custom (non-local) provider is delivered to holders of
//!   that read perm; a user lacking it stays silent.
//! - `user_llm_provider` — `Permission("user_llm_providers::read")`. Emitted
//!   ALONGSIDE every provider mutation (dual-audience), so a regular user who
//!   holds the user read perm refreshes their accessible-providers view.
//! - `api_key` — `Owner`. A user saving THEIR OWN provider api key only
//!   notifies their own connections (the frame `id` is the provider id); a
//!   different user never observes it.

use std::time::Duration;

use reqwest::StatusCode;
use serde_json::json;

use crate::common::sync_probe::SyncProbe;

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);
const SILENCE_WINDOW: Duration = Duration::from_secs(1);

/// POST /llm-providers as `token`, creating a `custom` (non-local) provider so
/// no upstream network call is ever made. Returns the created provider JSON.
async fn create_custom_provider(
    server: &crate::common::TestServer,
    token: &str,
    name: &str,
) -> serde_json::Value {
    // `custom` skips the "enabled non-local provider must have an api_key"
    // validation AND never reaches out over the network — purely a DB row.
    let payload = json!({
        "name": name,
        "provider_type": "custom",
        "enabled": false,
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "custom provider create should return 201"
    );
    response.json().await.unwrap()
}

// =====================================================
// llm_provider — Permission("llm_providers::read") audience
// (and the dual-emitted user_llm_provider — Permission("user_llm_providers::read"))
// =====================================================

#[tokio::test]
async fn provider_create_and_update_emit_llm_provider_to_read_holder_and_user_view_to_user_read_holder()
{
    let server = crate::common::TestServer::start().await;

    // The actor mutates providers AND must hold `llm_providers::read` to be in
    // the `llm_provider` event's audience.
    let actor = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "llmprov_sync_actor",
        &[
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;
    // A regular user holding the user-facing read perm (granted by the default
    // Users group) — the dual-emitted `user_llm_provider` frame must reach them.
    let user_view = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "llmprov_sync_user_view",
        &[],
    )
    .await;
    // A user with ONLY `profile::read` (enough to open the sync stream) but
    // NEITHER `llm_providers::read` nor `user_llm_providers::read` — stripped
    // from the default group so no baseline read leaks in. They must observe
    // nothing at all from a provider mutation.
    let no_read = crate::common::test_helpers::create_user_with_only_permissions(
        &server,
        "llmprov_sync_no_read",
        &["profile::read"],
    )
    .await;

    let mut actor_probe = SyncProbe::open(&server, &actor.token).await;
    let mut user_view_probe = SyncProbe::open(&server, &user_view.token).await;
    let mut no_read_probe = SyncProbe::open(&server, &no_read.token).await;

    // --- Create: emits llm_provider/create + user_llm_provider/create ---
    let provider = create_custom_provider(&server, &actor.token, "Sync Provider").await;
    let id = provider["id"].as_str().unwrap().to_string();

    let created = actor_probe
        .expect_event("llm_provider", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(created.id, id, "llm_provider/create must carry the provider id");

    let user_created = user_view_probe
        .expect_event("user_llm_provider", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(
        user_created.id, id,
        "user_llm_provider/create must carry the provider id"
    );

    // The no-read user must not see EITHER dual-emitted frame.
    no_read_probe.expect_silence(SILENCE_WINDOW).await;

    // --- Update: emits llm_provider/update + user_llm_provider/update ---
    let update_resp = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{}", id)))
        .header("Authorization", format!("Bearer {}", actor.token))
        .json(&json!({ "name": "Sync Provider Renamed" }))
        .send()
        .await
        .unwrap();
    assert_eq!(update_resp.status(), StatusCode::OK);

    let updated = actor_probe
        .expect_event("llm_provider", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(updated.id, id, "llm_provider/update must carry the provider id");

    let user_updated = user_view_probe
        .expect_event("user_llm_provider", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(
        user_updated.id, id,
        "user_llm_provider/update must carry the provider id"
    );

    // Still nothing for the user lacking both read perms.
    no_read_probe.expect_silence(SILENCE_WINDOW).await;
}

/// DELETE /llm-providers/{id} must dual-emit llm_provider/delete (to
/// llm_providers::read holders) AND user_llm_provider/delete (to
/// user_llm_providers::read holders) — the create/update test covered both
/// dual-emits but not the delete action.
#[tokio::test]
async fn provider_delete_dual_emits_llm_provider_and_user_view_delete() {
    let server = crate::common::TestServer::start().await;

    let actor = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "del_actor",
        &[
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::delete",
        ],
    )
    .await;
    let user_view = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "del_user_view",
        &["user_llm_providers::read"],
    )
    .await;

    let mut actor_probe = SyncProbe::open(&server, &actor.token).await;
    let mut user_view_probe = SyncProbe::open(&server, &user_view.token).await;

    let provider = create_custom_provider(&server, &actor.token, "Delete Me Provider").await;
    let id = provider["id"].as_str().unwrap().to_string();
    // Drain the create frames so the delete assertions can't race them.
    actor_probe
        .expect_event("llm_provider", "create", EVENT_TIMEOUT)
        .await;
    user_view_probe
        .expect_event("user_llm_provider", "create", EVENT_TIMEOUT)
        .await;

    let del = reqwest::Client::new()
        .delete(server.api_url(&format!("/llm-providers/{}", id)))
        .header("Authorization", format!("Bearer {}", actor.token))
        .send()
        .await
        .unwrap();
    assert!(del.status().is_success(), "provider delete should succeed");

    let deleted = actor_probe
        .expect_event("llm_provider", "delete", EVENT_TIMEOUT)
        .await;
    assert_eq!(deleted.id, id, "llm_provider/delete must carry the provider id");
    let user_deleted = user_view_probe
        .expect_event("user_llm_provider", "delete", EVENT_TIMEOUT)
        .await;
    assert_eq!(
        user_deleted.id, id,
        "user_llm_provider/delete must carry the provider id"
    );
}

// =====================================================
// api_key — Owner audience
// =====================================================

#[tokio::test]
async fn user_api_key_save_emits_to_owner_only() {
    let server = crate::common::TestServer::start().await;

    // An admin creates a custom provider the users can attach a personal key to.
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "apikey_sync_admin",
        &["llm_providers::create"],
    )
    .await;
    // The OWNER: a vanilla user gets `profile::edit` (to save a key) +
    // `user_llm_providers::read` from the default Users group.
    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "apikey_sync_owner",
        &[],
    )
    .await;
    // A DIFFERENT vanilla user — must never see the owner's api_key event.
    let other = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "apikey_sync_other",
        &[],
    )
    .await;

    let provider = create_custom_provider(&server, &admin.token, "Key Provider").await;
    let provider_id = provider["id"].as_str().unwrap().to_string();

    let mut owner_probe = SyncProbe::open(&server, &owner.token).await;
    let mut other_probe = SyncProbe::open(&server, &other.token).await;

    // Owner saves their personal key for the provider (POST → 204, action=update,
    // the event id is the provider id).
    let save_resp = reqwest::Client::new()
        .post(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&json!({ "provider_id": provider_id, "api_key": "sk-owner-secret-1234" }))
        .send()
        .await
        .unwrap();
    assert_eq!(save_resp.status(), StatusCode::NO_CONTENT);

    let frame = owner_probe
        .expect_event("api_key", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(
        frame.id, provider_id,
        "api_key frame id must be the provider id (per the handler)"
    );

    // Owner-scoped: a different user observes nothing.
    other_probe.expect_silence(SILENCE_WINDOW).await;
}

#[tokio::test]
async fn user_api_key_save_then_delete_each_emit_to_owner() {
    let server = crate::common::TestServer::start().await;

    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "apikey_sync_cud_admin",
        &["llm_providers::create"],
    )
    .await;
    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "apikey_sync_cud_owner",
        &[],
    )
    .await;

    let provider = create_custom_provider(&server, &admin.token, "CUD Key Provider").await;
    let provider_id = provider["id"].as_str().unwrap().to_string();

    let mut owner_probe = SyncProbe::open(&server, &owner.token).await;
    let client = reqwest::Client::new();

    // Save → api_key/update.
    let save_resp = client
        .post(server.api_url("/user-llm-providers/api-keys"))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&json!({ "provider_id": provider_id, "api_key": "sk-cud-1234" }))
        .send()
        .await
        .unwrap();
    assert_eq!(save_resp.status(), StatusCode::NO_CONTENT);

    let saved = owner_probe
        .expect_event("api_key", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(saved.id, provider_id);

    // Delete → api_key/delete (also keyed by provider id).
    let delete_resp = client
        .delete(server.api_url(&format!("/user-llm-providers/api-keys/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();
    assert_eq!(delete_resp.status(), StatusCode::NO_CONTENT);

    let deleted = owner_probe
        .expect_event("api_key", "delete", EVENT_TIMEOUT)
        .await;
    assert_eq!(deleted.id, provider_id);
}
