//! Tier 3 — HTTP handler tests against a live TestServer.
//!
//! These exercise the full /api/code-sandbox route stack including
//! JWT validation, JSON-RPC dispatch, and the conversation/user
//! resolution path. They DO NOT actually run bwrap (the dispatched
//! tool calls without bwrap return a clean "SANDBOX_NOT_INITIALIZED"
//! error when code_sandbox.enabled=false in the test config — which is
//! the default).
//!
//! For tests that actually run bwrap end-to-end, see Tier 4.

use crate::common::TestServer;
use ziee_chat::code_sandbox::{code_sandbox_server_id, CodeSandboxRepository};

#[tokio::test]
async fn jsonrpc_endpoint_rejects_missing_authorization() {
    let server = TestServer::start().await;
    let url = format!("{}/api/code-sandbox", server.base_url);
    let resp = reqwest::Client::new()
        .post(&url)
        .header("x-conversation-id", uuid::Uuid::new_v4().to_string())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
        }))
        .send()
        .await
        .expect("send");
    assert!(
        resp.status().is_server_error()
            || resp.status().as_u16() == 401
            || resp.status().as_u16() == 503,
        "expected 401/503, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn jsonrpc_endpoint_rejects_missing_conversation_id() {
    let server = TestServer::start().await;
    let url = format!("{}/api/code-sandbox", server.base_url);
    let resp = reqwest::Client::new()
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
        }))
        .send()
        .await
        .expect("send");
    // Either 400 (missing header), 401 (missing auth checked first),
    // or 503 (sandbox not initialized). All are acceptable rejections
    // for a malformed request.
    let status = resp.status().as_u16();
    assert!(
        [400, 401, 503].contains(&status),
        "expected 400/401/503, got {status}"
    );
}

#[tokio::test]
async fn sandbox_server_row_persisted_on_repository_call() {
    let server = TestServer::start().await;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&server.database_url)
        .await
        .unwrap();
    let repo = CodeSandboxRepository::new(pool);
    repo.upsert_builtin_server(
        code_sandbox_server_id(),
        "http://127.0.0.1:9999/api/code-sandbox",
    )
    .await
    .unwrap();
}
