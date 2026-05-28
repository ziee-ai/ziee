//! `GET /api/auth/config` integration test.

use serde_json::Value;

#[tokio::test]
async fn auth_config_localhost_returns_multi_user_flags() {
    let server = crate::common::TestServer::start_desktop().await;
    let res = reqwest::Client::new()
        .get(server.api_url("/auth/config"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    // Default Host header is 127.0.0.1 (the test server origin) →
    // multi-user UX flags.
    assert_eq!(body["password_auth_enabled"], true);
    assert_eq!(body["magic_link_enabled"], false);
    assert_eq!(body["hide_username"], false);
}

#[tokio::test]
async fn auth_config_tunneled_reflects_settings() {
    let server = crate::common::TestServer::start_desktop().await;
    let res = reqwest::Client::new()
        .get(server.api_url("/auth/config"))
        .header("Host", "my-app.ngrok.app")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    // Default settings: password_auth_enabled=false.
    assert_eq!(body["password_auth_enabled"], false);
    assert_eq!(body["magic_link_enabled"], true);
    assert_eq!(body["hide_username"], true);
}
