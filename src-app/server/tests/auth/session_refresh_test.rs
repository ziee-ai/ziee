//! Tier 2 — the silent-refresh surface: httpOnly refresh-token cookies
//! (opt-in via `X-Refresh-Cookie: 1`), the body-in→body-out compat rule,
//! rotation + the 30s racing-client grace, logout cookie clearing +
//! revocation, real access-token expiry recovery (debug seconds seam),
//! jti whitelisting on every mint path, and refresh-token pruning.

use serde_json::{Value, json};

use crate::common::{TestServer, TestServerOptions};

const COOKIE_NAME: &str = "ziee_refresh";

/// Register a user; returns the parsed response body.
async fn register(
    client: &reqwest::Client,
    server: &TestServer,
    name: &str,
    cookie_mode: bool,
) -> (reqwest::StatusCode, Vec<String>, Value) {
    let mut req = client.post(server.api_url("/auth/register")).json(&json!({
        "username": name,
        "email": format!("{name}@example.com"),
        "password": "testpass123"
    }));
    if cookie_mode {
        req = req.header("X-Refresh-Cookie", "1");
    }
    let res = req.send().await.expect("register");
    let status = res.status();
    let set_cookies: Vec<String> = res
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_string))
        .collect();
    let body: Value = res.json().await.expect("register body");
    (status, set_cookies, body)
}

fn refresh_cookie<'a>(set_cookies: &'a [String]) -> Option<&'a String> {
    set_cookies
        .iter()
        .find(|c| c.starts_with(&format!("{COOKIE_NAME}=")))
}

#[tokio::test]
async fn test_login_with_cookie_header_sets_cookie_and_blanks_body() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    let (status, set_cookies, body) =
        register(&client, &server, "cookie_reg", true).await;
    assert_eq!(status, 201);

    // The refresh token moved into an httpOnly cookie…
    let cookie = refresh_cookie(&set_cookies).expect("Set-Cookie with ziee_refresh");
    assert!(cookie.contains("HttpOnly"), "cookie must be httpOnly: {cookie}");
    assert!(cookie.contains("SameSite=Strict"), "SameSite=Strict: {cookie}");
    assert!(cookie.contains("Path=/api/auth"), "Path scoping: {cookie}");
    assert!(cookie.contains("Max-Age="), "Max-Age present: {cookie}");
    // Plain-http test server, no trusted proxy → no Secure attribute.
    assert!(!cookie.contains("Secure"), "no Secure on plain http: {cookie}");

    // …and the JSON body's copy is blanked (page JS never sees it).
    assert_eq!(body["refresh_token"], "");
    assert!(body["access_token"].as_str().unwrap().len() > 20);

    // Login (same header) behaves identically.
    let res = client
        .post(server.api_url("/auth/login"))
        .header("X-Refresh-Cookie", "1")
        .json(&json!({ "username": "cookie_reg", "password": "testpass123" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let cookies: Vec<String> = res
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_string))
        .collect();
    assert!(refresh_cookie(&cookies).is_some(), "login sets the cookie too");
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["refresh_token"], "");
}

/// Without the opt-in header, behavior is EXACTLY as before: body token,
/// no cookie. This is the desktop-Tauri / tunnel-client regression guard.
#[tokio::test]
async fn test_login_without_header_unchanged() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    let (status, set_cookies, body) =
        register(&client, &server, "body_reg", false).await;
    assert_eq!(status, 201);
    assert!(
        refresh_cookie(&set_cookies).is_none(),
        "no opt-in header → no Set-Cookie"
    );
    assert!(
        body["refresh_token"].as_str().unwrap().len() > 20,
        "body keeps the refresh token"
    );
}

/// Cookie-mode refresh: `{}` body + the cookie → 200 with a ROTATED
/// cookie, a blank body token, a working new access token, and the old
/// jti revoked-by-rotation in the DB.
#[tokio::test]
async fn test_refresh_via_cookie_rotates_cookie() {
    let server = TestServer::start().await;
    // Browser-like client: the jar stores the register Set-Cookie and
    // attaches it to /api/auth/* requests automatically.
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();

    let (status, set_cookies, body) =
        register(&client, &server, "cookie_rot", true).await;
    assert_eq!(status, 201);
    let first_cookie = refresh_cookie(&set_cookies).unwrap().clone();
    let user_id =
        uuid::Uuid::parse_str(body["user"]["id"].as_str().unwrap()).unwrap();

    let res = client
        .post(server.api_url("/auth/refresh"))
        .header("X-Refresh-Cookie", "1")
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "cookie-sourced refresh must succeed");
    let rotated_cookies: Vec<String> = res
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_string))
        .collect();
    let rotated_cookie = refresh_cookie(&rotated_cookies)
        .expect("refresh must set the rotated cookie");
    assert_ne!(
        rotated_cookie, &first_cookie,
        "rotation must mint a DIFFERENT refresh token"
    );
    let pair: Value = res.json().await.unwrap();
    assert_eq!(pair["refresh_token"], "", "cookie-in → blank body token");

    // The new access token works.
    let me = client
        .get(server.api_url("/auth/me"))
        .header(
            "Authorization",
            format!("Bearer {}", pair["access_token"].as_str().unwrap()),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(me.status(), 200);

    // DB: exactly one revoked-by-rotation row (rotated_to set) + one active.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let (rotated, active): (i64, i64) = (
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM refresh_tokens WHERE user_id = $1 AND revoked_at IS NOT NULL AND rotated_to IS NOT NULL",
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM refresh_tokens WHERE user_id = $1 AND revoked_at IS NULL",
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
    );
    assert_eq!(rotated, 1, "old jti revoked with rotated_to recorded");
    assert_eq!(active, 1, "successor jti active");
    pool.close().await;
}

