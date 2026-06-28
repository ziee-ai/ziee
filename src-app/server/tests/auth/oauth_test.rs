/// OAuth2/OIDC Provider Integration Tests
///
/// These tests use testcontainers to automatically spawn OAuth mock servers.
/// Docker will be started automatically if not already running.
use crate::common::oauth_mock::OAuthMockServer;
use serde_json::json;

/// Test that we can start an OAuth mock server and connect to it
#[tokio::test]
async fn test_oauth_mock_server_connectivity() {
    let oauth_server = OAuthMockServer::start()
        .await
        .expect("Failed to start OAuth mock server");

    // Test that we can fetch the OpenID configuration
    let client = reqwest::Client::new();
    let response = client
        .get(oauth_server.well_known_url())
        .send()
        .await
        .expect("Failed to fetch well-known config");

    assert!(response.status().is_success());

    let config: serde_json::Value = response.json().await.expect("Invalid JSON");
    assert_eq!(config["issuer"], oauth_server.issuer_url);
}

/// Test creating an OAuth provider in the database
#[tokio::test]
async fn test_create_oauth_provider() {
    let test_server = crate::common::TestServer::start().await;
    let oauth_server = OAuthMockServer::start()
        .await
        .expect("Failed to start OAuth mock server");

    // Create OAuth provider configuration
    let provider_config = oauth_server.create_test_provider_config("test-client", "test-secret");

    // Insert provider into database
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .expect("Failed to connect to test database");

    let provider_id = sqlx::query_scalar!(
        r#"
        INSERT INTO auth_providers (name, provider_type, config, enabled)
        VALUES ($1, $2, $3, $4)
        RETURNING id
        "#,
        "test-oauth",
        "oauth2",
        provider_config,
        true
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create OAuth provider");

    assert!(!provider_id.is_nil());

    // Verify we can retrieve it
    let retrieved = sqlx::query!(
        r#"
        SELECT name, provider_type, config, enabled
        FROM auth_providers
        WHERE id = $1
        "#,
        provider_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to retrieve provider");

    assert_eq!(retrieved.name, "test-oauth");
    assert_eq!(retrieved.provider_type, "oauth2");
    assert!(retrieved.enabled);
}

/// Test complete OAuth authorization flow through our application
///
/// This test performs a full end-to-end OAuth flow:
/// 1. Start our application with OAuth provider configured
/// 2. Hit OUR /api/auth/oauth/{provider}/authorize endpoint
/// 3. Follow redirect to mock OAuth server
/// 4. Submit login form on mock server
/// 5. Follow redirect back to OUR /api/auth/oauth/{provider}/callback
/// 6. Verify OUR application returns JWT tokens
#[tokio::test]
async fn test_oauth_authorization_flow() {
    let test_server = crate::common::TestServer::start().await;
    let oauth_server = OAuthMockServer::start()
        .await
        .expect("Failed to start OAuth mock server");

    // Step 1: Create OAuth provider in database pointing to mock server
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .expect("Failed to connect to test database");

    let provider_config = serde_json::json!({
        "client_id": "test-client",
        "client_secret": "test-secret",
        "authorization_url": oauth_server.authorize_url(),
        "token_url": oauth_server.token_url(),
        "issuer_url": oauth_server.issuer_url,
        "scopes": ["openid", "profile", "email"],
        "attribute_mapping": {
            "user_id": "sub",
            "username": "sub",  // Use sub as username since mock server always returns this
            "email": "sub",     // Fallback to sub for email as well
            "display_name": "sub"
        }
    });

    sqlx::query!(
        r#"
        INSERT INTO auth_providers (name, provider_type, config, enabled)
        VALUES ($1, $2, $3, $4)
        "#,
        "test-oauth",
        "oauth2",
        provider_config,
        true
    )
    .execute(&pool)
    .await
    .expect("Failed to create OAuth provider");

    // Get the provider ID we just created
    let provider_id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM auth_providers WHERE name = $1")
            .bind("test-oauth")
            .fetch_one(&pool)
            .await
            .expect("Failed to get provider ID");

    // Create a test user
    let user_id = uuid::Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO users (id, username, email, is_active, is_admin, created_at, updated_at)
        VALUES ($1, $2, $3, true, false, NOW(), NOW())
        "#,
        user_id,
        "testuser",
        "testuser@example.com"
    )
    .execute(&pool)
    .await
    .expect("Failed to create test user");

    // Link the user to the OAuth provider
    // The mock OAuth server will use "testuser" as the sub claim
    sqlx::query!(
        r#"
        INSERT INTO user_auth_links (user_id, provider_id, external_id, created_at)
        VALUES ($1, $2, $3, NOW())
        "#,
        user_id,
        provider_id,
        "testuser"
    )
    .execute(&pool)
    .await
    .expect("Failed to create user auth link");

    // Step 2: Initiate OAuth flow through OUR application
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none()) // Manual redirect handling
        .cookie_store(true)
        .build()
        .unwrap();

    let callback_url = format!(
        "{}/api/auth/oauth/test-oauth/callback",
        test_server.base_url
    );
    let authorize_url = format!(
        "{}/api/auth/oauth/test-oauth/authorize?redirect_uri={}",
        test_server.base_url,
        callback_url.replace("/", "%2F").replace(":", "%3A")
    );
    println!("Step 1: Initiating OAuth flow at: {}", authorize_url);

    let auth_response = client
        .get(&authorize_url)
        .send()
        .await
        .expect("Failed to initiate OAuth flow");

    // Our app should redirect to the mock OAuth server
    assert_eq!(
        auth_response.status(),
        307,
        "Should redirect to OAuth provider"
    );

    let oauth_auth_url = auth_response
        .headers()
        .get("location")
        .expect("Should have Location header")
        .to_str()
        .expect("Location should be valid string");

    println!("Step 2: Redirected to OAuth provider: {}", oauth_auth_url);
    assert!(
        oauth_auth_url.contains("response_type=code"),
        "Should be OAuth authorize URL"
    );

    // Step 3: Submit login form to mock OAuth server
    let oauth_response = client
        .post(oauth_auth_url)
        .form(&[("username", "testuser")])
        .send()
        .await
        .expect("Failed to submit OAuth login");

    // Mock server redirects back to our callback with code
    assert_eq!(oauth_response.status(), 302, "OAuth server should redirect");

    let callback_redirect = oauth_response
        .headers()
        .get("location")
        .expect("Should have callback URL")
        .to_str()
        .expect("Location should be valid string");

    println!(
        "Step 3: OAuth server redirected to callback: {}",
        callback_redirect
    );
    assert!(
        callback_redirect.contains("/api/auth/oauth/test-oauth/callback"),
        "Should redirect to our callback"
    );
    assert!(
        callback_redirect.contains("code="),
        "Should include authorization code"
    );

    // Step 4: Follow redirect to OUR callback endpoint
    let callback_response = client
        .get(callback_redirect)
        .send()
        .await
        .expect("Failed to hit callback endpoint");

    let status = callback_response.status();
    println!("Step 4: Callback response status: {}", status);

    // Our application returns a redirect to /?token={access_token}
    assert!(
        status.is_redirection(),
        "Callback should redirect with token"
    );

    let redirect_url = callback_response
        .headers()
        .get("location")
        .expect("Should have Location header")
        .to_str()
        .expect("Location should be valid string");

    println!("Step 5: Redirected to: {}", redirect_url);
    // 01-auth F-01 (Critical) closure: the access token now lives in
    // the URL FRAGMENT (`/#token=…`) so it isn't logged on the server,
    // sent as a Referer, or kept in browser history. Extract from the
    // fragment instead of the query.
    assert!(
        redirect_url.contains("#token="),
        "Should include access token in URL fragment (01-auth F-01)"
    );

    let access_token = redirect_url
        .split_once("#token=")
        .map(|(_, t)| {
            // Fragment may carry additional `&key=value` pairs after a
            // future addition; take only up to the next separator.
            t.split('&').next().unwrap_or(t).to_string()
        })
        .expect("Token not found in URL fragment");

    println!("✅ Complete OAuth flow successful!");
    println!("   User authenticated: testuser");
    println!("   Got JWT access token from our application");
    println!(
        "   Access token: {}...",
        &access_token[..20.min(access_token.len())]
    );
}

