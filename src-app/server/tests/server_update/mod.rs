//! Integration tests for the server self-update notification endpoint.
//!
//! Covers the auth/permission gate, the air-gapped (disabled) path, and the
//! full mock-GitHub → checker → `/api/server-update/status` path via the
//! debug-only `SERVER_UPDATE_API_MIRROR` seam (no real GitHub call). The test
//! harness disables update_check by default, so only the mock test opts in.

use std::time::Duration;

use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::common::{TestServer, TestServerOptions, test_helpers};

const PERM: &str = "server_update::read";

fn status_url(server: &TestServer) -> String {
    server.api_url("/server-update/status")
}

/// Boot a server whose update checker polls a loopback mock returning `tag` as
/// the latest release, then poll the admin status endpoint until the boot check
/// has run, and return the parsed body.
async fn status_with_mock_tag(tag: &'static str) -> Value {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
    let addr = listener.local_addr().unwrap();
    let mirror = format!("http://{addr}");
    let body = json!({
        "tag_name": tag,
        "html_url": format!("https://github.com/phibya/ziee-chat-new/releases/tag/{tag}"),
        "body": "notes"
    });
    let app = axum::Router::new().route(
        "/repos/phibya/ziee-chat-new/releases/latest",
        axum::routing::get(move || {
            let body = body.clone();
            async move { axum::Json(body) }
        }),
    );
    let _mock = tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });

    let server = TestServer::start_with_options(TestServerOptions {
        update_check_enabled: Some(true),
        extra_env: vec![("SERVER_UPDATE_API_MIRROR".to_string(), mirror)],
        ..Default::default()
    })
    .await;
    let token = test_helpers::create_user_with_permissions(&server, "su_mock", &[PERM])
        .await
        .token;
    let client = reqwest::Client::new();
    let mut out = Value::Null;
    for _ in 0..30 {
        let resp = client
            .get(status_url(&server))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .expect("request");
        out = resp.json().await.expect("json");
        if !out["checked_at"].is_null() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    assert!(!out["checked_at"].is_null(), "checker never ran: {out}");
    out
}

#[tokio::test]
async fn up_to_date_via_mock_github() {
    // An older latest release than the running version → no update.
    let body = status_with_mock_tag("v0.0.1").await;
    assert_eq!(body["update_available"], json!(false), "body: {body}");
    assert_eq!(body["latest_version"], json!("0.0.1"));
    assert_eq!(body["enabled"], json!(true));
}

#[tokio::test]
async fn non_semver_tag_is_ignored() {
    // A garbage release name must not surface a "latest" or a false banner.
    let body = status_with_mock_tag("nightly-build").await;
    assert_eq!(body["update_available"], json!(false), "body: {body}");
    assert!(body["latest_version"].is_null(), "non-semver tag dropped: {body}");
    assert!(body["release_url"].is_null());
}

#[tokio::test]
async fn status_requires_authentication() {
    let server = TestServer::start().await;
    let resp = reqwest::Client::new()
        .get(status_url(&server))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn status_requires_permission() {
    let server = TestServer::start().await;
    // Authenticated but holds an unrelated permission → 403.
    let token = test_helpers::create_user_with_permissions(
        &server,
        "su_noperm",
        &["code_sandbox::resource_limits::read"],
    )
    .await
    .token;
    let resp = reqwest::Client::new()
        .get(status_url(&server))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 403);
}

#[tokio::test]
async fn admin_sees_status_with_checks_disabled() {
    // Default harness config disables update_check (air-gapped): the endpoint
    // still serves the current version + enabled:false and never polled.
    let server = TestServer::start().await;
    let token = test_helpers::create_user_with_permissions(&server, "su_admin", &[PERM])
        .await
        .token;
    let resp = reqwest::Client::new()
        .get(status_url(&server))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 200);
    let body: Value = resp.json().await.expect("json");
    assert!(
        !body["current_version"].as_str().unwrap_or("").is_empty(),
        "current_version should be populated: {body}"
    );
    assert_eq!(body["enabled"], json!(false), "checks disabled by config");
    assert_eq!(body["update_available"], json!(false));
    assert!(body["checked_at"].is_null(), "no check ran: {body}");
}

#[tokio::test]
async fn detects_update_via_mock_github() {
    // Loopback mock of GitHub's `releases/latest`, returning a much newer tag.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
    let addr = listener.local_addr().unwrap();
    let mirror = format!("http://{addr}");
    let app = axum::Router::new().route(
        "/repos/phibya/ziee-chat-new/releases/latest",
        axum::routing::get(|| async {
            axum::Json(json!({
                "tag_name": "v99.0.0",
                "html_url": "https://github.com/phibya/ziee-chat-new/releases/tag/v99.0.0",
                "body": "A big new release"
            }))
        }),
    );
    let _mock = tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });

    // Enable update_check AND point it at the mock (so the boot check hits the
    // mock, not GitHub).
    let server = TestServer::start_with_options(TestServerOptions {
        update_check_enabled: Some(true),
        extra_env: vec![("SERVER_UPDATE_API_MIRROR".to_string(), mirror)],
        ..Default::default()
    })
    .await;
    let token = test_helpers::create_user_with_permissions(&server, "su_admin2", &[PERM])
        .await
        .token;

    // Poll until the boot check completes (checked_at populated).
    let client = reqwest::Client::new();
    let mut body = Value::Null;
    for _ in 0..30 {
        let resp = client
            .get(status_url(&server))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .expect("request");
        body = resp.json().await.expect("json");
        if !body["checked_at"].is_null() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    assert!(!body["checked_at"].is_null(), "checker never ran: {body}");
    assert_eq!(body["update_available"], json!(true), "body: {body}");
    assert_eq!(body["latest_version"], json!("99.0.0"));
    assert_eq!(
        body["release_url"],
        json!("https://github.com/phibya/ziee-chat-new/releases/tag/v99.0.0")
    );
    assert_eq!(body["notes"], json!("A big new release"));
    assert_eq!(body["enabled"], json!(true));
}