/// body-in→body-out: when both a body token AND a cookie are presented,
/// the BODY token is the one consumed/rotated and the response keeps the
/// token in the body with NO Set-Cookie — even with `X-Refresh-Cookie: 1`
/// (the phone/tunnel guard: a browser without __TAURI__ sends the header
/// but drives the body path).
#[tokio::test]
async fn test_refresh_body_precedence_over_cookie() {
    let server = TestServer::start().await;
    let cookie_client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();
    let plain_client = reqwest::Client::new();

    // User A: cookie session (jar holds A's refresh cookie).
    let (_, _, body_a) = register(&cookie_client, &server, "prec_a", true).await;
    let user_a = uuid::Uuid::parse_str(body_a["user"]["id"].as_str().unwrap()).unwrap();
    // User B: body session.
    let (_, _, body_b) = register(&plain_client, &server, "prec_b", false).await;
    let token_b = body_b["refresh_token"].as_str().unwrap().to_string();
    let user_b = uuid::Uuid::parse_str(body_b["user"]["id"].as_str().unwrap()).unwrap();

    // Send B's token in the body THROUGH the cookie client (jar attaches
    // A's cookie) + the opt-in header.
    let res = cookie_client
        .post(server.api_url("/auth/refresh"))
        .header("X-Refresh-Cookie", "1")
        .json(&json!({ "refresh_token": token_b }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let set_cookies: Vec<String> = res
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_string))
        .collect();
    assert!(
        refresh_cookie(&set_cookies).is_none(),
        "body-in → NO Set-Cookie even with the opt-in header"
    );
    let pair: Value = res.json().await.unwrap();
    assert!(
        pair["refresh_token"].as_str().unwrap().len() > 20,
        "body-in → body-out (non-blank token)"
    );

    // DB: B's jti was rotated; A's cookie session is untouched.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let b_rotated: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM refresh_tokens WHERE user_id = $1 AND rotated_to IS NOT NULL",
    )
    .bind(user_b)
    .fetch_one(&pool)
    .await
    .unwrap();
    let a_active: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM refresh_tokens WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(user_a)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(b_rotated, 1, "the BODY token (B) was consumed");
    assert_eq!(a_active, 1, "the cookie token (A) was NOT consumed");
    pool.close().await;
}

