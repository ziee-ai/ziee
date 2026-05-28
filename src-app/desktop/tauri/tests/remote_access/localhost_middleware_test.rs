//! Localhost-Host middleware test — every `/api/remote-access/*`
//! route + the magic-link issue endpoint MUST reject requests whose
//! `Host` header is not 127.0.0.1 / localhost.

use serde_json::json;

#[tokio::test]
async fn remote_access_status_rejects_tunnel_host() {
    let server = crate::common::TestServer::start_desktop().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ra_admin_mw",
        &["remote_access::read", "remote_access::manage"],
    )
    .await;

    let res = reqwest::Client::new()
        .get(server.api_url("/remote-access/status"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .header("Host", "my-app.ngrok.app")
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        403,
        "tunneled request should be rejected by localhost middleware"
    );
}

#[tokio::test]
async fn remote_access_settings_get_rejects_tunnel_host() {
    let server = crate::common::TestServer::start_desktop().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ra_admin_mw_g",
        &["remote_access::read", "remote_access::manage"],
    )
    .await;

    let res = reqwest::Client::new()
        .get(server.api_url("/remote-access/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .header("Host", "abc123.ngrok-free.app")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn remote_access_settings_put_rejects_tunnel_host() {
    let server = crate::common::TestServer::start_desktop().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ra_admin_mw_p",
        &["remote_access::read", "remote_access::manage"],
    )
    .await;

    let res = reqwest::Client::new()
        .put(server.api_url("/remote-access/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .header("Host", "my-app.ngrok.app")
        .json(&json!({ "ngrok_auth_token": "x" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn magic_link_issue_rejects_tunnel_host() {
    let server = crate::common::TestServer::start_desktop().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ra_admin_mw_issue",
        &["remote_access::read", "remote_access::manage"],
    )
    .await;

    let res = reqwest::Client::new()
        .post(server.api_url("/auth/magic-link/issue"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .header("Host", "my-app.ngrok.app")
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        403,
        "magic-link issue should reject tunneled hosts"
    );
}

#[tokio::test]
async fn localhost_host_passes_through() {
    let server = crate::common::TestServer::start_desktop().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ra_admin_mw_ok",
        &["remote_access::read", "remote_access::manage"],
    )
    .await;

    // No explicit Host header → reqwest sets it to the request URL's
    // host (which is 127.0.0.1 for TestServer). The default flow
    // SHOULD succeed.
    let res = reqwest::Client::new()
        .get(server.api_url("/remote-access/status"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "localhost request should be allowed; got {}",
        res.status()
    );
}
