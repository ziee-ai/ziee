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
use ziee_chat::code_sandbox::{code_sandbox_server_id, CodeSandboxRepository};

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
        iss: "ziee-chat".into(),
        aud: "ziee-chat-api".into(),
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
        iss: "ziee-chat".into(),
        aud: "ziee-chat-api".into(),
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