#[tokio::test]
async fn test_refresh_missing_both_401() {
    let server = TestServer::start().await;
    let res = reqwest::Client::new()
        .post(server.api_url("/auth/refresh"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
    let err: Value = res.json().await.unwrap();
    assert_eq!(err["error_code"], "MISSING_REFRESH_TOKEN");
}

/// A token revoked FOR REAL (logout — `rotated_to` stays NULL) fails with
/// an explicit 401 + REFRESH_TOKEN_REVOKED, even seconds after revocation
/// (the rotation grace never applies to logout).
#[tokio::test]
async fn test_revoked_refresh_is_401_with_code() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    let (_, _, body) = register(&client, &server, "revoked_401", false).await;
    let access = body["access_token"].as_str().unwrap();
    let refresh = body["refresh_token"].as_str().unwrap();

    let res = client
        .post(server.api_url("/auth/logout"))
        .header("Authorization", format!("Bearer {access}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);

    // Immediately after logout — well inside the 30s window — the token
    // must hard-fail (logout sets no rotated_to → no grace).
    let res = client
        .post(server.api_url("/auth/refresh"))
        .json(&json!({ "refresh_token": refresh }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
    let err: Value = res.json().await.unwrap();
    assert_eq!(err["error_code"], "REFRESH_TOKEN_REVOKED");
}

/// Rotation grace: the SAME refresh token presented twice back-to-back
/// (a second tab / SSE reconnect racing the rotation) succeeds on both —
/// the second presentation lands inside the 30s grace window and mints a
/// working pair instead of logging the racing client out.
#[tokio::test]
async fn test_rotation_grace_window_allows_racing_refresh() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    let (_, _, body) = register(&client, &server, "grace_race", false).await;
    let original = body["refresh_token"].as_str().unwrap().to_string();

    // First refresh rotates the token.
    let first = client
        .post(server.api_url("/auth/refresh"))
        .json(&json!({ "refresh_token": &original }))
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), 200);

    let user_id =
        uuid::Uuid::parse_str(body["user"]["id"].as_str().unwrap()).unwrap();

    // Second presentation of the ALREADY-ROTATED token → grace → 200
    // with a working pair.
    let second = client
        .post(server.api_url("/auth/refresh"))
        .json(&json!({ "refresh_token": &original }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        second.status(),
        200,
        "a racing legitimate client inside the grace window must not be logged out"
    );
    let pair: Value = second.json().await.unwrap();
    let me = client
        .get(server.api_url("/auth/me"))
        .header(
            "Authorization",
            format!("Bearer {}", pair["access_token"].as_str().unwrap()),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(me.status(), 200, "grace-minted access token works");

    // SECURITY: the grace path must NOT fork an independent chain — it
    // re-issues the EXISTING successor family, so there is still exactly
    // ONE active refresh row for the user (the successor of the first
    // rotation), not two.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let active: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM refresh_tokens WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        active, 1,
        "grace must converge on the successor family, not fork an independent chain"
    );
    pool.close().await;
}

/// SECURITY (regression for the rotation-grace MEDIUM finding): an
/// explicit logout hard-fails even a token that was rotated < 30s
/// earlier. Without the successor-active clause in the grace lookup, a
/// token rotated just before logout could be replayed within the window
/// to revive the session AFTER the user signed out.
#[tokio::test]
async fn test_logout_after_rotation_kills_grace() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    let (_, _, body) = register(&client, &server, "grace_logout", false).await;
    let original = body["refresh_token"].as_str().unwrap().to_string();

    // Rotate original → successor (both within the grace window).
    let rotated: Value = client
        .post(server.api_url("/auth/refresh"))
        .json(&json!({ "refresh_token": &original }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let successor_access = rotated["access_token"].as_str().unwrap();

    // Log out (revokes the whole family, including the active successor).
    let res = client
        .post(server.api_url("/auth/logout"))
        .header("Authorization", format!("Bearer {successor_access}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);

    // Replaying the just-rotated `original` within 30s of its rotation
    // must STILL 401 — logout revoked the successor, so grace no longer
    // applies (the session cannot be revived after sign-out).
    let replay = client
        .post(server.api_url("/auth/refresh"))
        .json(&json!({ "refresh_token": &original }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        replay.status(),
        401,
        "a rotated token replayed within grace must NOT revive a logged-out session"
    );
    let err: Value = replay.json().await.unwrap();
    assert_eq!(err["error_code"], "REFRESH_TOKEN_REVOKED");
}

/// Truly-concurrent refreshes of the SAME token must all succeed (the
/// atomic claim picks one winner; the losers block on the presented
/// row's lock, then converge on the winner's successor via grace — never
/// a 401, never a forked chain). Proves the single-transaction
/// claim+register closes the commit-gap race.
#[tokio::test]
async fn test_concurrent_refresh_same_token_all_succeed_single_chain() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    let (_, _, body) = register(&client, &server, "concurrent_race", false).await;
    let token = body["refresh_token"].as_str().unwrap().to_string();
    let user_id =
        uuid::Uuid::parse_str(body["user"]["id"].as_str().unwrap()).unwrap();

    // Fire N refreshes of the SAME token simultaneously.
    let base = server.api_url("/auth/refresh");
    let mut handles = Vec::new();
    for _ in 0..6 {
        let c = client.clone();
        let url = base.clone();
        let tok = token.clone();
        handles.push(tokio::spawn(async move {
            c.post(url)
                .json(&json!({ "refresh_token": tok }))
                .send()
                .await
                .unwrap()
                .status()
                .as_u16()
        }));
    }
    let mut statuses = Vec::new();
    for h in handles {
        statuses.push(h.await.unwrap());
    }
    assert!(
        statuses.iter().all(|&s| s == 200),
        "every concurrent refresh of the same token must succeed (winner rotates, \
         losers grace onto the successor); got {statuses:?}"
    );

    // Exactly ONE active refresh row survives — no forked chains.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let active: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM refresh_tokens WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(active, 1, "concurrent race must leave exactly one active chain");
    pool.close().await;
}

/// The grace window is BOUNDED: a token rotated more than
/// `ROTATION_GRACE_SECONDS` (30s) ago hard-fails. Proven without sleeping
/// 30s by back-dating the rotation's `revoked_at` in the DB.
#[tokio::test]
async fn test_rotation_grace_expires_after_window() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    let (_, _, body) = register(&client, &server, "grace_expiry", false).await;
    let original = body["refresh_token"].as_str().unwrap().to_string();
    let user_id =
        uuid::Uuid::parse_str(body["user"]["id"].as_str().unwrap()).unwrap();

    // Rotate original → successor.
    let rotated = client
        .post(server.api_url("/auth/refresh"))
        .json(&json!({ "refresh_token": &original }))
        .send()
        .await
        .unwrap();
    assert_eq!(rotated.status(), 200);

    // Back-date the rotation past the grace window (simulates >30s elapsed).
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let affected = sqlx::query(
        "UPDATE refresh_tokens SET revoked_at = NOW() - INTERVAL '31 seconds' \
         WHERE user_id = $1 AND rotated_to IS NOT NULL",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap()
    .rows_affected();
    assert!(affected >= 1);
    pool.close().await;

    // Now the rotated token is outside the grace window → hard-fail.
    let replay = client
        .post(server.api_url("/auth/refresh"))
        .json(&json!({ "refresh_token": &original }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        replay.status(),
        401,
        "a token rotated >30s ago must be outside the grace window"
    );
    let err: Value = replay.json().await.unwrap();
    assert_eq!(err["error_code"], "REFRESH_TOKEN_REVOKED");
}

/// Logout clears the httpOnly cookie (Max-Age=0) and revokes every
/// refresh token, so neither the cookie nor a body token works afterward.
#[tokio::test]
async fn test_logout_clears_cookie_and_revokes_all() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();

    let (_, _, body) = register(&client, &server, "logout_cookie", true).await;
    let access = body["access_token"].as_str().unwrap();

    let res = client
        .post(server.api_url("/auth/logout"))
        .header("Authorization", format!("Bearer {access}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);
    let cookies: Vec<String> = res
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_string))
        .collect();
    let clearing = refresh_cookie(&cookies).expect("logout sets a clearing cookie");
    assert!(
        clearing.contains("Max-Age=0"),
        "clearing cookie must have Max-Age=0: {clearing}"
    );

    // The jar honored Max-Age=0 → subsequent cookie refresh has no token.
    let res = client
        .post(server.api_url("/auth/refresh"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
    let err: Value = res.json().await.unwrap();
    assert_eq!(err["error_code"], "MISSING_REFRESH_TOKEN");
}

/// The whole point of the feature, server-side: an EXPIRED access token
/// 401s, the refresh endpoint recovers the session, and the new access
/// token works — exercised with a real 2-second TTL via the debug-only
/// `access_token_expiry_seconds` seam.
#[tokio::test]
async fn test_expired_access_token_401_but_refresh_recovers() {
    let server = TestServer::start_with_options(TestServerOptions {
        access_token_expiry_seconds: Some(2),
        ..Default::default()
    })
    .await;
    let client = reqwest::Client::new();

    let (status, _, body) = register(&client, &server, "expiry_rec", false).await;
    assert_eq!(status, 201);
    assert_eq!(body["expires_in"], 2, "expires_in reflects the seam");
    let access = body["access_token"].as_str().unwrap();
    let refresh = body["refresh_token"].as_str().unwrap();

    // Fresh token works…
    let me = client
        .get(server.api_url("/auth/me"))
        .header("Authorization", format!("Bearer {access}"))
        .send()
        .await
        .unwrap();
    assert_eq!(me.status(), 200);

    // …expires for real. The token TTL is 2s but JWT validation carries a
    // small leeway (JWT_LEEWAY_SECONDS=5, absorbing sub-second skew), so
    // wait past 2s+5s before asserting rejection.
    tokio::time::sleep(std::time::Duration::from_secs(8)).await;
    let me = client
        .get(server.api_url("/auth/me"))
        .header("Authorization", format!("Bearer {access}"))
        .send()
        .await
        .unwrap();
    assert_eq!(me.status(), 401, "expired access token must 401");

    // …and the refresh token recovers the session.
    let res = client
        .post(server.api_url("/auth/refresh"))
        .json(&json!({ "refresh_token": refresh }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "refresh must survive access-token expiry");
    let pair: Value = res.json().await.unwrap();
    let me = client
        .get(server.api_url("/auth/me"))
        .header(
            "Authorization",
            format!("Bearer {}", pair["access_token"].as_str().unwrap()),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(me.status(), 200, "refreshed session works");
}

/// Every mint path issues jti-carrying, whitelisted refresh tokens
/// (register + login here; OAuth is covered in oauth_test.rs, setup_admin
/// below, desktop auto_login in the desktop crate's auth_tests).
#[tokio::test]
async fn test_all_mints_carry_jti() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    let (_, _, reg_body) = register(&client, &server, "jti_all", false).await;
    let login_body: Value = client
        .post(server.api_url("/auth/login"))
        .json(&json!({ "username": "jti_all", "password": "testpass123" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let user_id =
        uuid::Uuid::parse_str(reg_body["user"]["id"].as_str().unwrap()).unwrap();

    let svc = ziee::JwtService::try_new(ziee::JwtConfig {
        secret: "test-secret-key-for-jwt-tokens-min-32-chars-long".to_string(),
        issuer: "ziee".to_string(),
        audience: "ziee-api".to_string(),
        access_token_expiry_hours: 24,
        refresh_token_expiry_days: 30,
        access_token_expiry_seconds: None,
    })
    .unwrap();

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();

    for (path, body) in [("register", &reg_body), ("login", &login_body)] {
        let claims = svc
            .validate_refresh_token(body["refresh_token"].as_str().unwrap())
            .unwrap();
        let jti = uuid::Uuid::parse_str(
            claims.jti.as_deref().unwrap_or_else(|| panic!("{path} refresh token must carry a jti")),
        )
        .unwrap();
        let whitelisted: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM refresh_tokens WHERE jti = $1 AND user_id = $2 AND revoked_at IS NULL",
        )
        .bind(jti)
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(whitelisted, 1, "{path} jti must be whitelisted");
    }
    pool.close().await;
}

/// First-run `setup_admin` (a browser flow) honors cookie mode and mints
/// a whitelisted refresh token usable via the cookie immediately.
#[tokio::test]
async fn test_setup_admin_cookie_mode() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();

    let res = client
        .post(server.api_url("/app/setup/admin"))
        .header("X-Refresh-Cookie", "1")
        .json(&json!({
            "username": "setupadmin",
            "email": "setupadmin@example.com",
            "password": "setuppass123!"
        }))
        .send()
        .await
        .unwrap();

    // Each test gets a FRESH per-test DB (migrations only, no seeded admin),
    // so first-run setup always succeeds — no soft-skip.
    assert_eq!(res.status(), 201);
    let cookies: Vec<String> = res
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_string))
        .collect();
    assert!(refresh_cookie(&cookies).is_some(), "setup sets the cookie");
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["refresh_token"], "", "cookie mode blanks the body token");

    // The cookie refresh works immediately after setup.
    let res = client
        .post(server.api_url("/auth/refresh"))
        .header("X-Refresh-Cookie", "1")
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
}

/// The auth cleanup tick deletes refresh-token rows 7 days past
/// expiry/revocation and keeps active ones.
#[tokio::test]
async fn test_prune_deletes_stale_rows() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    let (_, _, body) = register(&client, &server, "prune_user", false).await;
    let user_id =
        uuid::Uuid::parse_str(body["user"]["id"].as_str().unwrap()).unwrap();

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();

    // Stale rows: one long-expired, one revoked long ago.
    let stale_expired = uuid::Uuid::new_v4();
    let stale_revoked = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO refresh_tokens (jti, user_id, expires_at) VALUES ($1, $2, NOW() - INTERVAL '10 days')",
    )
    .bind(stale_expired)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO refresh_tokens (jti, user_id, expires_at, revoked_at) VALUES ($1, $2, NOW() + INTERVAL '20 days', NOW() - INTERVAL '8 days')",
    )
    .bind(stale_revoked)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    let repo = ziee::AuthRepository::new(pool.clone());
    let (_, _, pruned) = repo.cleanup_expired_auth_rows().await.unwrap();
    assert!(pruned >= 2, "both stale rows pruned (got {pruned})");

    let remaining: Vec<uuid::Uuid> =
        sqlx::query_scalar("SELECT jti FROM refresh_tokens WHERE user_id = $1")
            .bind(user_id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(!remaining.contains(&stale_expired), "expired-stale gone");
    assert!(!remaining.contains(&stale_revoked), "revoked-stale gone");
    assert_eq!(remaining.len(), 1, "the live registration token survives");
    pool.close().await;
}

// ─────────────────────────────────────────────────────────────────────────────
// Access-token revocation on logout (`users.token_version` + the `ver` claim).
//
// Before this, logout revoked only the REFRESH token: the access token stayed a
// fully valid credential for its whole TTL (24h by default), so a held/leaked
// token — or simply another tab — kept full API access after "logging out".
// ─────────────────────────────────────────────────────────────────────────────

/// Log in and return the access token.
async fn login_access(client: &reqwest::Client, server: &TestServer, name: &str) -> String {
    let res = client
        .post(server.api_url("/auth/login"))
        .json(&json!({ "username": name, "password": "testpass123" }))
        .send()
        .await
        .expect("login");
    assert_eq!(res.status(), 200, "login should succeed for {name}");
    let body: Value = res.json().await.unwrap();
    body["access_token"].as_str().unwrap().to_string()
}

async fn get_status(client: &reqwest::Client, server: &TestServer, path: &str, access: &str) -> reqwest::StatusCode {
    client
        .get(server.api_url(path))
        .header("Authorization", format!("Bearer {access}"))
        .send()
        .await
        .expect("request")
        .status()
}

async fn logout(client: &reqwest::Client, server: &TestServer, access: &str) -> reqwest::StatusCode {
    client
        .post(server.api_url("/auth/logout"))
        .header("Authorization", format!("Bearer {access}"))
        .send()
        .await
        .expect("logout")
        .status()
}

/// TEST-4 — THE CORE GAP. The reported bug in one assertion: after logout the
/// SAME, still-unexpired access token must stop working.
#[tokio::test]
async fn test_logout_revokes_the_access_token() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();
    let (_, _, body) = register(&client, &server, "logout_revokes_access", false).await;
    let access = body["access_token"].as_str().unwrap().to_string();

    assert_eq!(
        get_status(&client, &server, "/auth/me", &access).await,
        200,
        "precondition: the token works before logout"
    );

    assert_eq!(logout(&client, &server, &access).await, 204);

    let res = client
        .get(server.api_url("/auth/me"))
        .header("Authorization", format!("Bearer {access}"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        401,
        "the access token must be dead immediately after logout, not at exp"
    );
    let err: Value = res.json().await.unwrap();
    assert_eq!(err["error_code"], "SESSION_REVOKED");
}

/// TEST-6 — the folded read path (`extract_authenticated_user` →
/// `get_by_id_with_token_version`), exercised via a `RequirePermissions` route.
/// `/conversations` is the LITERAL reported leak ("the new user can still see
/// the admin's conversations").
#[tokio::test]
async fn test_logout_revokes_access_on_permission_gated_routes() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();
    let (_, _, body) = register(&client, &server, "logout_perm_routes", false).await;
    let access = body["access_token"].as_str().unwrap().to_string();

    assert_eq!(
        get_status(&client, &server, "/conversations", &access).await,
        200,
        "precondition: conversations readable before logout"
    );
    assert_eq!(logout(&client, &server, &access).await, 204);
    assert_eq!(
        get_status(&client, &server, "/conversations", &access).await,
        401,
        "a RequirePermissions route must reject the logged-out token"
    );
}

/// TEST-5 — the bare-`JwtAuth` routes. `/auth/me` happens to re-check the user,
/// and every `RequirePermissions` route goes through the other read path — these
/// two go through NEITHER, so they are the ones that prove the extractor-level
/// coverage claim rather than a per-handler patch.
#[tokio::test]
async fn test_logout_revokes_access_on_bare_jwtauth_routes() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();
    let (_, _, body) = register(&client, &server, "logout_bare_jwt", false).await;
    let access = body["access_token"].as_str().unwrap().to_string();

    assert_eq!(
        get_status(&client, &server, "/onboarding/progress", &access).await,
        200,
        "precondition: onboarding progress readable before logout"
    );
    assert_eq!(logout(&client, &server, &access).await, 204);

    assert_eq!(
        get_status(&client, &server, "/onboarding/progress", &access).await,
        401,
        "bare JwtAuth route must reject the logged-out token"
    );
    assert_eq!(
        get_status(&client, &server, "/hub/installed", &access).await,
        401,
        "bare JwtAuth route must reject the logged-out token"
    );
}

/// TEST-7 — the executable proof that the counter design is right and the
/// rejected `sessions_revoked_at`-vs-`iat` design is not.
///
/// `iat` is whole seconds; NOW() has microseconds. A timestamp rule would make
/// the pre-logout and post-relogin tokens INDISTINGUISHABLE inside one second:
/// it would either kill both (an infinite login loop for the rest of that
/// second) or spare both (a 1s hole). No sleep here — the point is that logout
/// and re-login land in the same second.
#[tokio::test]
async fn test_logout_then_immediate_relogin_yields_a_working_token() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();
    let (_, _, body) = register(&client, &server, "relogin_same_second", false).await;
    let old_access = body["access_token"].as_str().unwrap().to_string();

    assert_eq!(logout(&client, &server, &old_access).await, 204);
    let new_access = login_access(&client, &server, "relogin_same_second").await;

    assert_eq!(
        get_status(&client, &server, "/auth/me", &new_access).await,
        200,
        "a token minted AFTER the logout must work, even in the same wall-clock second"
    );
    assert_eq!(
        get_status(&client, &server, "/auth/me", &old_access).await,
        401,
        "...while the pre-logout token from that same second stays dead"
    );
}

/// TEST-10 — the epoch is per-user: A's logout must not touch B's session.
#[tokio::test]
async fn test_logout_leaves_other_users_sessions_alone() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();
    let (_, _, a) = register(&client, &server, "epoch_user_a", false).await;
    let (_, _, b) = register(&client, &server, "epoch_user_b", false).await;
    let a_access = a["access_token"].as_str().unwrap().to_string();
    let b_access = b["access_token"].as_str().unwrap().to_string();

    assert_eq!(logout(&client, &server, &a_access).await, 204);

    assert_eq!(
        get_status(&client, &server, "/auth/me", &a_access).await,
        401
    );
    assert_eq!(
        get_status(&client, &server, "/auth/me", &b_access).await,
        200,
        "user B's session must survive user A's logout"
    );
}

/// TEST-11 — the zero-forced-logout deploy contract. A token minted before this
/// shipped carries no `ver`; it must keep working against a user still at the
/// column's DEFAULT 0. Hand-minted with the server's secret because the minter
/// can no longer produce a `ver`-less token.
#[tokio::test]
async fn test_pre_migration_ver_less_token_still_authenticates() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();
    let (_, _, body) = register(&client, &server, "ver_less_token", false).await;
    let user_id = body["user"]["id"].as_str().unwrap().to_string();

    // Same claims a pre-upgrade access token carried — no `ver`.
    let now = chrono::Utc::now().timestamp();
    let claims = json!({
        "sub": user_id,
        "exp": now + 3600,
        "iat": now,
        "iss": "ziee",
        "aud": "ziee-api",
        "username": "ver_less_token",
        "email": "ver_less_token@example.com",
        "is_admin": false,
    });
    // The harness's fixed test secret (tests/common/harness_inner.rs:543); the
    // same literal the sync suite hand-mints with (tests/sync/subscribe_test.rs:228).
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(b"test-secret-key-for-jwt-tokens-min-32-chars-long"),
    )
    .expect("hand-mint a ver-less token");

    assert_eq!(
        get_status(&client, &server, "/auth/me", &token).await,
        200,
        "a pre-migration token must keep working until it expires — deploying forces zero logouts"
    );
}

