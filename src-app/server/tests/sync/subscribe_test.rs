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
