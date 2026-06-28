//! Realtime-sync emission coverage for the summarization admin settings.
//!
//! A settings update through the production handler must publish a
//! `SummarizationAdminSettings`/`update` frame to holders of
//! `summarization::settings::read` (Audience::perm), and NOT reach a user
//! lacking that read perm. The wire id is `Uuid::nil` (the row is a singleton).
//! Mirrors `tests/web_search/settings_test.rs::test_web_search_settings_update_emits_sync_to_admins_only`.

use std::time::Duration;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::{create_user_with_only_permissions, create_user_with_permissions};
use crate::common::TestServer;

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);
const SILENCE_WINDOW: Duration = Duration::from_secs(1);

#[tokio::test]
async fn summarization_admin_settings_update_emits_sync_to_read_holders_only() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(
        &server,
        "summ_sync_admin",
        &["summarization::settings::read", "summarization::settings::manage"],
    )
    .await;
    // A user WITHOUT summarization::settings::read (only profile::read to
    // subscribe) is outside the audience — negative control.
    let plain =
        create_user_with_only_permissions(&server, "summ_sync_plain", &["profile::read"]).await;

    let mut admin_probe = SyncProbe::open(&server, &admin.token).await;
    let mut plain_probe = SyncProbe::open(&server, &plain.token).await;

    let res = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "summarize_after_tokens": 6000 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "settings update should 200");

    let frame = admin_probe
        .expect_event("summarization_admin_settings", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(frame.id, Uuid::nil().to_string(), "singleton → nil id");

    // The non-read user observes nothing.
    plain_probe.expect_silence(SILENCE_WINDOW).await;
}