/// TEST-8 — LOGOUT ATOMICITY (bump + revoke are one transaction).
///
/// If the bump could commit while the revoke failed, the surviving refresh
/// token would re-mint through `mint_session_tokens` — which reads the NEW
/// epoch — handing back a fully valid access token and defeating the logout.
/// Forces the revoke to fail with a trigger and asserts NOTHING committed.
///
/// Safe to mutate the schema: every test owns a unique per-test database.
#[tokio::test]
async fn test_logout_is_atomic_bump_rolls_back_if_revoke_fails() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();
    let (_, _, body) = register(&client, &server, "logout_atomic", false).await;
    let access = body["access_token"].as_str().unwrap().to_string();
    let refresh = body["refresh_token"].as_str().unwrap().to_string();

    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    sqlx::query(
        r#"CREATE OR REPLACE FUNCTION ziee_test_fail_revoke() RETURNS trigger AS $$
           BEGIN RAISE EXCEPTION 'forced revoke failure'; END; $$ LANGUAGE plpgsql;"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"CREATE TRIGGER ziee_test_fail_revoke_trg BEFORE UPDATE ON refresh_tokens
           FOR EACH ROW EXECUTE FUNCTION ziee_test_fail_revoke();"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // Logout must fail rather than half-apply.
    let status = logout(&client, &server, &access).await;
    assert!(
        status.is_server_error(),
        "logout should surface the DB failure, got {status}"
    );

    // NEITHER write may have landed. If the bump had committed alone, the
    // surviving refresh token below would mint a token carrying the new epoch.
    let version: i32 = sqlx::query_scalar("SELECT token_version FROM users WHERE username = $1")
        .bind("logout_atomic")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        version, 0,
        "token_version must roll back when the refresh-token revoke fails"
    );
    assert_eq!(
        get_status(&client, &server, "/auth/me", &access).await,
        200,
        "the access token must still work — the logout did not happen"
    );

    // Drop the trigger; the refresh token must still be live (not half-revoked).
    sqlx::query("DROP TRIGGER ziee_test_fail_revoke_trg ON refresh_tokens;")
        .execute(&pool)
        .await
        .unwrap();
    let res = client
        .post(server.api_url("/auth/refresh"))
        .json(&json!({ "refresh_token": refresh }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        200,
        "the refresh token must be untouched by the failed logout"
    );

    // And a real logout still works afterwards.
    let access2 = login_access(&client, &server, "logout_atomic").await;
    assert_eq!(logout(&client, &server, &access2).await, 204);
    assert_eq!(
        get_status(&client, &server, "/auth/me", &access2).await,
        401
    );
}

