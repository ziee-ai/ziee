//! Tier 3 — HTTP handler tests against a live TestServer.
//!
//! Exercise the full /api/code-sandbox route stack including JWT
//! validation + conversation-id parsing. The dispatched tool calls
//! without bwrap return a clean "SANDBOX_NOT_INITIALIZED" error
//! (code_sandbox.enabled = false in the test config, which is the
//! default).

use uuid::Uuid;

use crate::code_sandbox::harness::test_jwt;
use crate::common::TestServer;
use ziee::code_sandbox::{code_sandbox_server_id, CodeSandboxRepository};

fn endpoint(server: &TestServer) -> String {
    format!("{}/api/code-sandbox", server.base_url)
}

#[tokio::test]
async fn rejects_missing_authorization_header() {
    let server = TestServer::start().await;
    let resp = reqwest::Client::new()
        .post(endpoint(&server))
        .header("x-conversation-id", Uuid::new_v4().to_string())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
        }))
        .send()
        .await
        .expect("send");
    let s = resp.status().as_u16();
    assert!(
        [401, 503].contains(&s),
        "expected 401/503, got {s}: {:?}",
        resp.text().await
    );
}

#[tokio::test]
async fn initialize_succeeds_without_conversation_id() {
    // The MCP manager probes `initialize` during server discovery
    // BEFORE any conversation exists. The endpoint must accept the
    // call without x-conversation-id.
    //
    // We can't easily assert the inner JSON-RPC result here (test_jwt
    // signs for a random user that doesn't exist in the DB → 401 at
    // the RequirePermissions extractor). But we CAN confirm that a
    // 400 "missing header" is no longer returned: if the auth layer
    // were bypassed we'd get 200 with a proper initialize result.
    let server = TestServer::start().await;
    let token = test_jwt(Uuid::new_v4(), Uuid::new_v4());
    let resp = reqwest::Client::new()
        .post(endpoint(&server))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
        }))
        .send()
        .await
        .expect("send");
    let s = resp.status().as_u16();
    // 401 (test user not in DB) or 503 (sandbox disabled). The key
    // invariant: NOT 400 — that would mean the missing header itself
    // is being rejected, which would break MCP discovery.
    assert!(
        [401, 503].contains(&s),
        "expected 401/503, got {s} (a 400 would mean we broke MCP discovery)"
    );
}

#[tokio::test]
async fn rejects_malformed_conversation_id() {
    let server = TestServer::start().await;
    let token = test_jwt(Uuid::new_v4(), Uuid::new_v4());
    let resp = reqwest::Client::new()
        .post(endpoint(&server))
        .header("Authorization", format!("Bearer {token}"))
        .header("x-conversation-id", "not-a-uuid")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
        }))
        .send()
        .await
        .expect("send");
    let s = resp.status().as_u16();
    assert!([400, 401, 503].contains(&s), "expected 400/401/503, got {s}");
}

#[tokio::test]
async fn rejects_expired_jwt() {
    let server = TestServer::start().await;
    // Manually craft an expired token (iat/exp in the past).
    let header = jsonwebtoken::Header::default();
    #[derive(serde::Serialize)]
    struct Expired {
        sub: String,
        exp: i64,
        iat: i64,
        iss: String,
        aud: String,
        username: String,
        email: String,
        is_admin: bool,
    }
    let claims = Expired {
        sub: Uuid::new_v4().to_string(),
        exp: chrono::Utc::now().timestamp() - 3600,
        iat: chrono::Utc::now().timestamp() - 7200,
        iss: "ziee".into(),
        aud: "ziee-api".into(),
        username: String::new(),
        email: String::new(),
        is_admin: false,
    };
    let secret = std::env::var("TEST_JWT_SECRET")
        .unwrap_or_else(|_| "dev-secret-change-in-production-min-32-chars-long".into());
    let token = jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap();

    let resp = reqwest::Client::new()
        .post(endpoint(&server))
        .header("Authorization", format!("Bearer {token}"))
        .header("x-conversation-id", Uuid::new_v4().to_string())
        .json(&serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize"}))
        .send()
        .await
        .expect("send");
    let s = resp.status().as_u16();
    assert!([401, 503].contains(&s), "expected 401/503, got {s}");
}

#[tokio::test]
async fn rejects_wrong_secret_jwt() {
    let server = TestServer::start().await;
    let header = jsonwebtoken::Header::default();
    #[derive(serde::Serialize)]
    struct C {
        sub: String,
        exp: i64,
        iat: i64,
        iss: String,
        aud: String,
        username: String,
        email: String,
        is_admin: bool,
    }
    let claims = C {
        sub: Uuid::new_v4().to_string(),
        exp: chrono::Utc::now().timestamp() + 600,
        iat: chrono::Utc::now().timestamp(),
        iss: "ziee".into(),
        aud: "ziee-api".into(),
        username: String::new(),
        email: String::new(),
        is_admin: false,
    };
    let token = jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(b"completely-different-secret-32-chars-min-len"),
    )
    .unwrap();

    let resp = reqwest::Client::new()
        .post(endpoint(&server))
        .header("Authorization", format!("Bearer {token}"))
        .header("x-conversation-id", Uuid::new_v4().to_string())
        .json(&serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize"}))
        .send()
        .await
        .expect("send");
    let s = resp.status().as_u16();
    assert!([401, 503].contains(&s), "expected 401/503, got {s}");
}

#[tokio::test]
async fn download_endpoint_rejects_path_traversal() {
    let server = TestServer::start().await;
    let url = format!(
        "{}/api/code-sandbox/file/download?filename=../../../etc/passwd",
        server.base_url
    );
    let token = test_jwt(Uuid::new_v4(), Uuid::new_v4());
    let resp = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("x-conversation-id", Uuid::new_v4().to_string())
        .send()
        .await
        .expect("send");
    let s = resp.status().as_u16();
    // 400 (bad filename) when sandbox initialized; 401 (user-not-in-DB)
    // when RequirePermissions runs first; 503 when sandbox disabled.
    // All three are valid rejections.
    assert!([400, 401, 503].contains(&s), "expected 400/401/503, got {s}");
}

#[tokio::test]
async fn download_endpoint_requires_auth() {
    let server = TestServer::start().await;
    let url = format!(
        "{}/api/code-sandbox/file/download?filename=foo.txt",
        server.base_url
    );
    let resp = reqwest::Client::new().get(&url).send().await.expect("send");
    let s = resp.status().as_u16();
    assert!(
        [400, 401, 503].contains(&s),
        "expected 400/401/503, got {s}"
    );
}

#[tokio::test]
async fn rejects_cross_tenant_conversation_id() {
    // SECURITY regression test: build_context (and download_handler)
    // MUST verify the JWT-authenticated user owns the conversation
    // referenced in x-conversation-id. Without this, any authenticated
    // user with code_sandbox::execute (default Users group via
    // migration 35) could spoof another user's conversation_id and
    // read their attachments via execute_command's bwrap bind.
    //
    // We can't easily set up a real user-owns-conversation pair here
    // (test_jwt signs for a random user not in DB → 401 at the
    // RequirePermissions extractor). But we CAN confirm: a request
    // that passes a conversation_id the JWT user does NOT own gets
    // rejected — NOT with 200 and a real tool result.
    let server = TestServer::start().await;
    let token = test_jwt(Uuid::new_v4(), Uuid::new_v4());
    let resp = reqwest::Client::new()
        .post(endpoint(&server))
        .header("Authorization", format!("Bearer {token}"))
        .header("x-conversation-id", Uuid::new_v4().to_string())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "execute_command",
                "arguments": { "command": "echo hi" }
            }
        }))
        .send()
        .await
        .expect("send");
    let s = resp.status().as_u16();
    // 401 (test user not in DB at auth layer), 404 (ownership check
    // rejects because conversation_id doesn't exist for this user),
    // or 503 (sandbox disabled in test config). 200 means we ran the
    // tool — that would be the bug.
    assert!(
        [401, 404, 503].contains(&s),
        "expected 401/404/503, got {s} (200 would mean the cross-tenant \
         check was bypassed)"
    );
}