// ============================================================
// New-branch coverage — added with the OAuth social-login feature
// ============================================================
//
// These exercise the previously-unreachable branches in oauth_callback:
//   - auto-provisioning a brand-new user from social claims
//   - First-Broker-Link when an existing local email collides
//   - Microsoft `tid` allowlist (accept + reject paths)
//   - return_to round-trip through `oauth_sessions.return_to`
//
// The pattern is the same as `test_oauth_authorization_flow`:
//   1. seed an auth_providers row pointing at the navikt mock
//   2. GET our /authorize → follow 307 → POST navikt /authorize → follow
//      302 back to our /callback (with `code` + `state`)
//   3. assert the final redirect / status

use ziee::hash_password;

/// Seed an OIDC auth_providers row that points at the navikt mock.
/// `extra_config` is merged into the JSONB to test allowed_tenant_ids,
/// display_name, etc. without retyping the boilerplate.
async fn seed_oidc_provider(
    pool: &sqlx::PgPool,
    name: &str,
    oauth_server: &OAuthMockServer,
    extra_config: serde_json::Value,
) -> uuid::Uuid {
    let mut config = serde_json::json!({
        "client_id": "test-client",
        "client_secret": "test-secret",
        "authorization_url": oauth_server.authorize_url(),
        "token_url": oauth_server.token_url(),
        "issuer_url": oauth_server.issuer_url,
        "scopes": ["openid", "profile", "email"],
        "attribute_mapping": {
            "user_id": "sub",
            "username": "preferred_username",
            "email": "email",
            "display_name": "name"
        }
    });
    if let serde_json::Value::Object(extra) = extra_config {
        if let serde_json::Value::Object(target) = &mut config {
            for (k, v) in extra {
                target.insert(k, v);
            }
        }
    }
    sqlx::query!(
        r#"
        INSERT INTO auth_providers (name, provider_type, config, enabled)
        VALUES ($1, 'oidc', $2, true)
        "#,
        name,
        config,
    )
    .execute(pool)
    .await
    .expect("Failed to create OIDC provider");

    sqlx::query_scalar!(
        r#"SELECT id FROM auth_providers WHERE name = $1"#,
        name
    )
    .fetch_one(pool)
    .await
    .expect("Failed to read provider id")
}

