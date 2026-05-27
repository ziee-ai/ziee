//! Plan-3 Phase-4 (Cos1) — OAuth 2.1 `client_credentials` client conformance.
//!
//! Drives the headless flow against the mock, which co-locates the MCP server,
//! its RFC 9728 protected-resource metadata, the RFC 8414 authorization-server
//! metadata, and the token endpoint:
//!
//!   initialize → 401 + WWW-Authenticate → fetch PRM → fetch AS metadata →
//!   POST /token (Basic client auth + client_credentials) → retry initialize
//!   with `Authorization: Bearer` → success.

use super::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};
use ziee::{HttpMcpClient, McpClient, McpServer, OAuthClientConfig, TransportType, UsageMode};

fn server_config(url: String) -> McpServer {
    McpServer {
        id: uuid::Uuid::new_v4(),
        user_id: None,
        name: "mock-mcp-oauth".to_string(),
        display_name: "Mock MCP (oauth fixture)".to_string(),
        description: None,
        enabled: true,
        is_system: false,
        transport_type: TransportType::Http,
        command: None,
        args: serde_json::json!([]),
        environment_variables: serde_json::json!({}),
        url: Some(url),
        headers: serde_json::json!({}),
        timeout_seconds: 10,
        supports_sampling: false,
        usage_mode: UsageMode::Auto,
        max_concurrent_sessions: None,
        is_built_in: false,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

fn oauth(client_id: &str, secret: &str) -> OAuthClientConfig {
    OAuthClientConfig {
        client_id: client_id.to_string(),
        client_secret: secret.to_string(),
        scopes: Some("mcp".to_string()),
        resource: None,
    }
}

// ─── Cos1: 401 → discover → token → retry → success ──────────────────────────

#[tokio::test]
async fn connect_runs_client_credentials_flow_on_401() {
    let mock = MockMcpServer::start().await;
    mock.enable_oauth("mcp-client", "mcp-secret", "mock-access-token");

    let mut client =
        HttpMcpClient::new_with_oauth(server_config(mock.base_url()), oauth("mcp-client", "mcp-secret"))
            .unwrap();

    // initialize 401s, the client acquires a token and retries → connect OK.
    client
        .connect()
        .await
        .expect("client must run the OAuth flow and connect");

    // The token endpoint was hit with a client_credentials grant + Basic auth.
    let received = mock.received();
    let token_req = received
        .iter()
        .find(|r| r.method == "__token")
        .expect("client must POST the token endpoint");
    assert!(
        token_req
            .body
            .as_str()
            .map(|b| b.contains("grant_type=client_credentials"))
            .unwrap_or(false),
        "token request must use the client_credentials grant"
    );
    assert!(
        token_req.headers.get("authorization").map(|v| v.starts_with("Basic ")).unwrap_or(false),
        "token request must carry HTTP Basic client authentication"
    );

    // A protected JSON-RPC POST after auth must carry the bearer.
    mock.on_method(
        "tools/list",
        MockResponse::JsonOk(serde_json::json!({
            "tools": [{ "name": "t", "description": "d", "inputSchema": { "type": "object" } }]
        })),
    );
    let tools = client.list_tools().await.expect("authorized list_tools");
    assert_eq!(tools.len(), 1);

    let listed = mock
        .received()
        .into_iter()
        .find(|r| r.method == "tools/list")
        .expect("tools/list reached the server");
    assert_eq!(
        listed.headers.get("authorization").map(String::as_str),
        Some("Bearer mock-access-token"),
        "authorized requests must carry the acquired bearer"
    );

    client.disconnect().await.ok();
}

// ─── Wrong client secret → token endpoint rejects → connect fails ─────────────

#[tokio::test]
async fn connect_fails_when_client_credentials_are_wrong() {
    let mock = MockMcpServer::start().await;
    mock.enable_oauth("mcp-client", "right-secret", "mock-access-token");

    let mut client = HttpMcpClient::new_with_oauth(
        server_config(mock.base_url()),
        oauth("mcp-client", "WRONG-secret"),
    )
    .unwrap();

    let res = client.connect().await;
    assert!(
        res.is_err(),
        "connect must fail when the token endpoint rejects the client credentials"
    );
    let msg = res.unwrap_err().to_string();
    assert!(
        msg.contains("token endpoint") || msg.contains("invalid_client") || msg.contains("401"),
        "error should reflect the failed token exchange, got: {msg}"
    );
}

// ─── Refresh-on-expiry: a stored refresh_token drives a refresh_token grant ──

/// Phase-4 spec: `auth::refresh_token` MUST attempt `grant_type=refresh_token`
/// before falling back to `client_credentials`. Driven directly at the
/// auth-module entry point (`http.rs::acquire_oauth_token` selects this branch
/// when both a cached token AND a cached endpoint exist).
#[tokio::test]
async fn refresh_token_grant_is_used_when_a_refresh_token_is_stored() {
    let mock = MockMcpServer::start().await;
    mock.enable_oauth("mcp-client", "mcp-secret", "mock-access-token");

    let cfg = oauth("mcp-client", "mcp-secret");
    let token_endpoint = format!("{}token", mock.base_url());

    // Stored token with a `refresh_token`: this is the only state needed to
    // exercise the refresh-grant branch without orchestrating a clock advance.
    let stored = ziee::StoredToken {
        access_token: "stale-access".into(),
        refresh_token: Some("rt-from-an-earlier-issuance".into()),
        expires_at: None,
    };

    let http = reqwest::Client::builder()
        .build()
        .expect("reqwest client");

    let fresh = ziee::oauth_refresh_token(&http, &token_endpoint, &cfg, &stored)
        .await
        .expect("refresh_token grant should succeed against the mock");
    assert_eq!(
        fresh.access_token, "mock-access-token",
        "the mock returns a fixed access_token on every successful exchange"
    );

    // The mock saw EXACTLY ONE token request, and it carried the
    // `refresh_token` grant + the stored refresh_token value + HTTP Basic.
    let token_reqs: Vec<_> = mock
        .received()
        .into_iter()
        .filter(|r| r.method == "__token")
        .collect();
    assert_eq!(token_reqs.len(), 1, "exactly one token endpoint call expected");
    let body = token_reqs[0].body.as_str().unwrap_or("");
    assert!(
        body.contains("grant_type=refresh_token"),
        "refresh path MUST use the refresh_token grant; body={body}"
    );
    assert!(
        body.contains("refresh_token=rt-from-an-earlier-issuance"),
        "refresh path MUST include the stored refresh_token; body={body}"
    );
    assert!(
        token_reqs[0]
            .headers
            .get("authorization")
            .map(|v| v.starts_with("Basic "))
            .unwrap_or(false),
        "refresh path MUST carry HTTP Basic client authentication"
    );
}

// ─── No OAuth configured + 401 → surfaces the 401, no token attempt ───────────

#[tokio::test]
async fn unauthenticated_client_surfaces_401_without_token_flow() {
    let mock = MockMcpServer::start().await;
    mock.enable_oauth("mcp-client", "mcp-secret", "mock-access-token");

    // Plain client — no OAuth configured.
    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    let res = client.connect().await;
    assert!(res.is_err(), "without OAuth a 401 must surface as an error");
    assert_eq!(
        mock.count_for("__token"),
        0,
        "a client without OAuth must not attempt a token exchange"
    );
}