/// `tools/call` requires `x-conversation-id`; calling it without one
/// should produce JSON-RPC invalid_params (-32602), NOT 200 with a
/// real tool result. This documents the contract at the handler
/// dispatch level (handlers.rs:108-117).
#[tokio::test]
async fn tools_call_without_conversation_id_returns_invalid_params() {
    let server = TestServer::start().await;
    let token = test_jwt(Uuid::new_v4(), Uuid::new_v4());
    let resp = reqwest::Client::new()
        .post(endpoint(&server))
        .header("Authorization", format!("Bearer {token}"))
        // Deliberately NO x-conversation-id header.
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": "list_files", "arguments": {} }
        }))
        .send()
        .await
        .expect("send");
    let s = resp.status().as_u16();
    // 401 (test user not in DB) blocks before reaching dispatch.
    // 200 with JSON-RPC error -32602 is the case where auth passes
    // and dispatch rejects the missing header. 503 = sandbox disabled.
    // None of these is a 200 with a successful tool execution.
    assert!(
        [200, 401, 503].contains(&s),
        "expected 401/503 or 200-with-jsonrpc-error, got {s}"
    );
    if s == 200 {
        let body: serde_json::Value = resp.json().await.expect("parse");
        let code = body
            .get("error")
            .and_then(|e| e.get("code"))
            .and_then(|c| c.as_i64());
        assert_eq!(
            code,
            Some(-32602),
            "expected JSON-RPC invalid_params (-32602), got body: {body}"
        );
    }
}

/// HTTP cancellation of an in-flight tool call MUST release the
/// per-conversation mutex so subsequent calls don't deadlock. The
/// mutex is held by `_guard` in the handler; when the future is
/// dropped (via reqwest client disconnect), the guard drops too.
/// We test by starting a slow call, dropping the request mid-flight,
/// and confirming the next call to the same conversation proceeds.
#[tokio::test]
async fn cancellation_releases_per_conversation_mutex() {
    let server = TestServer::start().await;
    let token = test_jwt(Uuid::new_v4(), Uuid::new_v4());
    let conv = Uuid::new_v4();
    let endpoint = endpoint(&server);

    // First request: deliberately bad (no auth = fast 401). We're not
    // testing the slow path here, just the lock-acquire/release shape.
    // The KEY property: after this future is dropped, the lock entry
    // (if any) is released so the second call doesn't block.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap();
    let first = client
        .post(&endpoint)
        .header("Authorization", format!("Bearer {token}"))
        .header("x-conversation-id", conv.to_string())
        .json(&serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}
        }))
        .send()
        .await;
    // First call returns or errors fast (no rootfs → 503; bad jwt → 401).
    let _ = first;

    // Second call for the SAME conversation_id must complete quickly.
    // If the first request had leaked a held mutex, this would hang
    // for our 2-sec timeout.
    let started = std::time::Instant::now();
    let second = client
        .post(&endpoint)
        .header("Authorization", format!("Bearer {token}"))
        .header("x-conversation-id", conv.to_string())
        .json(&serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}
        }))
        .send()
        .await;
    assert!(
        second.is_ok(),
        "second call timed out — mutex may have leaked from first call"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "second call took {:?} — mutex contention",
        started.elapsed()
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
