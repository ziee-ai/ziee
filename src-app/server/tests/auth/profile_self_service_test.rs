//! Integration tests for the self-service profile endpoints:
//!   - POST /auth/profile   (update own username + display_name)
//!   - POST /auth/password  (change own password)
//!   - GET  /auth/me        (now carries `has_password`)
//!
//! These mirror the bearer-token pattern in `tests/user/mod.rs` and the
//! desktop `remote_access/change_password_test.rs`. Each `TestServer`
//! gets its own database, so the tests are isolated under `--test-threads=1`.

use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;

/// Register a fresh local-password user and return
/// `(access_token, user_id, username)`. Password is always
/// `password123` (>= 8 chars, satisfies the strength check).
async fn register(server: &crate::common::TestServer, base: &str) -> (String, String, String) {
    let username = format!("{}_{}", base, &uuid::Uuid::new_v4().to_string()[..8]);
    let res = reqwest::Client::new()
        .post(server.api_url("/auth/register"))
        .json(&json!({
            "username": username,
            "email": format!("{}@example.com", username),
            "password": "password123",
        }))
        .send()
        .await
        .expect("register request failed");
    assert_eq!(res.status(), 201, "registration should succeed");
    let body: Value = res.json().await.expect("parse register body");
    let token = body["access_token"].as_str().unwrap().to_string();
    let user_id = body["user"]["id"].as_str().unwrap().to_string();
    (token, user_id, username)
}

