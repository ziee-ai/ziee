//! Realtime-sync emission coverage for the `assistant` module.
//!
//! Proves that a real REST mutation through the production handler emits the
//! right `sync` frame to the right audience, end-to-end (handler → publish →
//! registry → SSE), via `SyncProbe`:
//!
//! - `assistant` is OWNER-scoped: only the creating user's stream observes the
//!   frame; a different user stays silent.
//! - `assistant_template` is EVERYONE-scoped: every subscribed user — the actor
//!   AND an unrelated baseline user — observes the frame (no silence check).
//!
//! Per the sync routing table in `modules/sync/event.rs`, `SyncEntity` and
//! `SyncAction` serialize `snake_case`, so the wire strings are `assistant` /
//! `assistant_template` and `create` / `update` / `delete`.

use std::time::Duration;

use serde_json::json;

use crate::common::sync_probe::SyncProbe;

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);
const SILENCE_WINDOW: Duration = Duration::from_secs(1);

/// POST /assistants as `token`, returning the new assistant id.
async fn create_user_assistant(
    server: &crate::common::TestServer,
    token: &str,
    name: &str,
) -> String {
    let res = reqwest::Client::new()
        .post(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "name": name }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201, "assistant create should return 201");
    let row: serde_json::Value = res.json().await.unwrap();
    row["id"].as_str().unwrap().to_string()
}

/// POST /assistant-templates as `token`, returning the new template id.
async fn create_template_assistant(
    server: &crate::common::TestServer,
    token: &str,
    name: &str,
) -> String {
    let res = reqwest::Client::new()
        .post(server.api_url("/assistant-templates"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "name": name }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201, "template create should return 201");
    let row: serde_json::Value = res.json().await.unwrap();
    row["id"].as_str().unwrap().to_string()
}

// =====================================================
// assistant — OWNER audience
// =====================================================

#[tokio::test]
async fn user_assistant_create_is_delivered_to_owner_not_to_other_users() {
    let server = crate::common::TestServer::start().await;
    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_assistant_owner",
        &["assistants::create"],
    )
    .await;
    // A different user holds only the baseline (default group → profile::read);
    // enough to subscribe, but they must NEVER see the owner-scoped event.
    let other = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_assistant_other",
        &[],
    )
    .await;

    // Open BOTH probes BEFORE creating the assistant: registering a user
    // auto-clones default template assistants before the user subscribes, so
    // those never reach the probe. We only assert on the assistant we create
    // after the probe is live.
    let mut owner_probe = SyncProbe::open(&server, &owner.token).await;
    let mut other_probe = SyncProbe::open(&server, &other.token).await;

    let id = create_user_assistant(&server, &owner.token, "Owned Assistant").await;

    let frame = owner_probe
        .expect_event("assistant", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(frame.id, id, "the frame must carry the new assistant's id");

    // Owner-scoped: the unrelated user must observe nothing.
    other_probe.expect_silence(SILENCE_WINDOW).await;
}

// =====================================================
// assistant_template — EVERYONE audience
// =====================================================

#[tokio::test]
async fn template_create_is_delivered_to_the_actor_and_every_other_user() {
    let server = crate::common::TestServer::start().await;
    // The actor needs the template-manage (create) permission.
    let actor = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_tmpl_actor",
        &["assistant_templates::create"],
    )
    .await;
    // A second, unrelated baseline user — no template permissions at all. The
    // Everyone audience must still reach them.
    let bystander = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_tmpl_bystander",
        &[],
    )
    .await;

    let mut actor_probe = SyncProbe::open(&server, &actor.token).await;
    let mut bystander_probe = SyncProbe::open(&server, &bystander.token).await;

    let id = create_template_assistant(&server, &actor.token, "Everyone Template").await;

    // Both streams receive the same Everyone-audience frame. No silence check —
    // the audience is Everyone, so silence on any subscriber would be a bug.
    let actor_frame = actor_probe
        .expect_event("assistant_template", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(actor_frame.id, id, "actor frame must carry the template id");

    let bystander_frame = bystander_probe
        .expect_event("assistant_template", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(
        bystander_frame.id, id,
        "bystander frame must carry the template id"
    );
}
