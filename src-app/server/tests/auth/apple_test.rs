//! Apple Sign In integration tests, driven against a wiremock-based
//! AppleMockServer that emulates `appleid.apple.com`'s wire behavior:
//!   - GET  /auth/keys  → JWKS containing our mock RSA public key
//!   - POST /auth/token → token response with a synthetic id_token
//!                        signed by the mock's RSA private key
//!
//! The mock cannot accept ES256 client_secret JWTs the way real Apple
//! does (Apple validates the team_id + key_id + signature against
//! their key registry); our mock just accepts any POST. We
//! separately unit-test that we GENERATE the right ES256 JWT.
//!
//! Strategy per test:
//!   1. AppleMockServer::start() — wiremock instance + RSA keypair
//!   2. Seed provider row with `base_url` pointing at the mock + the
//!      fixture .p8 key path
//!   3. GET /api/auth/oauth/apple/authorize  → 307 to mock with state+nonce
//!   4. Extract state + nonce from the redirect URL's query params
//!   5. apple_mock.sign_id_token(claims with matching nonce)
//!   6. apple_mock.queue_token_response(signed_jwt)
//!   7. POST /api/auth/oauth/apple/callback (form-encoded) with state+code
//!   8. Assert outcome (new user provisioned, JWT redirect, etc.)

use crate::common::apple_mock::AppleMockServer;
use chrono::Utc;
use serde_json::json;

/// Helper: seed an Apple auth_providers row pointing at the mock.
/// The default seeded `apple` row (migration 47) means tests must
/// pick a different name like `apple-test` to avoid the unique-name
/// constraint.
async fn seed_apple_provider(
    pool: &sqlx::PgPool,
    name: &str,
    services_id: &str,
    apple_mock: &AppleMockServer,
) -> uuid::Uuid {
    let key_path = AppleMockServer::fixture_p8_path();
    let config = json!({
        "team_id": "TESTTEAM12",
        "services_id": services_id,
        "key_id": "TESTKEYID1",
        "private_key_path": key_path.to_string_lossy(),
        "scopes": ["name", "email"],
        "base_url": apple_mock.base_url,
    });
    sqlx::query!(
        r#"INSERT INTO auth_providers (name, provider_type, config, enabled)
           VALUES ($1, 'apple', $2, true)"#,
        name,
        config,
    )
    .execute(pool)
    .await
    .expect("Failed to seed apple provider");
    sqlx::query_scalar!(
        r#"SELECT id FROM auth_providers WHERE name = $1"#,
        name
    )
    .fetch_one(pool)
    .await
    .expect("Failed to read provider id")
}

/// Helper: hit /authorize on our server, follow no redirects, extract
/// state and nonce from the Location header. The Location is the
/// mock's URL with `state=` and `nonce=` query params our code injected.
async fn init_apple_flow(
    test_server: &crate::common::TestServer,
    provider_name: &str,
) -> (String, String) {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let url = format!(
        "{}/api/auth/oauth/{}/authorize",
        test_server.base_url, provider_name
    );
    let resp = client.get(&url).send().await.unwrap();
    assert_eq!(resp.status(), 307, "Apple /authorize should 307 to provider");
    let loc = resp
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    // Parse out state + nonce from the query string.
    let parsed = url::Url::parse(&loc).expect("Valid redirect URL");
    let mut state = String::new();
    let mut nonce = String::new();
    for (k, v) in parsed.query_pairs() {
        if k == "state" {
            state = v.into_owned();
        } else if k == "nonce" {
            nonce = v.into_owned();
        }
    }
    assert!(!state.is_empty(), "state must be present in redirect");
    assert!(!nonce.is_empty(), "nonce must be present in redirect");
    (state, nonce)
}