/// Register a user, then exchange the legacy (no-`jti`) registration
/// refresh token once via `/auth/refresh` so the returned refresh token
/// is WHITELISTED (carries a `jti`). Only whitelisted tokens are governed
/// by revocation — register/login mint non-`jti` tokens that bypass the
/// whitelist. Returns `(access_token, whitelisted_refresh_token)`.
async fn register_and_whitelist(
    server: &crate::common::TestServer,
    base: &str,
) -> (String, String) {
    let client = reqwest::Client::new();
    let username = format!("{}_{}", base, &uuid::Uuid::new_v4().to_string()[..8]);
    let reg: Value = client
        .post(server.api_url("/auth/register"))
        .json(&json!({
            "username": username,
            "email": format!("{}@example.com", username),
            "password": "password123",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let reg_refresh = reg["refresh_token"].as_str().unwrap().to_string();

    let pair: Value = client
        .post(server.api_url("/auth/refresh"))
        .json(&json!({ "refresh_token": reg_refresh }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    (
        pair["access_token"].as_str().unwrap().to_string(),
        pair["refresh_token"].as_str().unwrap().to_string(),
    )
}

async fn get_me(server: &crate::common::TestServer, token: &str) -> Value {
    let res = reqwest::Client::new()
        .get(server.api_url("/auth/me"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("/auth/me request failed");
    assert_eq!(res.status(), 200, "/auth/me should be 200");
    res.json().await.expect("parse /auth/me body")
}

async fn null_out_password_hash(server: &crate::common::TestServer, user_id: &str) {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect to test DB");
    sqlx::query("UPDATE users SET password_hash = NULL WHERE id = $1")
        .bind(uuid::Uuid::parse_str(user_id).unwrap())
        .execute(&pool)
        .await
        .expect("null out password_hash");
    pool.close().await;
}

// =====================================================
// POST /auth/profile
// =====================================================

#[tokio::test]
async fn update_profile_display_name_happy() {
    let server = crate::common::TestServer::start().await;
    let (token, _id, _username) = register(&server, "disp").await;

    let res = reqwest::Client::new()
        .post(server.api_url("/auth/profile"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({ "display_name": "Brand New Name" }))
        .send()
        .await
        .expect("update profile failed");
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["display_name"], "Brand New Name");

    // /auth/me agrees.
    let me = get_me(&server, &token).await;
    assert_eq!(me["user"]["display_name"], "Brand New Name");
}

#[tokio::test]
async fn update_profile_username_then_login() {
    let server = crate::common::TestServer::start().await;
    let (token, _id, _old) = register(&server, "uname").await;
    let new_username = format!("renamed_{}", &uuid::Uuid::new_v4().to_string()[..8]);

    let res = reqwest::Client::new()
        .post(server.api_url("/auth/profile"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({ "username": new_username }))
        .send()
        .await
        .expect("update username failed");
    assert_eq!(res.status(), 200);

    // Can log in with the new username + original password.
    let login = reqwest::Client::new()
        .post(server.api_url("/auth/login"))
        .json(&json!({ "username": new_username, "password": "password123" }))
        .send()
        .await
        .expect("login failed");
    assert_eq!(login.status(), 200, "login with new username should work");
}

#[tokio::test]
async fn update_profile_both_fields() {
    let server = crate::common::TestServer::start().await;
    let (token, _id, _u) = register(&server, "both").await;
    let new_username = format!("both_{}", &uuid::Uuid::new_v4().to_string()[..8]);

    let res = reqwest::Client::new()
        .post(server.api_url("/auth/profile"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({ "username": new_username, "display_name": "Both Set" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["username"], new_username);
    assert_eq!(body["display_name"], "Both Set");
}

#[tokio::test]
async fn update_profile_empty_body_is_noop() {
    let server = crate::common::TestServer::start().await;
    let (token, _id, username) = register(&server, "noop").await;
    let client = reqwest::Client::new();

    // Give the user a display_name first, so the no-op can prove BOTH
    // fields are preserved.
    client
        .post(server.api_url("/auth/profile"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({ "display_name": "Keep Me" }))
        .send()
        .await
        .unwrap();

    let res = client
        .post(server.api_url("/auth/profile"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "empty patch should be a 200 no-op");
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["username"], username);
    assert_eq!(body["display_name"], "Keep Me", "display_name preserved");
}

#[tokio::test]
async fn update_profile_empty_display_name_clears_it() {
    let server = crate::common::TestServer::start().await;
    let (token, _id, _u) = register(&server, "dnclear").await;
    let client = reqwest::Client::new();

    // Set a display name, then clear it with "".
    client
        .post(server.api_url("/auth/profile"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({ "display_name": "Has Name" }))
        .send()
        .await
        .unwrap();
    let me = get_me(&server, &token).await;
    assert_eq!(me["user"]["display_name"], "Has Name");

    let res = client
        .post(server.api_url("/auth/profile"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({ "display_name": "" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let me = get_me(&server, &token).await;
    assert!(
        me["user"]["display_name"].is_null(),
        "empty display_name must clear it back to null, got {:?}",
        me["user"]["display_name"]
    );
}

#[tokio::test]
async fn update_profile_null_display_name_is_noop() {
    let server = crate::common::TestServer::start().await;
    let (token, _id, _u) = register(&server, "dnnull").await;
    let client = reqwest::Client::new();

    client
        .post(server.api_url("/auth/profile"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({ "display_name": "Stay" }))
        .send()
        .await
        .unwrap();

    // Explicit JSON null is a no-op (keep) — only "" clears.
    let res = client
        .post(server.api_url("/auth/profile"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({ "display_name": null }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let me = get_me(&server, &token).await;
    assert_eq!(
        me["user"]["display_name"], "Stay",
        "null is a no-op, keeps value"
    );
}

#[tokio::test]
async fn update_profile_trims_username() {
    let server = crate::common::TestServer::start().await;
    let (token, _id, _u) = register(&server, "trim").await;
    let target = format!("trimmed_{}", &uuid::Uuid::new_v4().to_string()[..8]);

    let res = reqwest::Client::new()
        .post(server.api_url("/auth/profile"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({ "username": format!("  {}  ", target) }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["username"], target, "username must be stored trimmed");

    // And login works with the trimmed value.
    let login = reqwest::Client::new()
        .post(server.api_url("/auth/login"))
        .json(&json!({ "username": target, "password": "password123" }))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), 200);
}

#[tokio::test]
async fn update_profile_username_conflict_returns_409() {
    let server = crate::common::TestServer::start().await;
    let (_token_a, _id_a, username_a) = register(&server, "conflicta").await;
    let (token_b, _id_b, _username_b) = register(&server, "conflictb").await;

    // B tries to take A's username.
    let res = reqwest::Client::new()
        .post(server.api_url("/auth/profile"))
        .header("Authorization", format!("Bearer {}", token_b))
        .json(&json!({ "username": username_a }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        reqwest::StatusCode::CONFLICT,
        "taking another user's username must be 409"
    );
}

#[tokio::test]
async fn update_profile_own_username_is_idempotent() {
    let server = crate::common::TestServer::start().await;
    let (token, _id, username) = register(&server, "idem").await;

    // Re-submitting your own current username is NOT a conflict.
    let res = reqwest::Client::new()
        .post(server.api_url("/auth/profile"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({ "username": username }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        200,
        "own username should be a 200 no-op, not 409"
    );
}

#[tokio::test]
async fn update_profile_blank_username_rejected() {
    let server = crate::common::TestServer::start().await;
    let (token, _id, _u) = register(&server, "blank").await;

    let res = reqwest::Client::new()
        .post(server.api_url("/auth/profile"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({ "username": "   " }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400, "blank username should be rejected");
    // Pin the *reason* so a 400 from an unrelated cause (e.g. body parse)
    // can't satisfy this test.
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["error_code"], "INVALID_USERNAME");
}

#[tokio::test]
async fn update_profile_whitespace_display_name_clears_it() {
    let server = crate::common::TestServer::start().await;
    let (token, _id, _u) = register(&server, "dnws").await;
    let client = reqwest::Client::new();

    client
        .post(server.api_url("/auth/profile"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({ "display_name": "Has Name" }))
        .send()
        .await
        .unwrap();

    // Whitespace-only is treated the same as empty → clears to NULL.
    let res = client
        .post(server.api_url("/auth/profile"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({ "display_name": "   " }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let me = get_me(&server, &token).await;
    assert!(
        me["user"]["display_name"].is_null(),
        "whitespace-only display_name must clear it, got {:?}",
        me["user"]["display_name"]
    );
}

/// Editing the profile (display_name) must NOT revoke refresh tokens —
/// only a credential change does. Uses a WHITELISTED token (so the
/// assertion is meaningful: a non-`jti` token would refresh regardless).
#[tokio::test]
async fn update_profile_does_not_revoke_refresh_tokens() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();
    let (access, refresh) = register_and_whitelist(&server, "noprofrevoke").await;

    // Rename the user.
    let res = client
        .post(server.api_url("/auth/profile"))
        .header("Authorization", format!("Bearer {}", access))
        .json(&json!({ "display_name": "Renamed Only" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    // The whitelisted refresh token is still valid (profile edits don't
    // rotate credentials, so revocation never fires).
    let refreshed = client
        .post(server.api_url("/auth/refresh"))
        .json(&json!({ "refresh_token": refresh }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        refreshed.status(),
        200,
        "a profile edit must not revoke refresh tokens"
    );
}

#[tokio::test]
async fn update_profile_requires_auth() {
    let server = crate::common::TestServer::start().await;
    let res = reqwest::Client::new()
        .post(server.api_url("/auth/profile"))
        .json(&json!({ "display_name": "x" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401, "missing bearer must be 401");
}

/// SECURITY: the request struct carries ONLY username + display_name.
/// Privileged fields injected into the JSON body must be silently
/// ignored (serde drops unknown fields) — proving this path can't be
/// abused to self-grant admin, flip is_active, hijack email, or
/// re-point the row id.
#[tokio::test]
async fn update_profile_ignores_privileged_fields() {
    let server = crate::common::TestServer::start().await;
    let (token, _id, _u) = register(&server, "escal").await;

    let me_before = get_me(&server, &token).await;
    let email_before = me_before["user"]["email"].clone();

    let res = reqwest::Client::new()
        .post(server.api_url("/auth/profile"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "display_name": "Harmless",
            "is_admin": true,
            "is_active": false,
            "permissions": ["users::delete", "*"],
            "email": "attacker@evil.example",
            "id": "00000000-0000-0000-0000-000000000000",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    let me_after = get_me(&server, &token).await;
    assert_eq!(
        me_after["user"]["is_admin"],
        json!(false),
        "must not self-grant admin"
    );
    assert_eq!(
        me_after["user"]["is_active"],
        json!(true),
        "must not flip is_active"
    );
    assert_eq!(
        me_after["user"]["email"], email_before,
        "must not change email"
    );
    assert_eq!(
        me_after["user"]["display_name"], "Harmless",
        "the allowed field still applies"
    );
    // Permissions are not granted by the inert array.
    let perms = me_after["permissions"].as_array().unwrap();
    assert!(
        !perms.iter().any(|p| p == "users::delete" || p == "*"),
        "must not self-grant permissions"
    );
}

/// The endpoint is gated on `profile::edit`. A user stripped of the
/// default group (and thus that permission) must be forbidden.
#[tokio::test]
async fn update_profile_without_permission_returns_403() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_no_permissions(&server, "noedit").await;

    let res = reqwest::Client::new()
        .post(server.api_url("/auth/profile"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "display_name": "nope" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403, "without profile::edit must be 403");
}

#[tokio::test]
async fn update_profile_scoped_to_caller() {
    let server = crate::common::TestServer::start().await;
    let (token_a, _id_a, _u_a) = register(&server, "scopea").await;
    let (token_b, _id_b, username_b) = register(&server, "scopeb").await;

    // A renames its own display_name.
    let res = reqwest::Client::new()
        .post(server.api_url("/auth/profile"))
        .header("Authorization", format!("Bearer {}", token_a))
        .json(&json!({ "display_name": "Only A" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    // B is untouched.
    let me_b = get_me(&server, &token_b).await;
    assert_eq!(me_b["user"]["username"], username_b);
    assert_ne!(me_b["user"]["display_name"], "Only A");
}

// =====================================================
// POST /auth/password
// =====================================================

#[tokio::test]
async fn change_password_happy_then_login_with_new() {
    let server = crate::common::TestServer::start().await;
    let (token, _id, username) = register(&server, "cphappy").await;

    let res = reqwest::Client::new()
        .post(server.api_url("/auth/password"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "current_password": "password123",
            "new_password": "NewStrongPassword456!",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);

    // Old password no longer logs in.
    let old = reqwest::Client::new()
        .post(server.api_url("/auth/login"))
        .json(&json!({ "username": username, "password": "password123" }))
        .send()
        .await
        .unwrap();
    assert_ne!(old.status(), 200, "old password must stop working");

    // New password logs in.
    let new = reqwest::Client::new()
        .post(server.api_url("/auth/login"))
        .json(&json!({ "username": username, "password": "NewStrongPassword456!" }))
        .send()
        .await
        .unwrap();
    assert_eq!(new.status(), 200, "new password must work");
}

/// Changing the password revokes the user's WHITELISTED refresh tokens
/// (OWASP session-invalidation), so a refresh token they held before the
/// change can no longer be exchanged.
///
/// Two important subtleties this test accounts for:
///  1. `register`/`login` mint refresh tokens WITHOUT a `jti`, and the
///     refresh handler skips the whitelist for non-`jti` tokens — so a raw
///     registration token is structurally non-revocable. We exchange it
///     once via `/auth/refresh` to obtain a `jti`-bearing, whitelisted
///     token (the kind revocation actually governs).
///  2. `/auth/refresh` ROTATES (revokes the presented jti), so we must NOT
///     "warm up" the token we later assert on. A second untouched user is
///     the control: their refresh must still succeed, isolating the cause
///     of the failure below to the targeted revocation.
#[tokio::test]
async fn change_password_revokes_existing_refresh_tokens() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    let (access, refresh) = register_and_whitelist(&server, "cprevoke").await;
    let (_control_access, control_refresh) =
        register_and_whitelist(&server, "cpcontrol").await;

    // Change the first user's password.
    let res = client
        .post(server.api_url("/auth/password"))
        .header("Authorization", format!("Bearer {}", access))
        .json(&json!({
            "current_password": "password123",
            "new_password": "RotatedStrongPass456!",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);

    // The changing user's pre-change refresh token is now revoked.
    let post = client
        .post(server.api_url("/auth/refresh"))
        .json(&json!({ "refresh_token": refresh }))
        .send()
        .await
        .unwrap();
    assert_ne!(
        post.status(),
        200,
        "refresh token issued before the change must be revoked"
    );

    // Control: an unrelated user's refresh token still works, so the
    // failure above is the targeted revocation — not a broken endpoint
    // and not over-broad revocation across users.
    let control = client
        .post(server.api_url("/auth/refresh"))
        .json(&json!({ "refresh_token": control_refresh }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        control.status(),
        200,
        "an unrelated user's refresh must be unaffected"
    );
}

#[tokio::test]
async fn change_password_bumps_password_changed_at() {
    let server = crate::common::TestServer::start().await;
    let (token, _id, _u) = register(&server, "cpbump").await;
    let client = reqwest::Client::new();

    // First change: null → timestamp. (The access token stays valid after
    // a password change; only refresh tokens are revoked.)
    client
        .post(server.api_url("/auth/password"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "current_password": "password123",
            "new_password": "AnotherStrongPass789!",
        }))
        .send()
        .await
        .unwrap();
    let ts1 = get_me(&server, &token).await["user"]["password_changed_at"]
        .as_str()
        .expect("password_changed_at set after first change")
        .to_string();

    // Second change must bump the timestamp strictly forward (proves the
    // column is re-stamped on every change, not just first-set).
    client
        .post(server.api_url("/auth/password"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "current_password": "AnotherStrongPass789!",
            "new_password": "ThirdStrongPass012!",
        }))
        .send()
        .await
        .unwrap();
    let ts2 = get_me(&server, &token).await["user"]["password_changed_at"]
        .as_str()
        .expect("password_changed_at set after second change")
        .to_string();

    // RFC3339 timestamps compare lexicographically.
    assert!(ts2 > ts1, "password_changed_at must advance: {} !> {}", ts2, ts1);
}

#[tokio::test]
async fn change_password_wrong_current_returns_401() {
    let server = crate::common::TestServer::start().await;
    let (token, _id, _u) = register(&server, "cpwrong").await;

    let res = reqwest::Client::new()
        .post(server.api_url("/auth/password"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "current_password": "not-the-password",
            "new_password": "WhateverStrong123!",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401, "wrong current password must be 401");
}

#[tokio::test]
async fn change_password_weak_new_returns_400() {
    let server = crate::common::TestServer::start().await;
    let (token, _id, _u) = register(&server, "cpweak").await;

    let res = reqwest::Client::new()
        .post(server.api_url("/auth/password"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "current_password": "password123",
            "new_password": "short",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400, "new password < 8 chars must be 400");
}

#[tokio::test]
async fn change_password_no_local_password_returns_400() {
    let server = crate::common::TestServer::start().await;
    let (token, id, _u) = register(&server, "cpnolocal").await;

    // Simulate an OAuth/LDAP-only account: wipe the password hash but
    // keep the (still-valid) JWT.
    null_out_password_hash(&server, &id).await;

    let res = reqwest::Client::new()
        .post(server.api_url("/auth/password"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "current_password": "password123",
            "new_password": "DoesNotMatter123!",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400, "no-local-password account must be 400");
}

#[tokio::test]
async fn change_password_without_permission_returns_403() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_no_permissions(&server, "nochpw").await;

    let res = reqwest::Client::new()
        .post(server.api_url("/auth/password"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "current_password": "password123",
            "new_password": "WhateverStrong123!",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403, "without profile::edit must be 403");
}

#[tokio::test]
async fn change_password_requires_auth() {
    let server = crate::common::TestServer::start().await;
    let res = reqwest::Client::new()
        .post(server.api_url("/auth/password"))
        .json(&json!({
            "current_password": "password123",
            "new_password": "WhateverStrong123!",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401, "missing bearer must be 401");
}

// =====================================================
// GET /auth/me — has_password
// =====================================================

#[tokio::test]
async fn me_has_password_true_for_local_account() {
    let server = crate::common::TestServer::start().await;
    let (token, _id, _u) = register(&server, "haspw").await;

    let me = get_me(&server, &token).await;
    assert_eq!(
        me["has_password"],
        json!(true),
        "a registered local user has a password"
    );
    // The hash itself is never serialized.
    assert!(
        me["user"].get("password_hash").is_none(),
        "password_hash must never be exposed"
    );
}

#[tokio::test]
async fn me_has_password_false_for_external_account() {
    let server = crate::common::TestServer::start().await;
    let (token, id, _u) = register(&server, "nopw").await;
    null_out_password_hash(&server, &id).await;

    let me = get_me(&server, &token).await;
    assert_eq!(
        me["has_password"],
        json!(false),
        "an external-only account has no local password"
    );
    assert!(
        me["user"].get("password_hash").is_none(),
        "password_hash must never be exposed"
    );
}

/// ensure_unique_username's collision-retry loop (handlers.rs:1350-1382): a free
/// base is returned as-is; a taken base derives base2, base3, … by probing the
/// users table. (The 999-attempt exhaustion-→500 cap is the same loop's terminal;
/// seeding 999 users is impractical, but the retry mechanism is what this drives.)
#[tokio::test]
#[serial_test::serial(repos)]
async fn test_ensure_unique_username_collision_retry() {
    let server = crate::common::TestServer::start().await;
    // `auth_ensure_unique_username` runs IN-PROCESS against the global
    // `Repos` factory, so it must be pointed at THIS test's database — and
    // the in-process-Repos tests must be serialized (the factory is a single
    // process global). Same canonical pattern as
    // `auth::mod::test_ensure_unique_username_collision_suffix_and_defaults`.
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    ziee::init_repositories(pool);

    // Free base → returned unchanged.
    let fresh = ziee::auth_ensure_unique_username("brandnewbase")
        .await
        .expect("free base resolves");
    assert_eq!(fresh, "brandnewbase");

    // Register a user that OCCUPIES "collide" so the next derivation must bump.
    crate::common::test_helpers::create_user_with_permissions(&server, "collide", &[]).await;
    let bumped = ziee::auth_ensure_unique_username("collide")
        .await
        .expect("taken base resolves to a numbered variant");
    assert_eq!(bumped, "collide2", "a taken base must derive base2");

    // Occupy "collide2" too → must skip to "collide3".
    crate::common::test_helpers::create_user_with_permissions(&server, "collide2", &[]).await;
    let bumped3 = ziee::auth_ensure_unique_username("collide")
        .await
        .expect("resolves past the second collision");
    assert_eq!(bumped3, "collide3", "base + base2 taken → base3");
}
