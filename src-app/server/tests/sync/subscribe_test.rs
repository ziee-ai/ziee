// Integration coverage for the realtime-sync SSE subscribe endpoint.
//
// The security-critical fan-out/audience routing is covered deterministically
// by the in-source unit tests (`modules/sync/{registry,event}.rs`), and the
// full real path (cross-device delivery + cross-user isolation) by the
// Playwright E2E (`ui/tests/e2e/13-sync`). Here we just assert the HTTP
// endpoint itself: it is auth-gated and opens an event-stream for an
// authenticated user. `reqwest::send()` resolves once the response headers
// arrive, so we can assert status + content-type without consuming the
// (intentionally long-lived) stream body — dropping the response closes it,
// and the server's ConnGuard unregisters the connection.

#[tokio::test]
async fn subscribe_rejects_unauthenticated() {
    let server = crate::common::TestServer::start().await;
    let res = reqwest::Client::new()
        .get(server.api_url("/sync/subscribe"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        401,
        "GET /sync/subscribe must require authentication"
    );
}

#[tokio::test]
async fn subscribe_with_valid_token_opens_event_stream() {
    let server = crate::common::TestServer::start().await;
    // profile::read is the baseline gate every active user holds.
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_subscriber",
        &["profile::read"],
    )
    .await;

    let res = reqwest::Client::new()
        .get(server.api_url("/sync/subscribe"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.status(),
        200,
        "an authenticated user must be able to open the sync stream"
    );
    let content_type = res
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("text/event-stream"),
        "sync subscribe must return an SSE stream, got content-type: {content_type}"
    );
    // Drop `res` here → closes the stream → server unregisters the connection.
}

/// File sync end-to-end through the SSE stream: uploading a file fires
/// `publish_file_changed` (owner-scoped `File`/`Update`), and the uploader's own
/// sync subscription receives a `file`/`update` frame carrying the file id. The
/// generic subscribe test above only proves the stream opens; this proves a
/// file-specific entity is actually delivered over it.
#[tokio::test]
async fn upload_delivers_file_sync_event_to_owner() {
    use crate::common::sync_probe::SyncProbe;
    use std::time::Duration;

    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_file_owner",
        &["files::upload", "files::read"],
    )
    .await;

    let mut probe = SyncProbe::open(&server, &user.token).await;

    // Upload a file (no X-Sync-Connection-Id header → no self-echo suppression,
    // so the uploader's own probe receives the event).
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(b"hello sync".to_vec())
            .file_name("sync.txt")
            .mime_str("text/plain")
            .unwrap(),
    );
    let res = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("upload");
    assert!(res.status().is_success(), "upload should succeed: {}", res.status());
    let body: serde_json::Value = res.json().await.unwrap();
    let file_id = body["id"].as_str().expect("uploaded file id").to_string();

    let frame = probe
        .expect_event("file", "update", Duration::from_secs(5))
        .await;
    assert_eq!(
        frame.id, file_id,
        "the file sync frame must carry the uploaded file id"
    );
}
