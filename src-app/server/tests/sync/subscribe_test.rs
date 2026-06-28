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

/// Account deactivation cuts off realtime sync: a user who could open the SSE
/// stream is REFUSED on (re)connect once an admin deactivates them, because the
/// subscribe handler's `RequirePermissions<(ProfileRead,)>` extractor re-checks
/// `is_active` from scratch every connect (the same check the stream's periodic
/// 60s re-resolve enforces mid-stream). Their JWT is still cryptographically
/// valid — it's the is_active gate that closes the door.
#[tokio::test]
async fn subscribe_refuses_a_deactivated_user() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_deact",
        &["profile::read"],
    )
    .await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_deact_admin",
        &["users::edit"],
    )
    .await;

    // Active → the stream opens.
    let ok = reqwest::Client::new()
        .get(server.api_url("/sync/subscribe"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), 200, "an active user must be able to subscribe");
    drop(ok); // close the stream

    // Admin deactivates the user.
    let deact = reqwest::Client::new()
        .post(server.api_url(&format!("/users/{}", user.user_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "is_active": false }))
        .send()
        .await
        .unwrap();
    assert!(
        deact.status().is_success(),
        "deactivation should succeed; got {}",
        deact.status()
    );

    // Same (still-unexpired) token → reconnect is now refused by the is_active gate.
    let refused = reqwest::Client::new()
        .get(server.api_url("/sync/subscribe"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert!(
        refused.status() == 401 || refused.status() == 403,
        "a deactivated user must be refused the SSE stream; got {}",
        refused.status()
    );
}

/// The SSE stream is bounded by the access token's `exp`: when the JWT lapses
/// mid-stream the server tears the connection down (sync/handlers.rs computes
/// `deadline = exp - now` and `select!`s a `sleep_until(deadline)` arm), so the
/// client is forced to reconnect with a fresh token (which re-runs the auth
/// extractor from scratch). This asserts that teardown actually fires: a stream
/// opened with a still-valid-but-near-expiry token closes on its own shortly
/// after `exp`, even though nothing else (disconnect, deactivation) ends it.
#[tokio::test]
async fn subscribe_stream_closes_when_jwt_expires_midstream() {
    use futures::StreamExt;

    let server = crate::common::TestServer::start().await;
    // A real, active user holding the baseline subscribe gate.
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_jwt_exp",
        &["profile::read"],
    )
    .await;

    // Mint a SHORT-exp (4s) access token for THIS user, signed with the
    // TestServer's JWT secret + iss/aud (harness_inner.rs) so both the auth
    // extractor and the handler's `validate_access_token(...)` accept it.
    // username/email are not validated (only signature + iss/aud + exp), so
    // they can be empty — the user is loaded from `sub`.
    #[derive(serde::Serialize)]
    struct ShortClaims {
        sub: String,
        exp: i64,
        iat: i64,
        iss: String,
        aud: String,
        username: String,
        email: String,
        is_admin: bool,
    }
    let now = chrono::Utc::now().timestamp();
    let claims = ShortClaims {
        sub: user.user_id.clone(),
        exp: now + 4,
        iat: now,
        iss: "ziee".into(),
        aud: "ziee-api".into(),
        username: String::new(),
        email: String::new(),
        is_admin: false,
    };
    let short_token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(
            b"test-secret-key-for-jwt-tokens-min-32-chars-long",
        ),
    )
    .expect("sign short-exp access token");

    let res = reqwest::Client::new()
        .get(server.api_url("/sync/subscribe"))
        .header("Authorization", format!("Bearer {short_token}"))
        .send()
        .await
        .expect("open the sync stream");
    assert_eq!(
        res.status(),
        200,
        "a still-valid (near-expiry) token must open the stream"
    );

    let mut stream = res.bytes_stream();

    // First frame must be the `connected` handshake — proves the stream really
    // opened before we assert it closes.
    let first = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
        .await
        .expect("handshake frame within 5s")
        .expect("stream yielded a frame")
        .expect("frame is Ok");
    assert!(
        String::from_utf8_lossy(&first).contains("connected"),
        "expected the `connected` handshake as the first SSE frame"
    );

    // Drain until the server closes the stream at the exp deadline. With a 4s
    // token exp the close lands well inside 30s; a regression that drops the
    // exp-deadline `select!` arm would leave the stream open (keep-alive pings)
    // indefinitely → this timeout fires and the test fails instead of hanging.
    let closed = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while let Some(chunk) = stream.next().await {
            let _ = chunk; // ignore keep-alive comments / buffered frames until EOF
        }
    })
    .await;
    assert!(
        closed.is_ok(),
        "the SSE stream must close once the JWT exp deadline passes; it stayed open >30s"
    );
}
