use std::time::Duration;
use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::create_user_with_permissions;
use serde_json::json;
use uuid::Uuid;
use crate::common::test_helpers::create_user_with_only_permissions;
use crate::common::TestServer;

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);

const SILENCE: Duration = Duration::from_secs(1);

#[tokio::test]
async fn summarization_settings_update_emits_to_read_holders_only() {
    let server = crate::common::TestServer::start().await;
    let admin = create_user_with_permissions(
        &server,
        "summ_sync_admin",
        &["summarization::settings::manage", "summarization::settings::read"],
    )
    .await;
    let reader = create_user_with_permissions(
        &server,
        "summ_sync_reader",
        &["summarization::settings::read"],
    )
    .await;
    // A user with neither perm — must NOT receive the perm-scoped frame.
    let outsider = create_user_with_permissions(&server, "summ_sync_outsider", &[]).await;

    let mut admin_probe = SyncProbe::open(&server, &admin.token).await;
    let mut reader_probe = SyncProbe::open(&server, &reader.token).await;
    let mut outsider_probe = SyncProbe::open(&server, &outsider.token).await;

    let res = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        // Must stay above the default summarizer_keep_recent_tokens (3000),
        // else the cross-field validation rejects the update.
        .json(&serde_json::json!({ "summarize_after_tokens": 8000 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "settings update: {}", res.text().await.unwrap_or_default());

    // Both read-perm holders (incl. the actor) observe the singleton update.
    let f = admin_probe
        .expect_event("summarization_admin_settings", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(f.id, "00000000-0000-0000-0000-000000000000", "singleton nil id");
    reader_probe
        .expect_event("summarization_admin_settings", "update", EVENT_TIMEOUT)
        .await;

    // The outsider (no read perm) must not see the perm-scoped frame.
    outsider_probe.expect_silence(SILENCE).await;
}

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

