//! `POST /api/remote-access/admin-password` integration tests.
//!
//! The endpoint is gated by `RemoteAccessManage` + the localhost-Host
//! middleware. It bypasses the standard change-password flow's
//! "verify current password" step — the localhost-Host gate is the
//! authentication proof. Always mutates the `admin` user.

use serde_json::json;

#[tokio::test]
async fn admin_password_requires_auth() {
    let server = crate::common::TestServer::start_desktop().await;
    let res = reqwest::Client::new()
        .post(server.api_url("/remote-access/admin-password"))
        .json(&json!({ "new_password": "ANewStrongPassword!1" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn admin_password_non_admin_forbidden() {
    let server = crate::common::TestServer::start_desktop().await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(
        &server, "ra_pw_nonadmin",
    )
    .await;
    let res = reqwest::Client::new()
        .post(server.api_url("/remote-access/admin-password"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "new_password": "ANewStrongPassword!1" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn admin_password_rejects_tunnel_host() {
    let server = crate::common::TestServer::start_desktop().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ra_pw_admin_for_tunnel",
        &["remote_access::read", "remote_access::manage"],
    )
    .await;
    let res = reqwest::Client::new()
        .post(server.api_url("/remote-access/admin-password"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .header("Host", "my-app.ngrok-free.app")
        .json(&json!({ "new_password": "ANewStrongPassword!1" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn admin_password_rejects_weak_password() {
    let server = crate::common::TestServer::start_desktop().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ra_pw_admin_weak",
        &["remote_access::read", "remote_access::manage"],
    )
    .await;
    // Also need a user named "admin" who IS is_admin in the DB for
    // the handler's NOT_ROOT_ADMIN gate. The create_user helper
    // suffixes usernames for uniqueness and doesn't set is_admin;
    // rename + flip the flag so the handler can find the row.
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    sqlx::query("UPDATE users SET username = 'admin', is_admin = TRUE WHERE id = $1")
        .bind(uuid::Uuid::parse_str(&admin.user_id).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    let res = reqwest::Client::new()
        .post(server.api_url("/remote-access/admin-password"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "new_password": "abc" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn admin_password_missing_body_field_returns_422() {
    let server = crate::common::TestServer::start_desktop().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ra_pw_admin_missing",
        &["remote_access::read", "remote_access::manage"],
    )
    .await;
    let res = reqwest::Client::new()
        .post(server.api_url("/remote-access/admin-password"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert!(matches!(res.status().as_u16(), 400 | 422));
}