/// TEST-9 — a session obtained via a refresh does not outlive a LATER logout.
///
/// HONEST SCOPE: this is sequential (the refresh completes, then logout runs),
/// so it does NOT exercise the read-before-claim ordering — with both operations
/// serialized, the epoch read returns the same value before or after the claim.
/// It pins the user-visible property (rotating your token doesn't buy you a
/// session that survives logout). The actual interleaving is covered by
/// TEST-23 below.
#[tokio::test]
async fn test_refresh_then_logout_kills_the_refreshed_session() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();
    let (_, _, body) = register(&client, &server, "refresh_race_logout", false).await;
    let access = body["access_token"].as_str().unwrap().to_string();
    let refresh = body["refresh_token"].as_str().unwrap().to_string();

    // A refresh completes (rotation claimed + committed).
    let res = client
        .post(server.api_url("/auth/refresh"))
        .json(&json!({ "refresh_token": refresh }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let refreshed: Value = res.json().await.unwrap();
    let refreshed_access = refreshed["access_token"].as_str().unwrap().to_string();
    assert_eq!(
        get_status(&client, &server, "/auth/me", &refreshed_access).await,
        200,
        "precondition: the refreshed token works"
    );

    // Now logout. Every token the user holds — including the one just minted by
    // the refresh — must die.
    assert_eq!(logout(&client, &server, &access).await, 204);
    assert_eq!(
        get_status(&client, &server, "/auth/me", &refreshed_access).await,
        401,
        "a token minted by a refresh must not survive a subsequent logout"
    );
}

/// TEST-23 — SECURITY: no refresh token may survive a logout, even when a
/// rotation is in flight at the moment the logout lands.
///
/// The bug this pins (found in the blind audit): logout revokes with
/// `UPDATE refresh_tokens ... WHERE user_id = $1 AND revoked_at IS NULL`. Under
/// READ COMMITTED that UPDATE only scans rows committed as of the command's
/// start, so a SUCCESSOR token that a concurrent `/auth/refresh` has INSERTed
/// but not yet COMMITTed is invisible to it — never scanned, never revoked —
/// while the epoch still moves to N+1. Replaying that successor then mints an
/// access token stamped with the NEW epoch: a fully valid session, i.e. logout
/// silently defeated. `claim_rotation_and_register` takes the `users` row lock
/// FOR SHARE first to force a serial order with logout.
///
/// Drives real concurrent HTTP (refresh chain vs logout) rather than a
/// contrived interleaving, then asserts the INVARIANT that must hold no matter
/// who won: after logout returns 204, the user has ZERO usable refresh tokens
/// and no working access token. Both outcomes are legal (the refresh may win
/// and rotate, or lose and 401) — what is illegal is a live token afterwards.
#[tokio::test]
async fn test_no_refresh_token_survives_a_concurrent_logout() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();

    // Repeat: the window is sub-millisecond, so one attempt could miss it.
    for round in 0..12 {
        let name = format!("race_logout_{round}");
        let (_, _, body) = register(&client, &server, &name, false).await;
        let access = body["access_token"].as_str().unwrap().to_string();
        let refresh = body["refresh_token"].as_str().unwrap().to_string();
        let user_id: uuid::Uuid = body["user"]["id"].as_str().unwrap().parse().unwrap();

        // Fire the rotation and the logout concurrently.
        let (c1, s1) = (client.clone(), server.api_url("/auth/refresh"));
        let refresher = tokio::spawn(async move {
            c1.post(s1)
                .json(&json!({ "refresh_token": refresh }))
                .send()
                .await
                .ok()
        });
        let (c2, s2) = (client.clone(), server.api_url("/auth/logout"));
        let acc = access.clone();
        let logouter = tokio::spawn(async move {
            c2.post(s2)
                .header("Authorization", format!("Bearer {acc}"))
                .send()
                .await
                .ok()
        });

        let refresh_res = refresher.await.unwrap();
        let logout_res = logouter.await.unwrap();
        assert_eq!(
            logout_res.expect("logout response").status(),
            204,
            "round {round}: logout must succeed"
        );

        // INVARIANT 1: not one active refresh-token row remains for this user.
        let active: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM refresh_tokens WHERE user_id = $1 AND revoked_at IS NULL",
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            active, 0,
            "round {round}: a refresh token survived the logout — it can re-mint a \
             valid access token stamped with the NEW epoch and restore the session"
        );

        // INVARIANT 2: if the refresh won the race, the session it handed back
        // must not work either.
        if let Some(res) = refresh_res
            && res.status() == 200
        {
            let refreshed: Value = res.json().await.unwrap();
            let new_access = refreshed["access_token"].as_str().unwrap().to_string();
            assert_eq!(
                get_status(&client, &server, "/auth/me", &new_access).await,
                401,
                "round {round}: a token minted by a refresh racing the logout must not work"
            );
            // ...and its refresh token must not be replayable.
            let new_refresh = refreshed["refresh_token"].as_str().unwrap().to_string();
            let replay = client
                .post(server.api_url("/auth/refresh"))
                .json(&json!({ "refresh_token": new_refresh }))
                .send()
                .await
                .unwrap();
            assert_eq!(
                replay.status(),
                401,
                "round {round}: the successor refresh token must be dead after logout"
            );
        }

        // INVARIANT 3: the original access token is dead.
        assert_eq!(
            get_status(&client, &server, "/auth/me", &access).await,
            401,
            "round {round}: the pre-logout access token must be dead"
        );
    }
}
