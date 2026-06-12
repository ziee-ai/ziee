//! Realtime-sync emission for the `conversation` entity.
//!
//! Asserts that a real REST mutation on `/conversations` produces the correct
//! `sync` frame (`conversation`/`create|update|delete`) on the OWNER's
//! subscribed stream, carrying the mutated conversation's id — and that a
//! DIFFERENT user never observes it (owner-scoped audience). Mirrors
//! `tests/project/sync_emit_test.rs`; chat was the one owning module missing a
//! sync-emit test despite emitting from `conversations.rs` create/rename/delete.
//!
//! (Per-turn message Update — emitted from `start_generation` on completion —
//! is covered in `chat_stream_test.rs`, which drives a real generation.)

use std::time::Duration;

use reqwest::StatusCode;

use super::helpers;
use crate::common::sync_probe::SyncProbe;

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);
const SILENCE_WINDOW: Duration = Duration::from_secs(1);

fn owner_conversation_permissions() -> &'static [&'static str] {
    &[
        "conversations::create",
        "conversations::read",
        "conversations::edit",
        "conversations::delete",
        "messages::read",
    ]
}

#[tokio::test]
async fn conversation_create_emits_to_owner_only() {
    let server = crate::common::TestServer::start().await;

    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "conv_sync_owner",
        owner_conversation_permissions(),
    )
    .await;
    // A second user (baseline profile::read only) — enough to subscribe, but he
    // must never see the owner's conversation event.
    let other =
        crate::common::test_helpers::create_user_with_permissions(&server, "conv_sync_other", &[])
            .await;

    let mut owner_probe = SyncProbe::open(&server, &owner.token).await;
    let mut other_probe = SyncProbe::open(&server, &other.token).await;

    let conversation =
        helpers::create_conversation(&server, &owner.token, None, Some("Sync Conv")).await;
    let id = conversation["id"].as_str().unwrap().to_string();

    let frame = owner_probe
        .expect_event("conversation", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(frame.id, id, "the frame must carry the new conversation's id");

    // Cross-user isolation: a user who does not own the conversation stays silent.
    other_probe.expect_silence(SILENCE_WINDOW).await;
}

#[tokio::test]
async fn conversation_create_update_delete_each_emit_to_owner() {
    let server = crate::common::TestServer::start().await;

    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "conv_sync_cud",
        owner_conversation_permissions(),
    )
    .await;
    let mut probe = SyncProbe::open(&server, &owner.token).await;

    // Create.
    let conversation =
        helpers::create_conversation(&server, &owner.token, None, Some("CUD Conv")).await;
    let id = conversation["id"].as_str().unwrap().to_string();
    let conv_uuid = helpers::parse_uuid(&conversation["id"]);
    let created = probe
        .expect_event("conversation", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(created.id, id);

    // Update (rename via PUT /conversations/{id}).
    helpers::update_conversation(&server, &owner.token, conv_uuid, Some("CUD Conv Renamed")).await;
    let updated = probe
        .expect_event("conversation", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(updated.id, id);

    // Delete.
    let status = helpers::delete_conversation(&server, &owner.token, conv_uuid).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let deleted = probe
        .expect_event("conversation", "delete", EVENT_TIMEOUT)
        .await;
    assert_eq!(deleted.id, id);
}