/// Drive the navikt mock end-to-end through OUR /authorize+/callback,
/// returning the (final_status, final_location) from our callback.
/// `claims_json` is the JSON we POST to navikt's authorize so the
/// mock emits those claims in the id_token.
async fn drive_oauth_flow(
    test_server: &crate::common::TestServer,
    provider_name: &str,
    subject: &str,
    claims_json: serde_json::Value,
    return_to: Option<&str>,
) -> (reqwest::StatusCode, Option<String>) {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();

    let mut authorize_url = format!(
        "{}/api/auth/oauth/{}/authorize",
        test_server.base_url, provider_name
    );
    if let Some(rt) = return_to {
        authorize_url.push_str(&format!(
            "?return_to={}",
            url::form_urlencoded::byte_serialize(rt.as_bytes()).collect::<String>()
        ));
    }

    let our_authorize = client
        .get(&authorize_url)
        .send()
        .await
        .expect("Failed to initiate OAuth flow");
    assert_eq!(
        our_authorize.status(),
        307,
        "Our /authorize should 307 to provider"
    );
    let provider_authorize_url = our_authorize
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    let provider_response = client
        .post(&provider_authorize_url)
        .form(&[
            ("username", subject),
            ("claims", &claims_json.to_string()),
        ])
        .send()
        .await
        .expect("Failed to POST navikt /authorize");
    assert_eq!(
        provider_response.status(),
        302,
        "Provider should 302 back to our callback"
    );
    let callback_url = provider_response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    let callback_resp = client
        .get(&callback_url)
        .send()
        .await
        .expect("Failed to hit our callback");
    let status = callback_resp.status();
    let location = callback_resp
        .headers()
        .get("location")
        .and_then(|h| h.to_str().ok())
        .map(String::from);
    (status, location)
}

/// G1 path 3 — no existing link, no email collision → provision new user.
#[tokio::test]
async fn test_oauth_auto_provisioning_new_user() {
    let test_server = crate::common::TestServer::start().await;
    let oauth_server = OAuthMockServer::start()
        .await
        .expect("Failed to start OAuth mock server");
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .expect("Failed to connect to test database");
    let provider_id = seed_oidc_provider(&pool, "test-oauth", &oauth_server, json!({})).await;

    let (status, location) = drive_oauth_flow(
        &test_server,
        "test-oauth",
        "new-external-sub-abc",
        json!({
            "email": "newcomer@example.com",
            "email_verified": true,
            "preferred_username": "newcomer",
            "name": "New Comer"
        }),
        None,
    )
    .await;

    assert!(
        status.is_redirection(),
        "Callback should redirect on success, got {}",
        status
    );
    let loc = location.expect("Should have Location header");
    assert!(
        loc.starts_with("/auth/callback#token="),
        "Should redirect to /auth/callback with token fragment, got: {}",
        loc
    );

    // The user + link should now exist.
    let row = sqlx::query!(
        r#"SELECT u.id, u.username, u.email, l.external_id, l.external_email
           FROM users u
           JOIN user_auth_links l ON l.user_id = u.id
           WHERE l.provider_id = $1 AND l.external_id = $2"#,
        provider_id,
        "new-external-sub-abc"
    )
    .fetch_one(&pool)
    .await
    .expect("Auto-provisioned user + link should exist");
    assert_eq!(row.email.as_str(), "newcomer@example.com");
    assert_eq!(row.external_email.as_deref(), Some("newcomer@example.com"));
}

