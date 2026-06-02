//! Regression test for the GitHub "Test connection" 403.
//!
//! The connectivity probe (`test_repository_connectivity`) must send a non-empty
//! User-Agent: GitHub's REST API rejects any UA-less request with 403 Forbidden
//! *before* it ever evaluates the token, so a perfectly valid token would fail
//! the connection test with a misleading 403. This test stands up a loopback
//! server, points a repository's `auth_test_api_endpoint` at it, and asserts the
//! probe sent a non-empty `ziee/<version>` User-Agent.

use std::sync::{Arc, Mutex};

use axum::{extract::State, http::HeaderMap, routing::get, Router};
use serde_json::json;

#[tokio::test]
async fn test_repository_connection_sends_user_agent() {
    // Loopback server that records the User-Agent of the request it receives and
    // returns 200 (the only status the probe treats as success).
    let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let app = Router::new()
        .route(
            "/whoami",
            get(
                |State(state): State<Arc<Mutex<Option<String>>>>, headers: HeaderMap| async move {
                    let ua = headers
                        .get("user-agent")
                        .and_then(|v| v.to_str().ok())
                        .map(|s| s.to_string());
                    *state.lock().unwrap() = ua;
                    "ok"
                },
            ),
        )
        .with_state(captured.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    // Abort the loopback server on scope exit — success OR panic-unwind — so a
    // pre-teardown assertion failure cannot leak the spawned task.
    struct AbortOnDrop(tokio::task::JoinHandle<()>);
    impl Drop for AbortOnDrop {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let _server = AbortOnDrop(tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    }));
    let endpoint = format!("http://127.0.0.1:{}/whoami", addr.port());

    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "repo_ua",
        &["llm_repositories::create", "llm_repositories::read"],
    )
    .await;

    // Mirror the built-in GitHub repo shape (bearer_token), but point the test
    // endpoint at the loopback so we can inspect the request headers.
    let response = reqwest::Client::new()
        .post(server.api_url("/llm-repositories/test"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "UA Probe",
            "url": "https://api.github.com",
            "auth_type": "bearer_token",
            "auth_config": {
                "token": "dummy-token",
                "auth_test_api_endpoint": endpoint,
            }
        }))
        .send()
        .await
        .expect("request failed");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body["success"].as_bool(),
        Some(true),
        "connectivity should succeed against the loopback (HTTP 200): {body}"
    );

    let ua = captured
        .lock()
        .unwrap()
        .clone()
        .expect("loopback should have received a User-Agent header");
    assert!(!ua.is_empty(), "User-Agent must be non-empty");
    assert!(
        ua.starts_with("ziee/"),
        "expected a ziee/<version> User-Agent, got {ua:?}"
    );

    // `_server` (AbortOnDrop) tears the loopback down on scope exit.
}
