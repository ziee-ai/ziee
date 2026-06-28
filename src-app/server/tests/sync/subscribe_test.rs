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

/// Mint an access token (HS256, the test config's secret/iss/aud) that expires
/// `secs` from now, for an existing user id. Mirrors how the real JwtService
/// shapes access-token claims so `validate_access_token` accepts it.
fn mint_access_token(user_id: &str, secs: i64) -> String {
    use jsonwebtoken::{encode, EncodingKey, Header};
    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({
        "sub": user_id,
        "exp": now + secs,
        "iat": now,
        "iss": "ziee",
        "aud": "ziee-api",
        "username": "sync_exp",
        "email": "sync_exp@example.com",
        "is_admin": false,
    });
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(b"test-secret-key-for-jwt-tokens-min-32-chars-long"),
    )
    .unwrap()
}

/// The stream is bounded by the access token's `exp`: a token expiring in ~2s
/// must tear the SSE stream down at the deadline (the `sleep_until(deadline)`
/// arm), so the response body completes on its own — NOT hang until the token
/// would otherwise live for 24h.
#[tokio::test]
async fn subscribe_stream_closes_at_token_expiry() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_exp_user",
        &["profile::read"],
    )
    .await;

    // A short-lived token (still valid NOW, so the stream opens; lapses in ~2s).
    let token = mint_access_token(&user.user_id, 2);
    let res = reqwest::Client::new()
        .get(server.api_url("/sync/subscribe"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "stream opens while the token is still valid");

    // Reading the full body returns when the server closes the stream at the
    // exp deadline. A generous 15s timeout guards against the body hanging
    // (which would mean the exp teardown didn't fire).
    let start = std::time::Instant::now();
    let body = tokio::time::timeout(std::time::Duration::from_secs(15), res.bytes()).await;
    assert!(body.is_ok(), "stream must close on token expiry, not hang open");
    let elapsed = start.elapsed();
    assert!(
        elapsed < std::time::Duration::from_secs(14),
        "stream closed well before the 24h fallback (at ~exp); elapsed={elapsed:?}"
    );
}

/// The sync stream's FIRST frame is the `connected` handshake carrying a valid
/// UUID `connection_id` — the contract every client depends on to echo
/// `X-Sync-Connection-Id` back for self-echo suppression. The existing open
/// test asserts only the 200 + content-type; this validates the handshake
/// frame's event name + payload shape on the wire.
#[tokio::test]
async fn subscribe_first_frame_is_connected_handshake_with_uuid() {
    use futures_util::StreamExt;
    use std::time::Duration;

    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_handshake",
        &["profile::read"],
    )
    .await;

    let resp = reqwest::Client::new()
        .get(server.api_url("/sync/subscribe"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Read until the first complete SSE frame (blank-line terminated), bounded.
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let deadline = tokio::time::sleep(Duration::from_secs(10));
    tokio::pin!(deadline);
    let frame = loop {
        tokio::select! {
            _ = &mut deadline => panic!("no handshake frame within 10s; buf={buf:?}"),
            chunk = stream.next() => match chunk {
                Some(Ok(b)) => {
                    buf.push_str(&String::from_utf8_lossy(&b));
                    if let Some(pos) = buf.find("\n\n") {
                        break buf[..pos].to_string();
                    }
                }
                Some(Err(e)) => panic!("stream error: {e}"),
                None => panic!("stream ended before a frame: {buf:?}"),
            }
        }
    };

    // The first frame names the `connected` event ...
    assert!(
        frame.lines().any(|l| l.trim() == "event: connected"),
        "first frame must be the connected handshake: {frame:?}"
    );
    // ... and its data payload carries a parseable UUID connection_id.
    let data_line = frame
        .lines()
        .find_map(|l| l.strip_prefix("data:"))
        .expect("a data: line in the handshake frame");
    let payload: serde_json::Value =
        serde_json::from_str(data_line.trim()).expect("handshake data is JSON");
    let conn = payload["connection_id"].as_str().expect("connection_id present");
    assert!(
        uuid::Uuid::parse_str(conn).is_ok(),
        "connection_id must be a valid UUID, got {conn:?}"
    );
}

/// The per-user connection cap is enforced through the HTTP `/sync/subscribe`
/// path (not just the registry unit test): one user opening more than
/// PER_USER_MAX_CONNECTIONS (12) live streams gets a 429 on the overflow
/// subscribe. The held responses keep the earlier connections registered.
#[tokio::test]
async fn subscribe_enforces_per_user_connection_cap_with_429() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_cap_http",
        &["profile::read"],
    )
    .await;
    let client = reqwest::Client::new();

    // Hold 12 live subscribe streams open (registered on the server).
    let mut held = Vec::new();
    for i in 0..12 {
        let res = client
            .get(server.api_url("/sync/subscribe"))
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 200, "connection {i} should open");
        held.push(res); // keep the stream alive → stays registered
    }

    // The 13th subscribe for the SAME user exceeds the per-user cap → 429.
    let overflow = client
        .get(server.api_url("/sync/subscribe"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        overflow.status(),
        429,
        "the (cap+1)th connection must be refused with 429"
    );

    drop(held);
}
