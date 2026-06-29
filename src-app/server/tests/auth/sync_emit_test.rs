//! Realtime-sync emission tests for admin auth-provider mutations.
//!
//! `admin_create_provider` / `admin_update_provider` / `admin_delete_provider`
//! each `sync_publish(SyncEntity::AuthProvider, ...)` to the
//! `Permission("auth_providers::read")` audience. Verified end-to-end (handler
//! → sync_publish → registry → SSE) via `SyncProbe`. A user WITHOUT
//! `auth_providers::read` must observe nothing.

use serde_json::json;
use std::time::Duration;

use crate::common::sync_probe::SyncProbe;

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);
const SILENCE_WINDOW: Duration = Duration::from_secs(1);

#[tokio::test]
async fn auth_provider_create_update_delete_emit_to_read_holder_only() {
    let server = crate::common::TestServer::start().await;

    // Actor: manage (to mutate) + read (to be in the audience).
    let actor = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "authprov_sync_actor",
        &["auth_providers::manage", "auth_providers::read"],
    )
    .await;
    // A user with ONLY profile::read (can open the stream) but NOT
    // auth_providers::read — must observe nothing from an auth-provider change.
    let no_read = crate::common::test_helpers::create_user_with_only_permissions(
        &server,
        "authprov_sync_no_read",
        &["profile::read"],
    )
    .await;

    let mut actor_probe = SyncProbe::open(&server, &actor.token).await;
    let mut no_read_probe = SyncProbe::open(&server, &no_read.token).await;

    let client = reqwest::Client::new();
    let bearer = format!("Bearer {}", actor.token);

    // --- Create (enabled=false so no upstream health probe runs) ---
    let created: serde_json::Value = client
        .post(server.api_url("/admin/auth-providers"))
        .header("Authorization", &bearer)
        .json(&json!({
            "name": "sync-oidc",
            "provider_type": "oidc",
            "enabled": false,
            "config": {}
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    // Create response is an envelope: { provider, connection_warning }.
    let id = created["provider"]["id"]
        .as_str()
        .expect("create returns id")
        .to_string();

    let ev = actor_probe
        .expect_event("auth_provider", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(ev.id, id, "auth_provider/create must carry the provider id");

    // --- Update ---
    let upd = client
        .put(server.api_url(&format!("/admin/auth-providers/{id}")))
        .header("Authorization", &bearer)
        .json(&json!({ "name": "sync-oidc-renamed" }))
        .send()
        .await
        .unwrap();
    assert!(upd.status().is_success(), "update should succeed: {}", upd.status());
    let ev = actor_probe
        .expect_event("auth_provider", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(ev.id, id, "auth_provider/update must carry the provider id");

    // --- Delete ---
    let del = client
        .delete(server.api_url(&format!("/admin/auth-providers/{id}")))
        .header("Authorization", &bearer)
        .send()
        .await
        .unwrap();
    assert!(del.status().is_success(), "delete should succeed: {}", del.status());
    let ev = actor_probe
        .expect_event("auth_provider", "delete", EVENT_TIMEOUT)
        .await;
    assert_eq!(ev.id, id, "auth_provider/delete must carry the provider id");

    // The non-read user must have seen NONE of the three frames.
    no_read_probe.expect_silence(SILENCE_WINDOW).await;
}
