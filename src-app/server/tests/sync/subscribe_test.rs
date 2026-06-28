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

// audit id all-9e841aed8753 — the connection-cap 429 path exercised through the
// REAL HTTP handler (not just the in-source registry unit test). `register`
// runs in `subscribe_sync` BEFORE the SSE stream is returned (handlers.rs:78),
// so a capped registration surfaces as a 429 HTTP status on the response
// headers. We open PER_USER_MAX_CONNECTIONS (12) streams for one user and hold
// them alive in a Vec (dropping a response would close the stream and free a
// slot), then assert the 13th concurrent subscribe is rejected with 429.
#[tokio::test]
async fn subscribe_rejects_when_per_user_connection_cap_exceeded() {
    const PER_USER_MAX_CONNECTIONS: usize = 12;

    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_cap",
        &["profile::read"],
    )
    .await;
    let client = reqwest::Client::new();

    // Hold the max number of concurrent streams open (kept in scope so the
    // underlying connections stay registered server-side).
    let mut held = Vec::new();
    for i in 0..PER_USER_MAX_CONNECTIONS {
        let res = client
            .get(server.api_url("/sync/subscribe"))
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .unwrap();
        assert_eq!(
            res.status(),
            200,
            "connection {i} (within the cap) must be accepted"
        );
        held.push(res);
    }

    // The next subscribe for the SAME user exceeds the per-user cap → 429.
    let over = client
        .get(server.api_url("/sync/subscribe"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        over.status(),
        429,
        "subscribe beyond the per-user connection cap must be rejected with 429"
    );

    // Keep the held streams alive until the assertion above has run.
    drop(held);
}
