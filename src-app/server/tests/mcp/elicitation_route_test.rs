//! HTTP route tests for `POST /api/mcp/elicitation/{id}/respond`.
//!
//! Scope: negative-path cases that don't require a pre-registered
//! elicitation in the server-process's in-memory registry. The
//! TestServer spawns the backend as a child process, so the registry the
//! test sees and the registry the server sees are different `Lazy`
//! instances — we can't pre-populate it from here.
//!
//! Happy-path (accept/decline/cancel that successfully delivers to the
//! registry) is exercised end-to-end by the chat-level elicitation
//! integration test, which goes through a real tool-call SSE roundtrip
//! so the registry is populated in the server process.

use crate::common::test_helpers;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn test_elicitation_respond_unknown_id_returns_404() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::read"])
        .await;

    let unknown_id = Uuid::new_v4();
    let url = server.api_url(&format!("/mcp/elicitation/{}/respond", unknown_id));
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "action": "accept", "content": {} }))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 404,
               "Unknown elicitation_id must return 404 (not 500/200)");
}

#[tokio::test]
async fn test_elicitation_respond_invalid_action_returns_400() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::read"])
        .await;

    // The handler validates `action` BEFORE looking up the registry, so a
    // bad action returns 400 even when the id isn't registered.
    let elicitation_id = Uuid::new_v4();
    let url = server.api_url(&format!("/mcp/elicitation/{}/respond", elicitation_id));
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "action": "yolo" }))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 400, "Invalid action must return 400 (before registry lookup)");
    let body: serde_json::Value = response.json().await.expect("parse");
    assert_eq!(body["error_code"], "INVALID_ACTION");
}

#[tokio::test]
async fn test_elicitation_respond_permission_required() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_no_permissions(&server, "user").await;

    let elicitation_id = Uuid::new_v4();
    let url = server.api_url(&format!("/mcp/elicitation/{}/respond", elicitation_id));
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "action": "accept", "content": {} }))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403,
               "User without mcp_servers::read must get 403, not 404");
    let body: serde_json::Value = response.json().await.expect("parse");
    assert_eq!(body["error_code"], "INSUFFICIENT_PERMISSIONS");
}

#[tokio::test]
async fn test_elicitation_respond_no_auth_header_returns_401() {
    let server = crate::common::TestServer::start().await;
    let elicitation_id = Uuid::new_v4();

    let url = server.api_url(&format!("/mcp/elicitation/{}/respond", elicitation_id));
    let response = reqwest::Client::new()
        .post(&url)
        .json(&json!({ "action": "accept", "content": {} }))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 401, "Missing Authorization header must return 401");
}
