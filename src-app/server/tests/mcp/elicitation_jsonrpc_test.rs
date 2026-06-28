//! JSON-RPC route tests for the built-in `elicitation` MCP server at
//! `POST /api/elicitation/mcp` (`elicitation_mcp/handlers.rs::jsonrpc_handler`).
//!
//! The chat tool-loop intercepts `ask_user` and drives the elicitation inline
//! (it needs the live chat `sse_tx`), so the loopback `tools/call` branch is a
//! NEVER-HIT fallback that must fail loudly — an out-of-loop invocation has no
//! user form channel. `elicitation_mcp_test.rs` covers the in-loop chat path;
//! these drive the loopback handler directly, which no prior test does.

use serde_json::{json, Value};

use crate::common::test_helpers::create_user_with_permissions;
use crate::common::TestServer;

/// JSON-RPC POST to the elicitation built-in endpoint.
fn jsonrpc(
    server: &TestServer,
    token: &str,
    method: &str,
    params: Value,
) -> reqwest::RequestBuilder {
    reqwest::Client::new()
        .post(server.api_url("/elicitation/mcp"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "jsonrpc": "2.0", "id": 7, "method": method, "params": params }))
}

/// The `tools/call` fallback (handlers.rs:73-82): reaching it means `ask_user`
/// was invoked outside an interactive chat stream, so there is no user to ask.
/// It must return a JSON-RPC *result* (not a protocol error) whose payload is an
/// `isError: true` tool result carrying the "interactive chat turn" explanation,
/// so the caller fails loudly instead of silently no-op'ing.
#[tokio::test]
async fn tools_call_fallback_returns_is_error_result() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "elicit_fallback", &["mcp_servers::read"]).await;

    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "ask_user", "arguments": { "message": "pick one" } }),
    )
    .send()
    .await
    .expect("post tools/call");

    assert_eq!(res.status(), 200, "tools/call fallback is a 200 JSON-RPC envelope");
    let body: Value = res.json().await.expect("json body");
    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(body["id"], 7);
    // It is a JSON-RPC *result*, not a protocol-level error.
    assert!(body["error"].is_null(), "fallback must not be a JSON-RPC error: {body}");
    let result = &body["result"];
    assert_eq!(
        result["isError"], true,
        "the fallback tool result must be flagged as an error: {body}"
    );
    let text = result["content"][0]["text"]
        .as_str()
        .expect("fallback result carries explanatory text");
    assert!(
        text.contains("interactive chat turn"),
        "fallback text must explain ask_user can't run here; got: {text:?}"
    );
}

/// An unrelated method falls through to the `_ =>` arm and returns a
/// JSON-RPC method-not-found error (distinct from the `tools/call` fallback,
/// which is a successful is_error result).
#[tokio::test]
async fn unknown_method_returns_method_not_found() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "elicit_unknown", &["mcp_servers::read"]).await;

    let res = jsonrpc(&server, &user.token, "tools/nope", json!({}))
        .send()
        .await
        .expect("post unknown method");

    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.expect("json body");
    assert!(
        body["result"].is_null(),
        "an unknown method must not produce a result: {body}"
    );
    // JSON-RPC method-not-found is -32601.
    assert_eq!(body["error"]["code"], -32601, "method-not-found code: {body}");
}

/// The endpoint is permission-gated (`mcp_servers::read`); a request with no
/// Authorization header is rejected at the extractor before any dispatch.
#[tokio::test]
async fn tools_call_requires_auth() {
    let server = TestServer::start().await;

    let res = reqwest::Client::new()
        .post(server.api_url("/elicitation/mcp"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {} }))
        .send()
        .await
        .expect("post no-auth");

    assert_eq!(res.status(), 401, "missing Authorization must be 401");
}