/// Apple → us POST callback. The form body is the union of what Apple
/// sends in `response_mode=form_post`: `code`, `state`, `id_token`,
/// optional `user` (first-auth-only JSON string).
async fn post_apple_callback(
    test_server: &crate::common::TestServer,
    provider_name: &str,
    code: &str,
    state: &str,
    user_json: Option<&str>,
) -> reqwest::Response {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let url = format!(
        "{}/api/auth/oauth/{}/callback",
        test_server.base_url, provider_name
    );
    let mut form: Vec<(&str, &str)> = vec![("code", code), ("state", state)];
    if let Some(u) = user_json {
        form.push(("user", u));
    }
    client.post(&url).form(&form).send().await.unwrap()
}

/// Happy path: brand-new Apple user, no email collision, auto-provision.
#[tokio::test]
async fn test_apple_first_login_auto_provisions_user() {
    let test_server = crate::common::TestServer::start().await;
    let apple_mock = AppleMockServer::start().await;
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .unwrap();
    let services_id = "com.example.app";
    let provider_id =
        seed_apple_provider(&pool, "apple-test", services_id, &apple_mock).await;

    let (state, nonce) = init_apple_flow(&test_server, "apple-test").await;

    let now = Utc::now().timestamp();
    let id_token = apple_mock.sign_id_token(&json!({
        "iss": apple_mock.base_url,
        "aud": services_id,
        "sub": "001234.aaaaaaaaaaaaaaaaaaaaaaaaaaaa.5678",
        "iat": now,
        "exp": now + 3600,
        "email": "tester@privaterelay.appleid.com",
        "email_verified": "true",        // <-- Apple's string-not-bool quirk
        "is_private_email": "true",
        "nonce": nonce,
    }));
    apple_mock.queue_token_response(&id_token).await;

    let resp = post_apple_callback(
        &test_server,
        "apple-test",
        "test-apple-code",
        &state,
        Some(r#"{"name":{"firstName":"Test","lastName":"User"},"email":"tester@privaterelay.appleid.com"}"#),
    )
    .await;

    let status = resp.status();
    let loc = resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let body_for_debug = if !status.is_redirection() {
        resp.text().await.unwrap_or_default()
    } else {
        String::new()
    };
    assert!(
        status.is_redirection(),
        "Callback should redirect on success, got {} loc={:?} body={}",
        status,
        loc,
        body_for_debug
    );
    let loc = loc.unwrap();
    assert!(
        loc.starts_with("/auth/callback#token="),
        "Should hand off to /auth/callback, got: {}",
        loc
    );

    // User + auth_link created.
    let row = sqlx::query!(
        r#"SELECT u.username, u.email, u.display_name, l.external_id, l.external_email
           FROM users u
           JOIN user_auth_links l ON l.user_id = u.id
           WHERE l.provider_id = $1"#,
        provider_id,
    )
    .fetch_one(&pool)
    .await
    .expect("Apple auto-provisioned user + link should exist");
    assert_eq!(
        row.external_id,
        "001234.aaaaaaaaaaaaaaaaaaaaaaaaaaaa.5678"
    );
    assert_eq!(
        row.external_email.as_deref(),
        Some("tester@privaterelay.appleid.com")
    );
    // user JSON merged display_name on first auth.
    assert_eq!(row.display_name.as_deref(), Some("Test User"));
}

/// Second login with the same Apple `sub` → existing-link path,
/// no duplicate user. Apple sends NO `user` JSON on the second auth.
#[tokio::test]
async fn test_apple_second_login_reuses_existing_user() {
    let test_server = crate::common::TestServer::start().await;
    let apple_mock = AppleMockServer::start().await;
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .unwrap();
    let services_id = "com.example.repeat";
    let provider_id =
        seed_apple_provider(&pool, "apple-test", services_id, &apple_mock).await;

    // First login — create the user via the same auto-provision path.
    let (state1, nonce1) = init_apple_flow(&test_server, "apple-test").await;
    let now = Utc::now().timestamp();
    let id_token1 = apple_mock.sign_id_token(&json!({
        "iss": apple_mock.base_url,
        "aud": services_id,
        "sub": "001234.repeat-user.5678",
        "iat": now,
        "exp": now + 3600,
        "email": "repeat@privaterelay.appleid.com",
        "email_verified": "true",
        "nonce": nonce1,
    }));
    apple_mock.queue_token_response(&id_token1).await;
    let r1 = post_apple_callback(
        &test_server,
        "apple-test",
        "code1",
        &state1,
        Some(r#"{"name":{"firstName":"Re","lastName":"Peat"},"email":"repeat@privaterelay.appleid.com"}"#),
    )
    .await;
    assert!(r1.status().is_redirection());

    // Second login — same sub, NO user JSON (Apple-correct).
    let (state2, nonce2) = init_apple_flow(&test_server, "apple-test").await;
    let now = Utc::now().timestamp();
    let id_token2 = apple_mock.sign_id_token(&json!({
        "iss": apple_mock.base_url,
        "aud": services_id,
        "sub": "001234.repeat-user.5678",
        "iat": now,
        "exp": now + 3600,
        "email": "repeat@privaterelay.appleid.com",
        "email_verified": "true",
        "nonce": nonce2,
    }));
    apple_mock.queue_token_response(&id_token2).await;
    let r2 = post_apple_callback(&test_server, "apple-test", "code2", &state2, None).await;
    assert!(r2.status().is_redirection());

    // Exactly one user, exactly one link.
    let user_count: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "c!" FROM user_auth_links WHERE provider_id = $1"#,
        provider_id,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(user_count, 1, "Second login must NOT create a new user");
}

/// Bad nonce → callback fails (signature verifies but nonce mismatch).
/// Protects against replayed id_tokens from other sessions.
#[tokio::test]
async fn test_apple_callback_rejects_nonce_mismatch() {
    let test_server = crate::common::TestServer::start().await;
    let apple_mock = AppleMockServer::start().await;
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .unwrap();
    let services_id = "com.example.nonce";
    seed_apple_provider(&pool, "apple-test", services_id, &apple_mock).await;

    let (state, _real_nonce) = init_apple_flow(&test_server, "apple-test").await;

    let now = Utc::now().timestamp();
    let id_token = apple_mock.sign_id_token(&json!({
        "iss": apple_mock.base_url,
        "aud": services_id,
        "sub": "001234.nonce-bad.5678",
        "iat": now,
        "exp": now + 3600,
        "email": "nonce@example.com",
        "email_verified": "true",
        "nonce": "DEFINITELY_NOT_THE_RIGHT_NONCE",
    }));
    apple_mock.queue_token_response(&id_token).await;
    let resp = post_apple_callback(&test_server, "apple-test", "code", &state, None).await;
    assert_eq!(resp.status(), 401, "Nonce mismatch must be 401");
}

/// Bad signature (different key) → callback fails JWKS verification.
#[tokio::test]
async fn test_apple_callback_rejects_bad_signature() {
    let test_server = crate::common::TestServer::start().await;
    let apple_mock = AppleMockServer::start().await;
    // SECOND mock with a DIFFERENT key — we use this to sign the token,
    // then queue it on the first mock. Result: signature won't verify
    // against the first mock's JWKS.
    let foreign_mock = AppleMockServer::start().await;
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .unwrap();
    let services_id = "com.example.badsig";
    seed_apple_provider(&pool, "apple-test", services_id, &apple_mock).await;

    let (state, nonce) = init_apple_flow(&test_server, "apple-test").await;

    let now = Utc::now().timestamp();
    let id_token = foreign_mock.sign_id_token(&json!({
        "iss": apple_mock.base_url,
        "aud": services_id,
        "sub": "001234.bad-sig.5678",
        "iat": now,
        "exp": now + 3600,
        "email": "bad@example.com",
        "email_verified": "true",
        "nonce": nonce,
    }));
    apple_mock.queue_token_response(&id_token).await;
    let resp = post_apple_callback(&test_server, "apple-test", "code", &state, None).await;
    assert_eq!(resp.status(), 401, "Bad signature must be 401");
}

/// Test_connection on the admin /test endpoint should succeed when
/// JWKS is reachable + the .p8 key is valid (sign client_secret JWT works).
#[tokio::test]
async fn test_apple_test_connection_succeeds_for_valid_config() {
    let test_server = crate::common::TestServer::start().await;
    let apple_mock = AppleMockServer::start().await;
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .unwrap();
    let provider_id =
        seed_apple_provider(&pool, "apple-test", "com.example.test-conn", &apple_mock).await;

    // Need an admin user + token to hit the admin endpoint.
    let setup_body = json!({
        "username": "rootadmin",
        "email": "root@example.com",
        "password": "ComplexPass1!"
    });
    let client = reqwest::Client::new();
    let setup = client
        .post(test_server.api_url("/app/setup/admin"))
        .json(&setup_body)
        .send()
        .await
        .unwrap();
    assert_eq!(setup.status(), 201);
    let body: serde_json::Value = setup.json().await.unwrap();
    let token = body["access_token"].as_str().unwrap().to_string();

    let resp = client
        .post(test_server.api_url(&format!(
            "/admin/auth-providers/{}/test",
            provider_id
        )))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["ok"], json!(true), "test_connection must succeed");
}

/// SECURITY: when the signed id_token already carries a verified email, an
/// attacker-supplied `user` JSON email (POST body, NOT signed) must be IGNORED
/// — otherwise a captured (code, state) pair could rebind a victim's email.
#[tokio::test]
async fn test_apple_user_json_email_cannot_override_verified_id_token_email() {
    let test_server = crate::common::TestServer::start().await;
    let apple_mock = AppleMockServer::start().await;
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .unwrap();
    let services_id = "com.example.app";
    let provider_id =
        seed_apple_provider(&pool, "apple-spoof", services_id, &apple_mock).await;

    let (state, nonce) = init_apple_flow(&test_server, "apple-spoof").await;
    let now = Utc::now().timestamp();
    let id_token = apple_mock.sign_id_token(&json!({
        "iss": apple_mock.base_url,
        "aud": services_id,
        "sub": "001234.spoofspoofspoofspoofspoof.0001",
        "iat": now,
        "exp": now + 3600,
        "email": "victim@privaterelay.appleid.com",   // verified by the id_token
        "email_verified": "true",
        "is_private_email": "true",
        "nonce": nonce,
    }));
    apple_mock.queue_token_response(&id_token).await;

    // Attacker supplies a different email in the unsigned `user` blob.
    let resp = post_apple_callback(
        &test_server,
        "apple-spoof",
        "spoof-code",
        &state,
        Some(r#"{"name":{"firstName":"A","lastName":"B"},"email":"attacker@evil.com"}"#),
    )
    .await;
    assert!(resp.status().is_redirection(), "callback should still succeed");

    let row = sqlx::query!(
        r#"SELECT l.external_email
           FROM user_auth_links l WHERE l.provider_id = $1"#,
        provider_id,
    )
    .fetch_one(&pool)
    .await
    .expect("link should exist");
    assert_eq!(
        row.external_email.as_deref(),
        Some("victim@privaterelay.appleid.com"),
        "the verified id_token email must win; the unsigned user-JSON email must be ignored"
    );
}

/// First-login-only private-relay path: when the id_token carries NO email,
/// a relay-shaped email from the `user` JSON IS accepted (Apple only sends the
/// `user` blob on the first authorization).
#[tokio::test]
async fn test_apple_user_json_relay_email_accepted_when_id_token_has_none() {
    let test_server = crate::common::TestServer::start().await;
    let apple_mock = AppleMockServer::start().await;
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .unwrap();
    let services_id = "com.example.app";
    let provider_id =
        seed_apple_provider(&pool, "apple-relay", services_id, &apple_mock).await;

    let (state, nonce) = init_apple_flow(&test_server, "apple-relay").await;
    let now = Utc::now().timestamp();
    // id_token deliberately omits `email`.
    let id_token = apple_mock.sign_id_token(&json!({
        "iss": apple_mock.base_url,
        "aud": services_id,
        "sub": "001234.relayrelayrelayrelay.0002",
        "iat": now,
        "exp": now + 3600,
        "nonce": nonce,
    }));
    apple_mock.queue_token_response(&id_token).await;

    let resp = post_apple_callback(
        &test_server,
        "apple-relay",
        "relay-code",
        &state,
        Some(r#"{"name":{"firstName":"R","lastName":"E"},"email":"newuser@privaterelay.appleid.com"}"#),
    )
    .await;
    assert!(resp.status().is_redirection(), "callback should succeed");

    let row = sqlx::query!(
        r#"SELECT l.external_email
           FROM user_auth_links l WHERE l.provider_id = $1"#,
        provider_id,
    )
    .fetch_one(&pool)
    .await
    .expect("link should exist");
    assert_eq!(
        row.external_email.as_deref(),
        Some("newuser@privaterelay.appleid.com"),
        "a relay email from the first-auth user JSON must be accepted when the id_token had none"
    );
}

// audit id all-381d7fed6bb1 — Apple sends the user-JSON blob (name) ONLY on the
// FIRST authorization. A relogin that re-submits a FORGED user JSON must NOT be
// allowed to overwrite the established display_name — identity is the id_token
// `sub`, the form `user` blob is first-auth-only. Security path untested.
#[tokio::test]
async fn test_apple_relogin_forged_user_json_does_not_overwrite_name() {
    let test_server = crate::common::TestServer::start().await;
    let apple_mock = AppleMockServer::start().await;
    let pool = sqlx::PgPool::connect(&test_server.database_url).await.unwrap();
    let services_id = "com.example.forge";
    let provider_id = seed_apple_provider(&pool, "apple-test", services_id, &apple_mock).await;
    let sub = "001234.forge-user.9999";

    // First login establishes display_name = "Real Name".
    let (state1, nonce1) = init_apple_flow(&test_server, "apple-test").await;
    let now = Utc::now().timestamp();
    apple_mock
        .queue_token_response(&apple_mock.sign_id_token(&json!({
            "iss": apple_mock.base_url, "aud": services_id, "sub": sub,
            "iat": now, "exp": now + 3600,
            "email": "forge@privaterelay.appleid.com", "email_verified": "true", "nonce": nonce1,
        })))
        .await;
    let r1 = post_apple_callback(
        &test_server, "apple-test", "code1", &state1,
        Some(r#"{"name":{"firstName":"Real","lastName":"Name"},"email":"forge@privaterelay.appleid.com"}"#),
    )
    .await;
    assert!(r1.status().is_redirection());

    // Second login (same sub) with a FORGED user JSON claiming a different name.
    let (state2, nonce2) = init_apple_flow(&test_server, "apple-test").await;
    let now = Utc::now().timestamp();
    apple_mock
        .queue_token_response(&apple_mock.sign_id_token(&json!({
            "iss": apple_mock.base_url, "aud": services_id, "sub": sub,
            "iat": now, "exp": now + 3600,
            "email": "forge@privaterelay.appleid.com", "email_verified": "true", "nonce": nonce2,
        })))
        .await;
    let r2 = post_apple_callback(
        &test_server, "apple-test", "code2", &state2,
        Some(r#"{"name":{"firstName":"Hacker","lastName":"Override"},"email":"forge@privaterelay.appleid.com"}"#),
    )
    .await;
    assert!(r2.status().is_redirection());

    // The display_name must remain the first-auth value, NOT the forged one.
    let row = sqlx::query!(
        r#"SELECT u.display_name FROM users u
           JOIN user_auth_links l ON l.user_id = u.id WHERE l.provider_id = $1"#,
/// SECURITY — first-time-only `user` JSON must NOT clobber a stored profile.
///
/// Apple sends the `user` form field (name/email) ONLY on the user's FIRST
/// authorization. A SECOND callback for the same `sub` that nonetheless
/// carries a `user` JSON — whether a benign client replay or an attacker
/// forging the form field to overwrite a victim's display name — must be
/// ignored: the existing-link branch issues a JWT off the `sub` mapping and
/// never writes the merged name back. This asserts the no-clobber contract
/// the `reuses_existing_user` test (which only checks the row count) leaves
/// unguarded.
#[tokio::test]
async fn test_apple_second_login_user_json_does_not_clobber_profile() {
    let test_server = crate::common::TestServer::start().await;
    let apple_mock = AppleMockServer::start().await;
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .unwrap();
    let services_id = "com.example.noclobber";
    let provider_id =
        seed_apple_provider(&pool, "apple-test", services_id, &apple_mock).await;

    let sub = "001234.no-clobber-user.5678";

    // ── First login: user JSON establishes display_name "Real Name". ──
    let (state1, nonce1) = init_apple_flow(&test_server, "apple-test").await;
    let now = Utc::now().timestamp();
    let id_token1 = apple_mock.sign_id_token(&json!({
        "iss": apple_mock.base_url,
        "aud": services_id,
        "sub": sub,
        "iat": now,
        "exp": now + 3600,
        "email": "victim@privaterelay.appleid.com",
        "email_verified": "true",
        "is_private_email": "true",
        "nonce": nonce1,
    }));
    apple_mock.queue_token_response(&id_token1).await;
    let r1 = post_apple_callback(
        &test_server,
        "apple-test",
        "code1",
        &state1,
        Some(r#"{"name":{"firstName":"Real","lastName":"Name"},"email":"victim@privaterelay.appleid.com"}"#),
    )
    .await;
    assert!(r1.status().is_redirection(), "first login should succeed");

    let first = sqlx::query!(
        r#"SELECT u.id, u.display_name, u.email, l.external_email
           FROM users u JOIN user_auth_links l ON l.user_id = u.id
           WHERE l.provider_id = $1 AND l.external_id = $2"#,
        provider_id,
        sub,
    )
    .fetch_one(&pool)
    .await
    .expect("user provisioned on first login");
    assert_eq!(first.display_name.as_deref(), Some("Real Name"));

    // ── Second login: SAME sub, but the callback carries an ANOMALOUS
    //    user JSON forging a different name (Apple never resends it). ──
    let (state2, nonce2) = init_apple_flow(&test_server, "apple-test").await;
    let now = Utc::now().timestamp();
    let id_token2 = apple_mock.sign_id_token(&json!({
        "iss": apple_mock.base_url,
        "aud": services_id,
        "sub": sub,
        "iat": now,
        "exp": now + 3600,
        "email": "victim@privaterelay.appleid.com",
        "email_verified": "true",
        "is_private_email": "true",
        "nonce": nonce2,
    }));
    apple_mock.queue_token_response(&id_token2).await;
    let r2 = post_apple_callback(
        &test_server,
        "apple-test",
        "code2",
        &state2,
        Some(r#"{"name":{"firstName":"Evil","lastName":"Overwrite"},"email":"attacker@evil.example"}"#),
    )
    .await;
    assert!(r2.status().is_redirection(), "second login should authenticate");

    // ── No new user, and the stored profile is untouched. ──
    let count: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "c!" FROM user_auth_links WHERE provider_id = $1"#,
        provider_id,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        row.display_name.as_deref(),
        Some("Real Name"),
        "a forged relogin user JSON must NOT overwrite the established display_name"
    assert_eq!(count, 1, "second login must not create a second link/user");

    let after = sqlx::query!(
        r#"SELECT u.display_name, u.email, l.external_email
           FROM users u JOIN user_auth_links l ON l.user_id = u.id
           WHERE l.provider_id = $1 AND l.external_id = $2"#,
        provider_id,
        sub,
    )
    .fetch_one(&pool)
    .await
    .expect("same user resolved on second login");
    assert_eq!(
        after.display_name.as_deref(),
        Some("Real Name"),
        "a forged second-auth user JSON must NOT overwrite the stored display_name"
    );
    assert_eq!(
        first.id,
        sqlx::query_scalar!(
            r#"SELECT u.id FROM users u JOIN user_auth_links l ON l.user_id = u.id
               WHERE l.provider_id = $1 AND l.external_id = $2"#,
            provider_id,
            sub,
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        "second login must resolve the SAME user id",
    );
}
