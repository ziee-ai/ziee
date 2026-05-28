//! HTTP 429 backoff/retry tests for the MCP HTTP client.
//!
//! The built-in code-sandbox MCP server is reached over loopback through the
//! SAME Axum router as public traffic, so rapid agent tool loops can self-inflict
//! HTTP 429 from the global rate limiter (the transcript that motivated this:
//! "MCP HTTP error 429 Too Many Requests: Too Many Requests! Wait for 0s").
//! The client must honor the server's wait hint and retry rather than surfacing
//! 429 as a hard tool failure that poisons the agent loop — while still giving
//! up (and surfacing the error) after a bounded number of retries.

use super::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};
use ziee::{HttpMcpClient, McpClient, McpServer, TransportType, UsageMode};

fn server_config(url: String) -> McpServer {
    McpServer {
        id: uuid::Uuid::new_v4(),
        user_id: None,
        name: "mock-mcp-ratelimit".to_string(),
        display_name: "Mock MCP (rate-limit fixture)".to_string(),
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

fn init_result() -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "mock", "version": "0.0.1" },
    })
}

fn tools_result() -> serde_json::Value {
    serde_json::json!({
        "tools": [{ "name": "echo", "description": "d", "inputSchema": { "type": "object" } }]
    })
}

/// Mirror our loopback governor's 429: a plain-text body carrying a "Wait for Ns"
/// hint (no `Retry-After` header).
fn rate_limited() -> MockResponse {
    MockResponse::Raw {
        status: 429,
        content_type: "text/plain",
        body: "Too Many Requests! Wait for 0s".to_string(),
    }
}

/// A transient 429 is retried and the call ultimately succeeds — the agent loop
/// is NOT poisoned by an error tool_result.
#[tokio::test]
async fn request_retries_after_429_then_succeeds() {
    let mock = MockMcpServer::start().await;
    mock.on_method("initialize", MockResponse::JsonOk(init_result()));
    // First tools/list attempt is rate-limited; the retry succeeds.
    mock.on_method("tools/list", rate_limited());
    mock.on_method("tools/list", MockResponse::JsonOk(tools_result()));

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let tools = tokio::time::timeout(std::time::Duration::from_secs(10), client.list_tools())
        .await
        .expect("client must not hang on a 429")
        .expect("a transient 429 must be retried, not surfaced as a hard error");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "echo");

    // Exactly two tools/list attempts: the 429 and the successful retry.
    let attempts = mock
        .received()
        .iter()
        .filter(|r| r.method == "tools/list")
        .count();
    assert_eq!(
        attempts, 2,
        "client must retry the rate-limited request exactly once before succeeding"
    );

    client.disconnect().await.ok();
}

/// A persistently-saturated limiter is bounded: after the retry budget the client
/// gives up and surfaces the 429 rather than retrying forever.
#[tokio::test]
async fn request_gives_up_after_bounded_429_retries() {
    let mock = MockMcpServer::start().await;
    mock.on_method("initialize", MockResponse::JsonOk(init_result()));
    // Far more 429s than the retry budget — every attempt is rate-limited.
    for _ in 0..8 {
        mock.on_method("tools/list", rate_limited());
    }

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let err = tokio::time::timeout(std::time::Duration::from_secs(15), client.list_tools())
        .await
        .expect("client must not hang")
        .expect_err("a persistent 429 must eventually surface as an error");
    assert!(
        err.to_string().contains("429"),
        "surfaced error should mention HTTP 429, got: {err}"
    );

    // Bounded: 1 initial attempt + RL_RETRY_MAX (4) retries = 5 attempts, no more.
    let attempts = mock
        .received()
        .iter()
        .filter(|r| r.method == "tools/list")
        .count();
    assert_eq!(
        attempts, 5,
        "client must stop after the bounded 429 retry budget (1 + RL_RETRY_MAX)"
    );

    client.disconnect().await.ok();
}
