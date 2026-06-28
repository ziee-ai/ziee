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
/// File sync end-to-end through the SSE stream: uploading a file fires
/// `publish_file_changed` (owner-scoped `File`/`Update`), and the uploader's own
/// sync subscription receives a `file`/`update` frame carrying the file id. The
/// generic subscribe test above only proves the stream opens; this proves a
/// file-specific entity is actually delivered over it.
#[tokio::test]
async fn upload_delivers_file_sync_event_to_owner() {
    use crate::common::sync_probe::SyncProbe;
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
        "sync_cap",
        &["profile::read"],
    )
    .await;
    let client = reqwest::Client::new();

    // Hold the max number of concurrent streams open (kept in scope so the
    // underlying connections stay registered server-side).
    let mut held = Vec::new();
    for i in 0..PER_USER_MAX_CONNECTIONS {
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

/// The subscribe handler enforces a PER-USER connection cap at connect time
/// (`registry.rs` `PER_USER_MAX_CONNECTIONS` = 12 concurrent SSE streams per
/// account): the (cap+1)th `GET /sync/subscribe` for the SAME user is refused
/// with `429 SYNC_USER_LIMIT`. The registry unit test exercises `register()`
/// directly; this proves the cap is surfaced through the real HTTP handler.
/// The cap is keyed on this fresh user's id, so it is isolated from any other
/// test's connections in the process-wide registry.
#[tokio::test]
async fn subscribe_refuses_excess_connections_for_one_user_with_429() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_cap_user",
        &["profile::read"],
    )
    .await;

    let client = reqwest::Client::new();
    // The per-user cap. Hold these responses ALIVE so their server-side
    // connections stay registered (dropping a response closes the stream →
    // ConnGuard unregisters). Each `send()` returns only after the handler has
    // already run `register()`, so by the 12th success all 12 are registered.
    const PER_USER_MAX: usize = 12;
    let mut held = Vec::with_capacity(PER_USER_MAX);
    for i in 0..PER_USER_MAX {
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
        assert_eq!(
            res.status(),
            200,
            "connection {i} (within the cap) must be accepted"
            "sync connection #{} (under the per-user cap) must open",
            i + 1
        );
        held.push(res);
    }

    // The next subscribe for the SAME user exceeds the per-user cap → 429.
    let over = client
    // The (cap+1)th concurrent connection for the SAME user must be refused.
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
        over.status(),
        429,
        "subscribe beyond the per-user connection cap must be rejected with 429"
    );

    // Keep the held streams alive until the assertion above has run.
        overflow.status(),
        429,
        "the (cap+1)th concurrent sync stream for one user must be refused (SYNC_USER_LIMIT)"
    );
    let body = overflow.text().await.unwrap_or_default();
    assert!(
        body.contains("SYNC_USER_LIMIT") || body.contains("Too many open sync connections"),
        "the 429 body should carry the SYNC_USER_LIMIT error, got: {body}"
    );

    // Drop the held responses → closes the 12 streams → ConnGuard unregisters
    // each, leaving the process-wide registry clean for sibling tests.
    drop(held);
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
        overflow.status(),
        429,
        "the (cap+1)th connection must be refused with 429"
    );

    drop(held);
}