/// G1 path 2 — email collision with an existing local-password user
/// → First-Broker-Link. Server must NOT auto-link; instead it should
/// 302 to /auth/link-account?link_token=...
#[tokio::test]
async fn test_oauth_first_broker_link_redirects_to_confirm() {
    let test_server = crate::common::TestServer::start().await;
    let oauth_server = OAuthMockServer::start()
        .await
        .expect("Failed to start OAuth mock server");
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .expect("Failed to connect to test database");
    let provider_id = seed_oidc_provider(&pool, "test-oauth", &oauth_server, json!({})).await;

    // Pre-seed a LOCAL user whose email will collide with the
    // social-login email below. Must have a non-NULL password_hash —
    // FBL is only available to users who have a password to verify.
    let local_user_id = uuid::Uuid::new_v4();
    let pw_hash = hash_password("correct-horse-battery-staple").unwrap();
    sqlx::query!(
        r#"
        INSERT INTO users (id, username, email, password_hash, is_active, is_admin, created_at, updated_at)
        VALUES ($1, $2, $3, $4, true, false, NOW(), NOW())
        "#,
        local_user_id,
        "alice",
        "alice@example.com",
        pw_hash,
    )
    .execute(&pool)
    .await
    .expect("Failed to pre-seed local user");

    let (status, location) = drive_oauth_flow(
        &test_server,
        "test-oauth",
        "social-sub-alice",
        json!({
            "email": "alice@example.com",
            "email_verified": true,
            "preferred_username": "alice-social",
            "name": "Alice Social"
        }),
        None,
    )
    .await;

    assert_eq!(
        status, 307,
        "FBL collision should 307 (Redirect::temporary)"
    );
    let loc = location.expect("Should have Location header");
    assert!(
        loc.starts_with("/auth/link-account?link_token="),
        "Should redirect to /auth/link-account, got: {}",
        loc
    );

    // Crucial: no auth_link should have been created yet. Linking
    // is gated on the password-confirmation step.
    let link_count: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "c!" FROM user_auth_links
           WHERE provider_id = $1 AND user_id = $2"#,
        provider_id,
        local_user_id,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(link_count, 0, "No auth_link before password confirmation");

    // Pending link row should exist.
    let pending_count: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "c!" FROM pending_account_links
           WHERE target_user_id = $1"#,
        local_user_id,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pending_count, 1);
}

/// G1 + link_account — confirm with correct password → link created,
/// JWT issued. Wrong password → 401.
#[tokio::test]
async fn test_link_account_password_confirmation() {
    let test_server = crate::common::TestServer::start().await;
    let oauth_server = OAuthMockServer::start()
        .await
        .expect("Failed to start OAuth mock server");
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .expect("Failed to connect to test database");
    let provider_id = seed_oidc_provider(&pool, "test-oauth", &oauth_server, json!({})).await;

    let local_user_id = uuid::Uuid::new_v4();
    let pw_hash = hash_password("hunter2-is-still-bad").unwrap();
    sqlx::query!(
        r#"INSERT INTO users (id, username, email, password_hash, is_active, is_admin, created_at, updated_at)
           VALUES ($1, $2, $3, $4, true, false, NOW(), NOW())"#,
        local_user_id, "bob", "bob@example.com", pw_hash
    )
    .execute(&pool).await.unwrap();

    // Drive the flow to create the pending_link row.
    let (_, location) = drive_oauth_flow(
        &test_server,
        "test-oauth",
        "social-sub-bob",
        json!({
            "email": "bob@example.com",
            "email_verified": true,
            "preferred_username": "bob-social",
            "name": "Bob Social"
        }),
        None,
    )
    .await;
    let loc = location.expect("Location header");
    let link_token = loc
        .split_once("link_token=")
        .map(|(_, t)| t.split('&').next().unwrap_or(t).to_string())
        .expect("link_token in URL");
    // link_token is a UUID — no URL-encoding needed.

    let client = reqwest::Client::new();
    let link_endpoint = format!("{}/api/auth/link-account", test_server.base_url);

    // Wrong password → 401, link still not created.
    let bad = client
        .post(&link_endpoint)
        .json(&json!({ "link_token": &link_token, "password": "wrong" }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 401);
    let after_bad: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "c!" FROM user_auth_links WHERE provider_id = $1"#,
        provider_id,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after_bad, 0, "Wrong password must not create link");

    // Audit fix: link_account uses peek-then-delete so a wrong
    // password preserves the token for retry (forcing a fresh OAuth
    // dance on every typo would be hostile UX). Per-token brute-force
    // protection now comes from the `attempts` counter, capped at 5.
    let after_bad_pending: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "c!" FROM pending_account_links"#
    )
    .fetch_one(&pool).await.unwrap();
    assert_eq!(
        after_bad_pending, 1,
        "peek-then-delete: wrong-password attempt preserves the row for retry"
    );

    // Continue using the SAME link_token (no need for a second OAuth
    // dance now that the row survives the wrong-password attempt).
    let location2 = Some(format!("/auth/link-account?link_token={}", link_token));
    let _ = location2; // silence unused — we use link_token below
    // Correct password on the SAME token → 200 + link row created.
    // This proves peek-then-delete: the row survived the wrong-password
    // attempt, and we consume it now on success.
    let good = client
        .post(&link_endpoint)
        .json(&json!({
            "link_token": link_token,
            "password": "hunter2-is-still-bad"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(good.status(), 200);
    let after_good: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "c!" FROM user_auth_links WHERE provider_id = $1 AND user_id = $2"#,
        provider_id, local_user_id,
    )
    .fetch_one(&pool).await.unwrap();
    assert_eq!(after_good, 1, "Correct password must create the link");

    // The pending row is consumed on success.
    let after_good_pending: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "c!" FROM pending_account_links"#
    )
    .fetch_one(&pool).await.unwrap();
    assert_eq!(after_good_pending, 0, "successful link must consume the token");
}

