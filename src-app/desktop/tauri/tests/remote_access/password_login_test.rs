//! `POST /api/auth/login-password-only` integration tests.
//!
//! Phone-facing password login endpoint. Unauthenticated. Gated by
//! `password_auth_enabled` on the singleton settings row. Uses
//! timing-equalized bcrypt verification against the admin user
//! (constant-time wrt admin-exists-or-not).

use serde_json::json;

#[tokio::test]
async fn password_login_disabled_by_default_returns_403() {
    let server = crate::common::TestServer::start_desktop().await;
    let res = reqwest::Client::new()
        .post(server.api_url("/auth/login-password-only"))
        // Non-localhost Host triggers the disabled-flag check.
        .header("Host", "my-app.ngrok-free.app")
        .json(&json!({ "password": "whatever" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn password_login_missing_body_field_returns_422() {
    let server = crate::common::TestServer::start_desktop().await;
    let res = reqwest::Client::new()
        .post(server.api_url("/auth/login-password-only"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert!(matches!(res.status().as_u16(), 400 | 422));
}

#[tokio::test]
async fn password_login_when_enabled_but_no_admin_returns_401() {
    let server = crate::common::TestServer::start_desktop().await;
    // Enable password_auth via direct DB UPDATE (bypassing the
    // "password_rotated?" invariant check that the handler enforces;
    // we want to exercise the login endpoint specifically).
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    sqlx::query("UPDATE remote_access_settings SET password_auth_enabled = TRUE")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    let res = reqwest::Client::new()
        .post(server.api_url("/auth/login-password-only"))
        .json(&json!({ "password": "any-password-no-admin-exists-here" }))
        .send()
        .await
        .unwrap();
    // 401 — no admin user exists in test DB, so bcrypt fails on the
    // dummy hash and the timing-equalized branch returns
    // INVALID_CREDENTIALS rather than leaking "user not found".
    assert_eq!(res.status(), 401);
}
