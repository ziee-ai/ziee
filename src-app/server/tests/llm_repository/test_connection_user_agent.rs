//! Regression test for the GitHub "Test connection" 403.
//!
//! The connectivity probe (`test_repository_connectivity`) must send a non-empty
//! User-Agent: GitHub's REST API rejects any UA-less request with 403 Forbidden
//! *before* it ever evaluates the token, so a perfectly valid token would fail
//! the connection test with a misleading 403. This test stands up a loopback
//! server, points a repository's `auth_test_api_endpoint` at it, and asserts the
//! probe sent a non-empty `ziee/<version>` User-Agent.

use std::sync::{Arc, Mutex};

use axum::{Router, extract::State, http::HeaderMap, routing::get};
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

    // Point the GitHub capability-probe base at the same loopback (the
    // debug-only `LLM_REPOSITORY_GITHUB_API_BASE` seam) so this regression
    // test stays offline while keeping the real `https://api.github.com`
    // repository URL that gives it its GitHub shape.
    let server = crate::common::TestServer::start_with_options(crate::common::TestServerOptions {
        extra_env: vec![(
            "LLM_REPOSITORY_GITHUB_API_BASE".to_string(),
            format!("http://127.0.0.1:{}", addr.port()),
        )],
        ..Default::default()
    })
    .await;
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

    // DEC-15 — this spec's assertion is re-scoped to the User-Agent header it
    // exists to prove. It used to assert `success: true` because the loopback
    // answered 200 with the body `"ok"`; that is precisely the "a socket
    // answered 200 ⇒ healthy" defect. The probe now additionally requires a
    // confirmed model listing, which a `/whoami` endpoint does not serve, so
    // the OUTCOME here is deliberately not asserted — the credential step
    // (which is what carries the User-Agent to the loopback) is what this test
    // covers. `body` is still parsed so a malformed response is caught.
    assert!(
        body.get("status").is_some(),
        "the probe response must carry a health status: {body}"
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
