// Stdio transport — production-like conformance tests (audit all-1459b409bc13).
//
// `runtime.rs` exercises the stdio path via `uvx mcp-server-fetch`, but no test
// drives the REAL stdio transport (`modules/mcp/client/stdio.rs`: spawn a child,
// frame JSON-RPC over stdin/stdout, `initialize` → `notifications/initialized`
// → `tools/list` → `tools/call` → shutdown) against the canonical
// `@modelcontextprotocol/server-everything` reference server — the one that
// rejects non-conforming clients. `http_transport_test.rs` does exactly this for
// the HTTP transport; this is its stdio twin.
//
// The ziee server spawns the child itself (transport_type=stdio, command=npx),
// so the test cannot pre-spawn it. Instead we use the EverythingServer fixture
// purely as an `npx`+package AVAILABILITY probe (it self-skips when node/npx is
// absent or the registry is unreachable, matching every other server-everything
// test in this suite); if it can start over HTTP, the same package is cached and
// reachable for the stdio spawn the ziee server performs. We drop the probe and
// drive everything through the real stdio transport.

use crate::common::test_helpers;
use serde_json::json;
use uuid::Uuid;

use super::fixtures::everything_server::EverythingServer;

/// Register a stdio MCP system server pointing at the `server-everything`
/// reference binary, after confirming `npx` + the package are available.
/// Returns `None` (caller should `return`) when the toolchain isn't present.
async fn create_stdio_everything_server(
    server: &crate::common::TestServer,
    user: &test_helpers::TestUser,
    test_name: &str,
) -> Option<Uuid> {
    // Availability probe only — confirms `npx -y @modelcontextprotocol/server-everything`
    // can run on this host. Dropped immediately (kill_on_drop) so it doesn't hold a
    // port; the npx package cache stays warm for the stdio spawn below.
    let probe = EverythingServer::try_start_or_skip(test_name).await?;
    drop(probe);

    let unique_id = Uuid::new_v4().to_string();
    let payload = json!({
        "name": format!("stdio_everything_{}", &unique_id[..8]),
        "display_name": "Stdio Everything Server",
        "description": "server-everything over real stdio transport",
        "enabled": true,
        "transport_type": "stdio",
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-everything", "stdio"],
        "timeout_seconds": 60
    });

    let url = server.api_url("/mcp/system-servers");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to create stdio server");

    assert_eq!(
        response.status(),
        201,
        "Should create stdio server successfully"
    );
    let body: serde_json::Value = response.json().await.unwrap();
    Some(Uuid::parse_str(body["id"].as_str().unwrap()).unwrap())
}

// ============================================================================
// Spawn → initialize → tools/list over real stdio
// ============================================================================

#[tokio::test]
async fn test_stdio_list_server_tools() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    let Some(server_id) =
        create_stdio_everything_server(&server, &admin, "test_stdio_list_server_tools").await
    else {
        return;
    };

    // GET /tools lazily connects: the ziee server spawns the npx child, performs
    // the full stdio handshake (initialize + notifications/initialized), then
    // tools/list — all over the real stdio transport. server-everything refuses
    // tools/list before notifications/initialized, so a 200 here proves the
    // handshake completed correctly.
    let url = server.api_url(&format!("/mcp/servers/{}/tools", server_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    let status = response.status();
    let body_text = response.text().await.expect("Failed to get response text");
    assert_eq!(
        status, 200,
        "Should list tools over stdio (status {}, body: {})",
        status, body_text
    );

    let body: serde_json::Value =
        serde_json::from_str(&body_text).expect("Failed to parse JSON");
    let tools = body["tools"].as_array().expect("Should have tools array");
    assert!(!tools.is_empty(), "server-everything exposes multiple tools");

    let has_echo = tools.iter().any(|t| t["name"].as_str() == Some("echo"));
    assert!(has_echo, "expected `echo` tool from server-everything over stdio");

    let first_tool = tools.first().unwrap();
    assert!(first_tool["name"].is_string(), "Tool should have name");
    assert!(
        first_tool["input_schema"].is_object(),
        "Tool should have input_schema"
    );
}

// ============================================================================
// tools/call round-trip over real stdio
// ============================================================================

#[tokio::test]
async fn test_stdio_call_echo_tool() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    let Some(server_id) =
        create_stdio_everything_server(&server, &admin, "test_stdio_call_echo_tool").await
    else {
        return;
    };

    // Call `echo` — the simplest round-trip — through the real stdio transport.
    let url = server.api_url(&format!("/mcp/servers/{}/tools/echo/call", server_id));
    let payload = json!({ "arguments": { "message": "stdio-route-canary" } });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    let status = response.status();
    let body_text = response.text().await.expect("Failed to get response text");
    assert_eq!(
        status, 200,
        "echo call over stdio should succeed (body: {})",
        body_text
    );

    let body: serde_json::Value =
        serde_json::from_str(&body_text).expect("Failed to parse JSON");
    assert!(body["content"].is_array(), "Should have content array");
    let combined = body["content"].to_string();
    assert!(
        combined.contains("stdio-route-canary"),
        "echo response must include our input round-tripped over stdio; got: {}",
        combined
    );
}

// ============================================================================
// Clean shutdown of a live stdio child
// ============================================================================

#[tokio::test]
async fn test_stdio_disconnect_server() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers::read"],
    )
    .await;

    let Some(server_id) =
        create_stdio_everything_server(&server, &admin, "test_stdio_disconnect_server").await
    else {
        return;
    };

    // Connect (spawns + handshakes the stdio child) by listing tools.
    let list_url = server.api_url(&format!("/mcp/servers/{}/tools", server_id));
    let list_response = reqwest::Client::new()
        .get(&list_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        list_response.status(),
        200,
        "Should connect to stdio server successfully"
    );

    // Disconnect tears down the live child process cleanly (no hang / no panic).
    let disconnect_url = server.api_url(&format!("/mcp/servers/{}/disconnect", server_id));
    let response = reqwest::Client::new()
        .delete(&disconnect_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        response.status(),
        200,
        "Should disconnect stdio server successfully"
    );
}
