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

/// Harness-level routing/auth smoke: an unknown `/api/*` path returns a clean
/// 404 (the fallback handler, not a panic / 5xx), and a permission-gated route
/// hit WITHOUT a token returns 401 (the auth layer runs before the handler).
/// These cross-cutting guarantees underpin every module's tests but weren't
/// asserted at the top level.
#[tokio::test]
async fn unknown_route_404_and_protected_route_401() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // Unknown API path → 404 (no token needed; routing fallback).
    let missing = client
        .get(server.api_url("/this-route-does-not-exist-xyz"))
        .send()
        .await
        .expect("missing route request");
    assert_eq!(missing.status(), 404, "unknown route must 404, got {}", missing.status());

    // A real, permission-gated route with NO Authorization header → 401
    // (the JWT auth layer rejects before the handler/permission check).
    let unauthed = client
        .get(server.api_url("/conversations"))
        .send()
        .await
        .expect("unauthed request");
    assert_eq!(
        unauthed.status(),
        401,
        "a protected route must 401 without a token, got {}",
        unauthed.status()
    );
}
