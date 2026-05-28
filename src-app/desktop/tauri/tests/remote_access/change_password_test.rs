//! `POST /api/users/me/password` integration tests — the
//! authenticated change-password endpoint that bumps
//! `users.password_changed_at`. Lives in the desktop crate because
//! its only consumer is the Remote Access password-auth gate (only
//! the desktop migrates `password_changed_at`).

use serde_json::json;

#[tokio::test]
async fn change_password_requires_auth() {
    let server = crate::common::TestServer::start_desktop().await;
    let res = reqwest::Client::new()
        .post(server.api_url("/users/me/password"))
        .json(&json!({ "current_password": "x", "new_password": "y" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn change_password_happy_path_allows_login_with_new() {
    let server = crate::common::TestServer::start_desktop().await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(
        &server, "cp_happy",
    )
    .await;

    // Pull the actual username back out of the response — the helper
    // appends a UUID suffix.
    let me_res = reqwest::Client::new()
        .get(server.api_url("/auth/me"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    let me_body: serde_json::Value = me_res.json().await.unwrap();
    let username = me_body["user"]["username"]
        .as_str()
        .expect("username in /me")
        .to_string();

    let res = reqwest::Client::new()
        .post(server.api_url("/users/me/password"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "current_password": "password123",
            "new_password": "NewStrongPassword456!"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);

    // Login with the new password works.
    let login_res = reqwest::Client::new()
        .post(server.api_url("/auth/login"))
        .json(&json!({
            "username": username,
            "password": "NewStrongPassword456!"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(login_res.status(), 200);
}

#[tokio::test]
async fn change_password_wrong_current_returns_401() {
    let server = crate::common::TestServer::start_desktop().await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(
        &server, "cp_wrong",
    )
    .await;

    let res = reqwest::Client::new()
        .post(server.api_url("/users/me/password"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "current_password": "DefinitelyNotMyPassword",
            "new_password": "NewStrongPassword456!"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn change_password_weak_new_returns_400() {
    let server = crate::common::TestServer::start_desktop().await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(
        &server, "cp_weak",
    )
    .await;

    let res = reqwest::Client::new()
        .post(server.api_url("/users/me/password"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "current_password": "password123",
            "new_password": "abc"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}
