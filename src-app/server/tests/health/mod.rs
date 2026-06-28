//! Integration coverage for the health module: GET /api/health returns 200
//! with the documented `{ "status": "ok" }` body. The endpoint is unauthed
//! (used by load balancers / the test harness readiness probe), so this also
//! asserts it is reachable without a token.

use serde_json::Value;

use crate::common::TestServer;

#[tokio::test]
async fn health_endpoint_returns_ok_without_auth() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    let res = client
        .get(server.api_url("/health"))
        .send()
        .await
        .expect("health request");

    assert_eq!(res.status(), 200, "health endpoint must return 200");

    let body: Value = res.json().await.expect("health body is json");
    assert_eq!(
        body["status"], "ok",
        "health body must be {{\"status\":\"ok\"}}, got {body}"
    );
}