/// G4a — single-tenant `allowed_tenant_ids` accepts matching tid.
#[tokio::test]
async fn test_oauth_tid_allowlist_accepts_matching() {
    let test_server = crate::common::TestServer::start().await;
    let oauth_server = OAuthMockServer::start()
        .await
        .expect("Failed to start OAuth mock server");
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .expect("Failed to connect to test database");
    let _provider_id = seed_oidc_provider(
        &pool,
        "test-oauth",
        &oauth_server,
        json!({
            "allowed_tenant_ids": ["good-tenant", "another-good-tenant"]
        }),
    )
    .await;

    let (status, location) = drive_oauth_flow(
        &test_server,
        "test-oauth",
        "ms-sub-1",
        json!({
            "email": "first@example.com",
            "email_verified": true,
            "preferred_username": "first",
            "tid": "good-tenant"
        }),
        None,
    )
    .await;
    assert!(
        status.is_redirection(),
        "tid in allowlist should succeed, got {} loc={:?}",
        status,
        location
    );
    let loc = location.unwrap();
    assert!(
        loc.starts_with("/auth/callback#token="),
        "Should issue JWT, got: {}",
        loc
    );
}

/// G4a — single-tenant `allowed_tenant_ids` rejects mismatching tid.
#[tokio::test]
async fn test_oauth_tid_allowlist_rejects_mismatch() {
    let test_server = crate::common::TestServer::start().await;
    let oauth_server = OAuthMockServer::start()
        .await
        .expect("Failed to start OAuth mock server");
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .expect("Failed to connect to test database");
    let _provider_id = seed_oidc_provider(
        &pool,
        "test-oauth",
        &oauth_server,
        json!({
            "allowed_tenant_ids": ["only-this-tenant"]
        }),
    )
    .await;

    let (status, _) = drive_oauth_flow(
        &test_server,
        "test-oauth",
        "ms-rejected-sub",
        json!({
            "email": "second@example.com",
            "email_verified": true,
            "preferred_username": "second",
            "tid": "the-wrong-tenant"
        }),
        None,
    )
    .await;
    assert_eq!(
        status, 401,
        "tid outside allowlist must be 401, got {}",
        status
    );
}

/// G3 — return_to query parameter must survive the round-trip through
/// the provider and arrive in the final fragment as `return_to=...`.
#[tokio::test]
async fn test_oauth_return_to_round_trip() {
    let test_server = crate::common::TestServer::start().await;
    let oauth_server = OAuthMockServer::start()
        .await
        .expect("Failed to start OAuth mock server");
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .expect("Failed to connect to test database");
    let _ = seed_oidc_provider(&pool, "test-oauth", &oauth_server, json!({})).await;

    let (status, location) = drive_oauth_flow(
        &test_server,
        "test-oauth",
        "rt-sub",
        json!({
            "email": "rt@example.com",
            "email_verified": true,
            "preferred_username": "rt"
        }),
        Some("/projects/42"),
    )
    .await;
    assert!(status.is_redirection());
    let loc = location.unwrap();
    assert!(
        loc.contains("return_to=%2Fprojects%2F42"),
        "return_to must be URL-encoded in fragment, got: {}",
        loc
    );
}

