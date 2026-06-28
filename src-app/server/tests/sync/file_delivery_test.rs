//! File-specific realtime-sync delivery over the REAL path. `delivery_test.rs`
//! proves the generic mechanism using `memory` as the owner-scoped vehicle;
//! this asserts the per-entity `File` delivery the `file` module owns: a real
//! `POST /files/upload` (which calls `file::sync::publish_file_changed` at
//! `handlers/upload.rs:245`) emits a `file`/`update` frame carrying the stable
//! file_id to the owner's subscribed stream — and never to another user
//! (owner-scoped audience). `SyncEntity::File` serializes snake_case → `file`,
//! `SyncAction::Update` → `update`.

use std::time::Duration;

use reqwest::multipart;
use serde_json::Value;

use crate::common::sync_probe::SyncProbe;

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);
const SILENCE_WINDOW: Duration = Duration::from_secs(1);

/// Upload a tiny inline text file as `token`; returns the new file id.
async fn upload_file(server: &crate::common::TestServer, token: &str) -> String {
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(b"file sync delivery body".to_vec())
            .file_name("sync.txt")
            .mime_str("text/plain")
            .unwrap(),
    );
    let res = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {token}"))
        .multipart(form)
        .send()
        .await
        .expect("upload request failed");
    assert_eq!(res.status(), 201, "file upload should return 201");
    let body: Value = res.json().await.expect("parse upload response");
    body["id"].as_str().expect("upload returns an id").to_string()
}

#[tokio::test]
async fn file_upload_delivers_file_update_to_owner_not_other_users() {
    let server = crate::common::TestServer::start().await;
    let alice = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "file_sync_alice",
        &["files::upload"],
    )
    .await;
    // Bob holds only the baseline (default group → profile::read): enough to
    // subscribe, but he must NEVER receive Alice's owner-scoped file event.
    let bob = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "file_sync_bob",
        &[],
    )
    .await;

    let mut alice_probe = SyncProbe::open(&server, &alice.token).await;
    let mut bob_probe = SyncProbe::open(&server, &bob.token).await;

    let file_id = upload_file(&server, &alice.token).await;

    // The real upload handler published a `file`/`update` carrying the file id.
    let frame = alice_probe
        .expect_event("file", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(
        frame.id, file_id,
        "the file sync frame must carry the uploaded file's stable id"
    );

    // Owner-scoped: Bob's stream stays silent.
    bob_probe.expect_silence(SILENCE_WINDOW).await;
}
