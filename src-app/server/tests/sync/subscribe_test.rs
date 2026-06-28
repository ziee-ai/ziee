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