/// G3 — open-redirect protection: external return_to must be dropped.
#[tokio::test]
async fn test_oauth_return_to_rejects_open_redirect() {
    let test_server = crate::common::TestServer::start().await;
    let oauth_server = OAuthMockServer::start()
        .await
        .expect("Failed to start OAuth mock server");
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .expect("Failed to connect to test database");
    let _ = seed_oidc_provider(&pool, "test-oauth", &oauth_server, json!({})).await;

    let (status, location) = drive_oauth_flow(
        &test_server,
        "test-oauth",
        "evil-rt-sub",
        json!({
            "email": "evil@example.com",
            "email_verified": true,
            "preferred_username": "evil"
        }),
        Some("//evil.com/steal"),
    )
    .await;
    assert!(status.is_redirection());
    let loc = location.unwrap();
    // The validator rejected `//evil.com/steal`, so the final fragment
    // should carry `return_to=%2F` (the fallback "/").
    assert!(
        loc.contains("return_to=%2F&") || loc.ends_with("return_to=%2F"),
        "Open-redirect return_to must fall back to '/', got: {}",
        loc
    );
}

/// Security regression: an OAuth provider that EXPLICITLY says
/// `email_verified=false` is refused at the provider layer (the
/// existing F-09 defense — see oauth2.rs::handle_oauth_callback).
/// The whole flow must terminate with 401, NOT auto-provision and
/// NOT enter FBL.
#[tokio::test]
async fn test_oauth_unverified_email_is_rejected_at_provider_layer() {
    let test_server = crate::common::TestServer::start().await;
    let oauth_server = OAuthMockServer::start()
        .await
        .expect("Failed to start OAuth mock server");
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .expect("Failed to connect to test database");
    let _ = seed_oidc_provider(&pool, "test-oauth", &oauth_server, json!({})).await;

    // Pre-create a local user with a password — this is the "victim".
    // We want to make sure NO bind to this user occurs.
    let victim_pw = bcrypt::hash("victim-pw", bcrypt::DEFAULT_COST).unwrap();
    let victim_id = sqlx::query_scalar!(
        r#"
        INSERT INTO users (id, username, email, password_hash, is_active, is_admin, created_at, updated_at)
        VALUES (gen_random_uuid(), $1, $2, $3, true, false, NOW(), NOW())
        RETURNING id
        "#,
        "victim",
        "victim@example.com",
        victim_pw,
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Attacker drives an OAuth flow that claims `email=victim@example.com`
    // but with `email_verified=false`. The OIDC provider layer refuses
    // the whole exchange (closes F-09); a 401 + no DB writes is the
    // expected outcome.
    let (status, _location) = drive_oauth_flow(
        &test_server,
        "test-oauth",
        "attacker-sub",
        json!({
            "email": "victim@example.com",
            "email_verified": false,
            "preferred_username": "attacker"
        }),
        None,
    )
    .await;
    assert_eq!(
        status, 401,
        "unverified email must be rejected at the provider layer (F-09)"
    );

    // NO user_auth_links row was created against the victim.
    let bound_to_victim: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "c!" FROM user_auth_links WHERE user_id = $1"#,
        victim_id,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(bound_to_victim, 0, "victim must not be auto-linked");

    // No NEW user was auto-provisioned either (we didn't even get
    // to the provision branch).
    let attacker_link: Option<uuid::Uuid> = sqlx::query_scalar!(
        r#"SELECT user_id FROM user_auth_links WHERE external_id = $1"#,
        "attacker-sub",
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(
        attacker_link.is_none(),
        "no user_auth_link row should exist when email_verified=false"
    );
}

/// Audit fix: 5-attempt brute-force cap on link_account.
/// 5 wrong-password attempts on the same link_token should return
/// TOO_MANY_ATTEMPTS on the 6th and the token must be deleted.
#[tokio::test]
async fn test_link_account_brute_force_blocked_at_5_attempts() {
    let test_server = crate::common::TestServer::start().await;
    let oauth_server = OAuthMockServer::start()
        .await
        .expect("Failed to start OAuth mock server");
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .expect("Failed to connect to test database");
    let provider_id = seed_oidc_provider(&pool, "test-oauth", &oauth_server, json!({})).await;

    // Pre-create the local account that owns the email.
    let pw = bcrypt::hash("correct-horse", bcrypt::DEFAULT_COST).unwrap();
    sqlx::query!(
        r#"
        INSERT INTO users (id, username, email, password_hash, is_active, is_admin, created_at, updated_at)
        VALUES (gen_random_uuid(), $1, $2, $3, true, false, NOW(), NOW())
        "#,
        "victim", "victim@example.com", pw,
    ).execute(&pool).await.unwrap();

    // Drive OAuth → /auth/link-account?link_token=...
    let (_, location) = drive_oauth_flow(
        &test_server,
        "test-oauth",
        "social-sub",
        json!({"email":"victim@example.com","email_verified":true,"preferred_username":"victim-social"}),
        None,
    ).await;
    let loc = location.unwrap();
    let link_token = loc
        .split_once("link_token=")
        .map(|(_, t)| t.split('&').next().unwrap_or(t).to_string())
        .unwrap();
    let client = reqwest::Client::new();
    let endpoint = format!("{}/api/auth/link-account", test_server.base_url);

    // 5 wrong-password attempts: each returns 401.
    for i in 1..=5 {
        let resp = client
            .post(&endpoint)
            .json(&json!({"link_token": &link_token, "password": "wrong"}))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 401, "attempt {} expected 401", i);
    }

    // 6th attempt: 429 TOO_MANY_ATTEMPTS, and the token is deleted.
    let resp6 = client
        .post(&endpoint)
        .json(&json!({"link_token": &link_token, "password": "wrong"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp6.status(), 429, "6th attempt must be throttled");

    // 7th attempt with the CORRECT password: token is gone → 401.
    let resp7 = client
        .post(&endpoint)
        .json(&json!({"link_token": &link_token, "password": "correct-horse"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp7.status(), 401, "deleted token must reject even correct password");
    let _ = provider_id;
}

/// Audit fix: POST callback refused for non-Apple providers (CSRF
/// + cross-origin form_post attack surface).
#[tokio::test]
async fn test_oauth_post_callback_refused_for_non_apple_providers() {
    let test_server = crate::common::TestServer::start().await;
    let oauth_server = OAuthMockServer::start()
        .await
        .expect("Failed to start OAuth mock server");
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .expect("Failed to connect to test database");
    let _ = seed_oidc_provider(&pool, "test-oauth", &oauth_server, json!({})).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "{}/api/auth/oauth/test-oauth/callback",
            test_server.base_url
        ))
        .form(&[("code", "x"), ("state", "y")])
        .send()
        .await
        .unwrap();
    // Round-2 audit fix: collapse 404/405/307 enumeration into a
    // single 400 INVALID_STATE so attackers can't fingerprint which
    // providers are Apple.
    assert_eq!(resp.status(), 400, "POST against non-Apple must be 400");
}

/// Audit fix: "local" provider type is explicitly forbidden from
/// admin create — having two "local" providers leaves login routing
/// in undefined state.
#[tokio::test]
async fn test_admin_create_provider_refuses_local_type() {
    let test_server = crate::common::TestServer::start().await;
    // Bootstrap an admin via /app/setup/admin (matches the pattern
    // in admin_providers_test::make_admin).
    let client = reqwest::Client::new();
    let setup = client
        .post(test_server.api_url("/app/setup/admin"))
        .json(&json!({
            "username": "rootadmin",
            "email": "root@example.com",
            "password": "ComplexPass1!",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(setup.status(), 201);
    let token = setup
        .json::<serde_json::Value>()
        .await
        .unwrap()["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = client
        .post(test_server.api_url("/admin/auth-providers"))
        .bearer_auth(&token)
        .json(&json!({
            "name": "evil-local",
            "provider_type": "local",
            "enabled": true,
            "config": {}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400, "local provider creation must be refused");
}

/// Test that OAuth provider configuration validation works
#[tokio::test]
async fn test_oauth_provider_validation() {
    let oauth_server = OAuthMockServer::start()
        .await
        .expect("Failed to start OAuth mock server");

    // Test with valid configuration
    let valid_config = oauth_server.create_test_provider_config("valid-client", "valid-secret");

    assert!(valid_config["client_id"].is_string());
    assert!(valid_config["client_secret"].is_string());
    assert!(valid_config["auth_url"].is_string());
    assert!(valid_config["token_url"].is_string());
    assert!(valid_config["scopes"].is_array());

    // Test with invalid configuration (missing required fields)
    let invalid_config = json!({
        "client_id": "test"
        // Missing client_secret, urls, etc.
    });

    // In a real test, you would validate against your provider schema
    assert!(invalid_config.get("client_secret").is_none());
}

/// Error path: a callback whose `state` matches no oauth_sessions row (never
/// solicited, or the row already expired/was deleted) must collapse to a
/// neutral 400 INVALID_STATE — never a 500, and never a provider-existence
/// oracle.
#[tokio::test]
async fn test_oauth_callback_invalid_or_expired_state_returns_400() {
    let test_server = crate::common::TestServer::start().await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let resp = client
        .get(format!(
            "{}/api/auth/oauth/some-provider/callback?code=abc&state={}",
            test_server.base_url,
            uuid::Uuid::new_v4()
        ))
        .send()
        .await
        .expect("callback request failed");
    assert_eq!(
        resp.status(),
        400,
        "an unknown/expired state must return 400, got {}",
        resp.status()
    );
}

/// Error path: token exchange fails (e.g. wrong client_secret / unreachable
/// token endpoint). We point `token_url` at a closed loopback port so the
/// exchange errors; OUR callback must surface a non-success status rather than
/// panicking or redirecting to a logged-in session.
#[tokio::test]
async fn test_oauth_token_exchange_failure_is_handled() {
    let test_server = crate::common::TestServer::start().await;
    let oauth_server = OAuthMockServer::start()
        .await
        .expect("mock oauth server");
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .unwrap();

    // Real authorize_url (so the flow reaches our callback with a code) but a
    // dead token endpoint → the token POST fails.
    seed_oidc_provider(
        &pool,
        "test-oauth-tokenfail",
        &oauth_server,
        serde_json::json!({ "token_url": "http://127.0.0.1:1/token" }),
    )
    .await;

    let (status, _loc) = drive_oauth_flow(
        &test_server,
        "test-oauth-tokenfail",
        "tokfail-subject",
        serde_json::json!({ "sub": "tokfail-subject", "email": "tf@example.com" }),
        None,
    )
    .await;

    assert!(
        !status.is_redirection(),
        "a failed token exchange must NOT yield a successful login redirect, got {}",
        status
    );
    assert!(
        status.is_client_error() || status.is_server_error(),
        "token-exchange failure should surface an error status, got {}",
        status
    );
}

// audit id all-1c6ce6d8c014 — ensure_unique_username (handlers.rs:1350) dedups a
// derived username against existing rows by appending 2..=999, returning 500
// only if all are taken. Auto-provisioning covers the no-collision path; nothing
// exercised the COLLISION → suffix branch (the loop's actual behavior; the
// 999-exhaustion 500 is a degenerate tail that would need ~1000 colliding
// users). Here a pre-existing local user OCCUPIES the username the OAuth claims
// derive, so the auto-provisioned SSO user must get the deduped "<name>2".
#[tokio::test]
async fn test_oauth_auto_provision_dedups_colliding_username() {
    let test_server = crate::common::TestServer::start().await;
    let oauth_server = OAuthMockServer::start()
        .await
        .expect("Failed to start OAuth mock server");
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .expect("Failed to connect to test database");
    let provider_id = seed_oidc_provider(&pool, "test-oauth", &oauth_server, json!({})).await;

    // Pre-occupy the username "takenname" with a LOCAL user whose email differs
    // from the OAuth claim (so the flow takes auto-provision, not First-Broker-
    // Link on an email collision).
    sqlx::query!(
        r#"INSERT INTO users (id, username, email, is_active, is_admin, created_at, updated_at)
           VALUES ($1, 'takenname', 'taken-local@example.com', true, false, NOW(), NOW())"#,
        uuid::Uuid::new_v4(),
    )
    .execute(&pool)
    .await
    .expect("seed colliding local user");

    let (status, _location) = drive_oauth_flow(
        &test_server,
        "test-oauth",
        "dedup-external-sub",
        json!({
            "email": "dedup-newcomer@example.com",
            "email_verified": true,
            "preferred_username": "takenname",
            "name": "Dedup Newcomer"
        }),
        None,
    )
    .await;
    assert!(status.is_redirection(), "auto-provision should redirect, got {status}");

    // The newly auto-provisioned SSO user must carry a DEDUPED username, never
    // the already-taken "takenname".
    let row = sqlx::query!(
        r#"SELECT u.username
           FROM users u JOIN user_auth_links l ON l.user_id = u.id
           WHERE l.provider_id = $1 AND l.external_id = $2"#,
        provider_id,
        "dedup-external-sub",
    )
    .fetch_one(&pool)
    .await
    .expect("auto-provisioned user must exist");
    assert_ne!(row.username, "takenname", "collision must be deduped, not reused");
    assert_eq!(
        row.username, "takenname2",
        "ensure_unique_username must append the first free suffix; got {}",
        row.username
    );
}

// audit id all-6bcac91964d0 — provider-error-on-callback. When the IdP denies
// consent it redirects back WITHOUT a `code` (e.g. ?error=access_denied). The
// callback requires `code`, so such a request is rejected (4xx) rather than
// authenticating. (Token-exchange failure + expired/invalid state are covered
// by test_oauth_token_exchange_failure_is_handled +
// test_oauth_callback_invalid_or_expired_state_returns_400.)
#[tokio::test]
async fn test_oauth_callback_without_code_is_rejected() {
    let test_server = crate::common::TestServer::start().await;
    let oauth_server = OAuthMockServer::start().await.expect("mock");
    let pool = sqlx::PgPool::connect(&test_server.database_url).await.unwrap();
    seed_oidc_provider(&pool, "test-oauth", &oauth_server, json!({})).await;

    // Provider error redirect: error + state, NO code.
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let resp = client
        .get(format!(
            "{}/api/auth/oauth/test-oauth/callback?error=access_denied&state=whatever",
            test_server.base_url
        ))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_client_error(),
        "a provider-error callback (no code) must be rejected, got {}",
        resp.status()
    );
}
