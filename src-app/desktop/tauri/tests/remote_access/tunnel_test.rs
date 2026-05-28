//! Tunnel start/stop preconditions. Real ngrok is NOT exercised here
//! (no auth token, no network); we only assert the handler's
//! validation gates fire correctly: 401/403 on auth, 422 when no
//! token is saved.

#[tokio::test]
async fn start_tunnel_requires_auth() {
    let server = crate::common::TestServer::start_desktop().await;
    let res = reqwest::Client::new()
        .post(server.api_url("/remote-access/tunnel/start"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn start_tunnel_non_admin_forbidden() {
    let server = crate::common::TestServer::start_desktop().await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(
        &server, "tun_nonadmin",
    )
    .await;
    let res = reqwest::Client::new()
        .post(server.api_url("/remote-access/tunnel/start"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn start_tunnel_no_token_returns_422() {
    let server = crate::common::TestServer::start_desktop().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tun_admin_nt",
        &["remote_access::read", "remote_access::manage"],
    )
    .await;
    let res = reqwest::Client::new()
        .post(server.api_url("/remote-access/tunnel/start"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        422,
        "tunnel start without saved token should 422"
    );
}

#[tokio::test]
async fn stop_tunnel_when_idle_is_idempotent() {
    let server = crate::common::TestServer::start_desktop().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tun_admin_stop",
        &["remote_access::read", "remote_access::manage"],
    )
    .await;
    let res = reqwest::Client::new()
        .post(server.api_url("/remote-access/tunnel/stop"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204, "stopping an idle tunnel should succeed");
}

#[tokio::test]
async fn status_reports_idle_when_nothing_running() {
    let server = crate::common::TestServer::start_desktop().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tun_admin_status",
        &["remote_access::read", "remote_access::manage"],
    )
    .await;
    let res = reqwest::Client::new()
        .get(server.api_url("/remote-access/status"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["tunnel_state"], "idle");
    assert_eq!(body["public_url"], serde_json::Value::Null);
}
