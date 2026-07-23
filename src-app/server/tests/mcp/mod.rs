use crate::common::test_helpers;
use serde_json::json;
use uuid::Uuid;

// NOTE: Old approval_test module uses outdated TestServer API
// Comprehensive approval workflow tests now live in this directory
// (mcp_approval_workflow_test) after the chat→mcp bridge extraction.

pub mod mock_sampling_server;
mod run_in_sandbox_test;
// Bridge-side tests moved out of tests/chat/ as part of the
// chat→mcp bridge extraction. They exercise the mcp chat-extension's
// behavior end-to-end via the test server's HTTP surface; they don't
// import bridge code directly (rely on `crate::chat::helpers::*` for
// model fixtures + SSE parsing, same pattern as project tests).
mod approval_claim_test;
mod conversation_settings_default_test;
mod elicitation_mcp_test;
mod mcp_approval_loop_test;
mod mcp_approval_workflow_test;
mod mcp_content_test;
mod mcp_defaults_test;
mod mcp_elicitation_test;
mod mcp_extension_test;
mod mcp_loop_settings_test;
mod mcp_sampling_test;
mod mcp_streaming_workflow_test;
mod resource_link_test;
// ============================================================================
// Runtime Tests
// ============================================================================

mod runtime;
// Stdio transport tests (18 tests)
mod http_transport_test;
// HTTP transport tests (12 tests)
// sse_transport_test removed — SSE transport deprecated in MCP 2025-03-26
pub mod fixtures;
// External MCP server fixtures (everything-server + mock)
mod conformance_test;
// Spec-conformance tests against `server-everything`
mod conformance_errors_test;
// Error-path tests against in-process mock server
mod conformance_streaming_test;
// SSE streaming edge-case tests via mock
mod conformance_extended_test;
// Deeper conformance tests against `server-everything`
mod conformance_elicitation_test;
// Elicitation roundtrip tests via mock SSE server
mod conformance_phase1_test;
// Plan-3 Phase-1: version negotiation, string id, pagination
mod conformance_resumability_test;
// Plan-3 Phase-3 (I1): SSE resume via Last-Event-Id
mod conformance_oauth_test;
// Plan-3 Phase-4 (Cos1): OAuth client_credentials
mod oauth_config_route_test;
// Plan-3 Phase-4: per-server OAuth config endpoints
mod conformance_cancellation_test;
// Plan-3 Phase-2 (C3): client notifications/cancelled
mod elicitation_route_test;
// HTTP route tests for /mcp/elicitation/{id}/respond
mod rate_limit_test;
// Global rate-limiter on/off regression (governor toggle)
mod test_connection_test;
// Connection-test endpoints (user + system test-connection)
mod http_headers_test;
// Custom-header transmission + trim/validation (create/update/test)
mod http_connection_reuse_test;
// Stale keep-alive reuse regression (proxy/tunnel reap → fresh conn per request)
mod sync_emit_test;
// Realtime-sync emission: mcp_server (owner) + system dual-audience
mod permission_revocation_test;
// all-f44bdb26e811: revoking group membership denies subsequent MCP calls
mod tool_call_history_test;
mod group_cascade_test;
// group-cascade system-MCP assignment grant/revoke via real endpoints
mod builtin_test_connection_test;
mod project_mcp_group_combined_test;
// HTTP transport tests (12 tests)
mod stdio_transport_test;
// Plan-3 Phase-2 (C3): client notifications/cancelled
mod tool_call_timeout_test;
// HTTP route tests for /mcp/elicitation/{id}/respond
mod elicitation_jsonrpc_test;
// JSON-RPC route tests for /elicitation/mcp (tools/call fallback)
mod elicitation_cancel_test;

// ============================================================================
// User MCP Server Tests
// ============================================================================

#[tokio::test]
async fn test_create_user_mcp_server() {
    // Uses http transport — the MCP user-policy force-sandboxes user
    // stdio servers, which requires code_sandbox.enabled (off in
    // tests by default). This test exercises the create CRUD path,
    // not stdio specifically, so http is the right fixture.
    let server = crate::common::TestServer::start().await;
    let user =
        test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::create"]).await;

    let payload = json!({
        "name": "my_local_server",
        "display_name": "My Local Server",
        "description": "My personal MCP server",
        "enabled": true,
        "transport_type": "http",
        "url": "http://127.0.0.1:9/mcp",
        "timeout_seconds": 30
    });

    let url = server.api_url("/mcp/servers");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    let status = response.status();
    let body_text = response.text().await.expect("Failed to get response text");

    if status != 201 {
        eprintln!("Error response (status {}): {}", status, body_text);
    }

    assert_eq!(status, 201, "Should create user server");

    let body: serde_json::Value = serde_json::from_str(&body_text).expect("Failed to parse JSON");
    assert_eq!(body["name"], "my_local_server");
    assert_eq!(body["display_name"], "My Local Server");
    assert_eq!(body["transport_type"], "http");
    assert_eq!(body["is_system"], false);
    assert_eq!(body["user_id"], user.user_id);
}

#[tokio::test]
async fn test_create_user_server_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_no_permissions(&server, "user").await;

    let payload = json!({
        "name": "my_server",
        "display_name": "My Server",
        "transport_type": "stdio",
        "command": "node",
        "args": ["server.js"]
    });

    let url = server.api_url("/mcp/servers");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should require permission");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["error_code"], "INSUFFICIENT_PERMISSIONS");
}

#[tokio::test]
async fn test_list_accessible_servers() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["mcp_servers::read", "mcp_servers::create"],
    )
    .await;

    // Create a personal server. Uses http transport — the MCP
    // user-policy force-sandboxes user stdio (requires sandbox);
    // this test just needs a personal server to verify the list
    // includes it, transport choice is incidental.
    let payload = json!({
        "name": "personal_server",
        "display_name": "Personal Server",
        "transport_type": "http",
        "url": "http://127.0.0.1:9/mcp"
    });

    let create_url = server.api_url("/mcp/servers");
    reqwest::Client::new()
        .post(&create_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    // List accessible servers (should include personal + system servers from groups)
    let list_url = server.api_url("/mcp/servers");
    let response = reqwest::Client::new()
        .get(&list_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Should list accessible servers");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let servers = body["servers"]
        .as_array()
        .expect("Should have servers array");

    // Debug: Print what servers we got
    println!("Found {} servers:", servers.len());
    for server in servers.iter() {
        println!("  - {} (name: {})", server["display_name"], server["name"]);
    }

    // Should have at least the personal server + fetch (assigned to default group)
    assert!(
        servers.len() >= 2,
        "Should have personal server and fetch server from default group. Found {} servers",
        servers.len()
    );

    // Verify personal server is in the list
    let has_personal = servers.iter().any(|s| s["name"] == "personal_server");
    assert!(has_personal, "Should include personal server");

    // Verify fetch server from default group is in the list
    let has_fetch = servers.iter().any(|s| s["name"] == "fetch");
    assert!(has_fetch, "Should include fetch server from default group");
}

#[tokio::test]
async fn test_get_user_server() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["mcp_servers::read", "mcp_servers::create"],
    )
    .await;

    // Create a server
    let payload = json!({
        "name": "test_server",
        "display_name": "Test Server",
        "transport_type": "http",
        "url": "http://localhost:3000",
        "headers": {"Authorization": "Bearer token"}
    });

    let create_url = server.api_url("/mcp/servers");
    let create_response = reqwest::Client::new()
        .post(&create_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    let created: serde_json::Value = create_response.json().await.expect("Failed to parse JSON");
    let server_id = created["id"].as_str().expect("Should have server ID");

    // Get the server
    let get_url = server.api_url(&format!("/mcp/servers/{}", server_id));
    let response = reqwest::Client::new()
        .get(&get_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Should get server");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["id"], server_id);
    assert_eq!(body["name"], "test_server");
    assert_eq!(body["transport_type"], "http");
}

#[tokio::test]
async fn test_update_user_server() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "mcp_servers::read",
            "mcp_servers::create",
            "mcp_servers::edit",
        ],
    )
    .await;

    // Create a server (http — user-policy force-sandboxes user
    // stdio; this test is about update CRUD, not transport).
    let payload = json!({
        "name": "original_server",
        "display_name": "Original Server",
        "transport_type": "http",
        "url": "http://127.0.0.1:9/mcp"
    });

    let create_url = server.api_url("/mcp/servers");
    let create_response = reqwest::Client::new()
        .post(&create_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    let created: serde_json::Value = create_response.json().await.expect("Failed to parse JSON");
    let server_id = created["id"].as_str().expect("Should have server ID");

    // Update the server
    let update_payload = json!({
        "display_name": "Updated Server",
        "description": "Updated description",
        "enabled": false,
        "url": "http://127.0.0.1:9/mcp-updated"
    });

    let update_url = server.api_url(&format!("/mcp/servers/{}", server_id));
    let response = reqwest::Client::new()
        .put(&update_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&update_payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Should update server");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["display_name"], "Updated Server");
    assert_eq!(body["description"], "Updated description");
    assert_eq!(body["enabled"], false);
}

#[tokio::test]
async fn test_delete_user_server() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "mcp_servers::read",
            "mcp_servers::create",
            "mcp_servers::delete",
        ],
    )
    .await;

    // Create a server (http — user-policy force-sandboxes stdio).
    let payload = json!({
        "name": "temp_server",
        "display_name": "Temporary Server",
        "transport_type": "http",
        "url": "http://127.0.0.1:9/mcp"
    });

    let create_url = server.api_url("/mcp/servers");
    let create_response = reqwest::Client::new()
        .post(&create_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    let created: serde_json::Value = create_response.json().await.expect("Failed to parse JSON");
    let server_id = created["id"].as_str().expect("Should have server ID");

    // Delete the server
    let delete_url = server.api_url(&format!("/mcp/servers/{}", server_id));
    let response = reqwest::Client::new()
        .delete(&delete_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 204, "Should delete server");

    // Verify it's deleted
    let get_url = server.api_url(&format!("/mcp/servers/{}", server_id));
    let get_response = reqwest::Client::new()
        .get(&get_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(get_response.status(), 404, "Server should be deleted");
}

#[tokio::test]
async fn test_user_cannot_access_other_user_server() {
    let server = crate::common::TestServer::start().await;
    let user1 = test_helpers::create_user_with_permissions(
        &server,
        "user1",
        &["mcp_servers::read", "mcp_servers::create"],
    )
    .await;
    let user2 =
        test_helpers::create_user_with_permissions(&server, "user2", &["mcp_servers::read"]).await;

    // User1 creates a server (http — user-policy force-sandboxes stdio).
    let payload = json!({
        "name": "user1_server",
        "display_name": "User1 Server",
        "transport_type": "http",
        "url": "http://127.0.0.1:9/mcp"
    });

    let create_url = server.api_url("/mcp/servers");
    let create_response = reqwest::Client::new()
        .post(&create_url)
        .header("Authorization", format!("Bearer {}", user1.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    let created: serde_json::Value = create_response.json().await.expect("Failed to parse JSON");
    let server_id = created["id"].as_str().expect("Should have server ID");

    // User2 tries to get User1's server
    let get_url = server.api_url(&format!("/mcp/servers/{}", server_id));
    let response = reqwest::Client::new()
        .get(&get_url)
        .header("Authorization", format!("Bearer {}", user2.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        404,
        "User should not access other user's server"
    );
}

// ============================================================================
// Admin System Server Tests
// ============================================================================

#[tokio::test]
async fn test_create_system_server() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;

    let payload = json!({
        "name": "system_server",
        "display_name": "System Server",
        "description": "System-wide MCP server",
        "enabled": true,
        "transport_type": "stdio",
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-filesystem"],
        "environment_variables": {},
        "timeout_seconds": 60
    });

    let url = server.api_url("/mcp/system-servers");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201, "Should create system server");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["name"], "system_server");
    assert_eq!(body["is_system"], true);
    assert!(
        body["user_id"].is_null(),
        "System server should not have user_id"
    );
}

#[tokio::test]
async fn test_create_system_server_requires_admin_permission() {
    let server = crate::common::TestServer::start().await;
    let user =
        test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::create"]).await;

    let payload = json!({
        "name": "system_server",
        "display_name": "System Server",
        "transport_type": "stdio",
        "command": "node",
        "args": ["server.js"]
    });

    let url = server.api_url("/mcp/system-servers");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should require admin permission");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["error_code"], "INSUFFICIENT_PERMISSIONS");
}

#[tokio::test]
async fn test_list_system_servers() {
    let server = crate::common::TestServer::start().await;
    let admin =
        test_helpers::create_user_with_permissions(&server, "admin", &["mcp_servers_admin::read"])
            .await;

    let url = server.api_url("/mcp/system-servers");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Should list system servers");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let servers = body["servers"]
        .as_array()
        .expect("Should have servers array");

    // Migration 157 deleted the three unused seeded system servers
    // (filesystem / browser / git), leaving `fetch` as the seeded, admin-
    // configurable one. The zero-config `*.ziee.internal` built-ins are
    // excluded from this listing by design.
    assert!(
        !servers.is_empty(),
        "Should have at least the seeded `fetch` system server"
    );
    assert!(
        servers.iter().any(|s| s["name"] == "fetch"),
        "The seeded `fetch` system server should be listed"
    );
    for removed in ["filesystem", "browser", "git"] {
        assert!(
            !servers.iter().any(|s| s["name"] == removed),
            "`{removed}` was removed by migration 157 and must not be listed"
        );
    }

    // Verify all are system servers
    for server in servers {
        assert_eq!(server["is_system"], true, "All should be system servers");
        assert!(
            server["user_id"].is_null(),
            "System servers should not have user_id"
        );
    }
}

#[tokio::test]
async fn test_get_system_server() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::read", "mcp_servers_admin::create"],
    )
    .await;

    // Create a system server
    let payload = json!({
        "name": "test_system_server",
        "display_name": "Test System Server",
        "transport_type": "sse",
        "url": "http://localhost:3000/sse",
        "headers": {"Authorization": "Bearer token"}
    });

    let create_url = server.api_url("/mcp/system-servers");
    let create_response = reqwest::Client::new()
        .post(&create_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    let created: serde_json::Value = create_response.json().await.expect("Failed to parse JSON");
    let server_id = created["id"].as_str().expect("Should have server ID");

    // Get the system server
    let get_url = server.api_url(&format!("/mcp/system-servers/{}", server_id));
    let response = reqwest::Client::new()
        .get(&get_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Should get system server");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["id"], server_id);
    assert_eq!(body["name"], "test_system_server");
    assert_eq!(body["transport_type"], "sse");
    assert_eq!(body["is_system"], true);
}

#[tokio::test]
async fn test_update_system_server() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "mcp_servers_admin::read",
            "mcp_servers_admin::create",
            "mcp_servers_admin::edit",
        ],
    )
    .await;

    // Create a system server
    let payload = json!({
        "name": "original_system_server",
        "display_name": "Original System Server",
        "transport_type": "stdio",
        "command": "node",
        "args": ["original.js"]
    });

    let create_url = server.api_url("/mcp/system-servers");
    let create_response = reqwest::Client::new()
        .post(&create_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    let created: serde_json::Value = create_response.json().await.expect("Failed to parse JSON");
    let server_id = created["id"].as_str().expect("Should have server ID");

    // Update the system server
    let update_payload = json!({
        "display_name": "Updated System Server",
        "description": "Updated system description",
        "enabled": false,
        "transport_type": "stdio",
        "command": "node",
        "args": ["updated.js"]
    });

    let update_url = server.api_url(&format!("/mcp/system-servers/{}", server_id));
    let response = reqwest::Client::new()
        .put(&update_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&update_payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Should update system server");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["display_name"], "Updated System Server");
    assert_eq!(body["description"], "Updated system description");
    assert_eq!(body["enabled"], false);
}

#[tokio::test]
async fn test_delete_system_server() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "mcp_servers_admin::read",
            "mcp_servers_admin::create",
            "mcp_servers_admin::delete",
        ],
    )
    .await;

    // Create a system server
    let payload = json!({
        "name": "temp_system_server",
        "display_name": "Temporary System Server",
        "transport_type": "stdio",
        "command": "node",
        "args": ["temp.js"]
    });

    let create_url = server.api_url("/mcp/system-servers");
    let create_response = reqwest::Client::new()
        .post(&create_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    let created: serde_json::Value = create_response.json().await.expect("Failed to parse JSON");
    let server_id = created["id"].as_str().expect("Should have server ID");

    // Delete the system server
    let delete_url = server.api_url(&format!("/mcp/system-servers/{}", server_id));
    let response = reqwest::Client::new()
        .delete(&delete_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 204, "Should delete system server");

    // Verify it's deleted
    let get_url = server.api_url(&format!("/mcp/system-servers/{}", server_id));
    let get_response = reqwest::Client::new()
        .get(&get_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        get_response.status(),
        404,
        "System server should be deleted"
    );
}

// ----------------------------------------------------------------------------
// Built-in update-guard regression (r2-fresh-sweep-01).
//
// Commit 52a5953b narrowed the `update_system_mcp_server` "built-in is
// immutable" guard (repository.rs) so it gates ONLY the two zero-config
// privileged built-ins — `files_mcp_server_id()` / `memory_mcp_server_id()`.
// The admin-CONFIGURABLE built-ins (`fetch` from migrations 7+25 — its
// siblings filesystem/browser/git were removed by migration 157 — plus
// code_sandbox) must stay editable. This pins BOTH sides.
//
// NOTE: the deterministic ids are `Uuid::new_v5(NAMESPACE_URL, b"<name>")`,
// matching `files_mcp::files_mcp_server_id()` / `memory_mcp::memory_mcp_server_id()`.
// Those server-crate fns are NOT re-exported through `ziee::` (their module
// tree is private), so we recompute the v5 UUID inline rather than import it.
// ----------------------------------------------------------------------------

fn files_mcp_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"files.ziee.internal")
}

fn memory_mcp_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"memory.ziee.internal")
}

/// The zero-config built-in rows (files/memory) are upserted by an async
/// `tokio::spawn` at module init, so a GET right after `TestServer::start`
/// can race the upsert. Poll the per-id GET in a short bounded loop until
/// the row materializes (status 200) before exercising the PUT guard.
async fn wait_for_system_server(
    server: &crate::common::TestServer,
    token: &str,
    id: Uuid,
) -> serde_json::Value {
    let get_url = server.api_url(&format!("/mcp/system-servers/{}", id));
    for _ in 0..40 {
        let response = reqwest::Client::new()
            .get(&get_url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .expect("Request failed");
        if response.status() == 200 {
            return response.json().await.expect("Failed to parse JSON");
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    panic!("built-in system server {} never materialized after boot upsert", id);
}

#[tokio::test]
async fn test_update_configurable_builtin_is_editable() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::read", "mcp_servers_admin::edit"],
    )
    .await;

    // Find the seeded `fetch` row (migration 7 — a random UUID, so it is
    // NOT in the zero-config guard set and must remain editable).
    let list_url = server.api_url("/mcp/system-servers");
    let list_body: serde_json::Value = reqwest::Client::new()
        .get(&list_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed")
        .json()
        .await
        .expect("Failed to parse JSON");
    let servers = list_body["servers"]
        .as_array()
        .expect("Should have servers array");
    let fetch = servers
        .iter()
        .find(|s| s["name"] == "fetch")
        .expect("Should have seeded fetch server");
    let fetch_id = fetch["id"].as_str().expect("Should have server ID");

    // PUT a benign config change (mirror `test_update_system_server` shape).
    let update_payload = json!({
        "display_name": "Web Fetch",
        "description": "Edited fetch description",
        "transport_type": "stdio",
        "command": "uvx",
        "args": ["mcp-server-fetch"]
    });

    let update_url = server.api_url(&format!("/mcp/system-servers/{}", fetch_id));
    let response = reqwest::Client::new()
        .put(&update_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&update_payload)
        .send()
        .await
        .expect("Request failed");

    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(
        status, 200,
        "Admin-configurable built-in (fetch) must stay editable; got: {body}"
    );
    assert_eq!(body["description"], "Edited fetch description");

    // Confirm persistence via a follow-up GET.
    let get_url = server.api_url(&format!("/mcp/system-servers/{}", fetch_id));
    let get_body: serde_json::Value = reqwest::Client::new()
        .get(&get_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed")
        .json()
        .await
        .expect("Failed to parse JSON");
    assert_eq!(
        get_body["description"], "Edited fetch description",
        "Edit must persist for the admin-configurable built-in"
    );
}

#[tokio::test]
async fn test_update_zero_config_builtin_is_immutable() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::read", "mcp_servers_admin::edit"],
    )
    .await;

    // Trivial, benign update payload. The zero-config guard fires BEFORE
    // transport validation, so the body never has to be valid for the row's
    // transport — only that the guard rejects it.
    let update_payload = json!({
        "display_name": "Hijacked",
        "description": "should not apply"
    });

    // files_mcp built-in — must reject with BUILT_IN_SERVER / 400.
    let files_id = files_mcp_server_id();
    wait_for_system_server(&server, &admin.token, files_id).await;
    let update_url = server.api_url(&format!("/mcp/system-servers/{}", files_id));
    let response = reqwest::Client::new()
        .put(&update_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&update_payload)
        .send()
        .await
        .expect("Request failed");
    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(
        status, 400,
        "files built-in must be immutable; got: {body}"
    );
    assert_eq!(
        body["error_code"], "BUILT_IN_SERVER",
        "files built-in rejection must carry BUILT_IN_SERVER; got: {body}"
    );

    // memory_mcp built-in — same immutability. The memory built-in row is only
    // registered by the server when memory is ENABLED (which requires an
    // embedding model — out of scope here), so materialize the row directly
    // (mirroring upsert_builtin_server) to exercise the immutability guard,
    // which is independent of memory being functionally enabled.
    let memory_id = memory_mcp_server_id();
    {
        let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
        sqlx::query(
            r#"INSERT INTO mcp_servers (
                   id, user_id, name, display_name, description,
                   enabled, is_system, is_built_in,
                   transport_type, url, headers,
                   timeout_seconds, supports_sampling, usage_mode, max_concurrent_sessions,
                   created_at, updated_at
               ) VALUES (
                   $1, NULL, 'memory', 'Memory',
                   'Built-in memory tools (remember / recall / forget)',
                   true, true, true,
                   'http', 'http://127.0.0.1:0/api/memories/mcp', '{}'::jsonb,
                   30, false, 'auto', 4,
                   NOW(), NOW()
               )
               ON CONFLICT (id) DO NOTHING"#,
        )
        .bind(memory_id)
        .execute(&pool)
        .await
        .expect("materialize memory built-in row");
    }
    wait_for_system_server(&server, &admin.token, memory_id).await;
    let update_url = server.api_url(&format!("/mcp/system-servers/{}", memory_id));
    let response = reqwest::Client::new()
        .put(&update_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&update_payload)
        .send()
        .await
        .expect("Request failed");
    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(
        status, 400,
        "memory built-in must be immutable; got: {body}"
    );
    assert_eq!(
        body["error_code"], "BUILT_IN_SERVER",
        "memory built-in rejection must carry BUILT_IN_SERVER; got: {body}"
    );
}

// ============================================================================
// Group Assignment Tests
// ============================================================================

#[tokio::test]
async fn test_assign_server_to_groups() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::read", "mcp_servers_admin::edit"],
    )
    .await;

    // Get group IDs
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let default_group = sqlx::query!("SELECT id FROM groups WHERE is_default = true LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("Failed to get default group");
    let default_group_id = default_group.id;

    // An UNASSIGNED system server. This test used to key off the seeded
    // `filesystem` row (removed by migration 157). Retargeting it at the
    // surviving `fetch` row would make it VACUOUS: migration 7 already assigns
    // `fetch` to the default group, so the "assigned" assertion would hold before
    // the POST was ever issued — the test would pass even if the assign endpoint
    // were a no-op. Create a server that is genuinely in no group instead.
    let fetch_server_id = sqlx::query!(
        r#"INSERT INTO mcp_servers (name, display_name, description, transport_type, is_system, enabled, url)
           VALUES ('assign_target', 'Assign Target', 'Group-assignment fixture', 'http', true, true, 'http://127.0.0.1:9/mcp')
           RETURNING id"#
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create the assignment fixture server")
    .id;

    // Precondition: it is in NO group — otherwise the assertion below is vacuous.
    let before = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM user_group_mcp_servers WHERE mcp_server_id = $1"#,
        fetch_server_id
    )
    .fetch_one(&pool)
    .await
    .expect("count groups");
    assert_eq!(before, 0, "the fixture server must start unassigned");

    pool.close().await;

    // Assign the fixture server to the default group
    let payload = json!({
        "group_ids": [default_group_id]
    });

    let url = server.api_url(&format!(
        "/mcp/system-servers/{}/groups",
        fetch_server_id
    ));
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 204, "Should assign server to groups");

    // Get server's assigned groups
    let get_url = server.api_url(&format!(
        "/mcp/system-servers/{}/groups",
        fetch_server_id
    ));
    let get_response = reqwest::Client::new()
        .get(&get_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(get_response.status(), 200, "Should get server groups");

    let body: serde_json::Value = get_response.json().await.expect("Failed to parse JSON");
    let group_ids = body.as_array().expect("Should be array of group IDs");
    assert_eq!(group_ids.len(), 1, "Should have 1 assigned group");
}

#[tokio::test]
async fn test_remove_server_from_group() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::read", "mcp_servers_admin::edit"],
    )
    .await;

    // Get the default group ID and fetch server ID
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let group = sqlx::query!("SELECT id FROM groups WHERE is_default = true LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("Failed to get default group");
    let group_id = group.id;

    let fetch_server =
        sqlx::query!("SELECT id FROM mcp_servers WHERE name = 'fetch' AND is_system = true")
            .fetch_one(&pool)
            .await
            .expect("Failed to get fetch server");
    let fetch_server_id = fetch_server.id;

    pool.close().await;

    // Remove fetch server from group (it was assigned in migration)
    let url = server.api_url(&format!(
        "/mcp/system-servers/{}/groups/{}",
        fetch_server_id, group_id
    ));
    let response = reqwest::Client::new()
        .delete(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 204, "Should remove server from group");

    // Verify it's removed
    let get_url = server.api_url(&format!("/mcp/system-servers/{}/groups", fetch_server_id));
    let get_response = reqwest::Client::new()
        .get(&get_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    let body: serde_json::Value = get_response.json().await.expect("Failed to parse JSON");
    let group_ids = body.as_array().expect("Should be array of group IDs");

    // Should not contain the default group
    let has_group = group_ids
        .iter()
        .any(|id| id.as_str() == Some(&group_id.to_string()));
    assert!(!has_group, "Should not have default group after removal");
}

// ============================================================================
// Group-Centric Assignment Tests (for UI Widgets)
// ============================================================================

#[tokio::test]
async fn test_get_group_system_servers() {
    let server = crate::common::TestServer::start().await;
    let admin =
        test_helpers::create_user_with_permissions(&server, "admin", &["mcp_servers_admin::read"])
            .await;

    // Get default group ID
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let group = sqlx::query!("SELECT id FROM groups WHERE is_default = true LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("Failed to get default group");
    let group_id = group.id;

    pool.close().await;

    // Get system servers for group (should include fetch server from migration)
    let url = server.api_url(&format!("/groups/{}/system-servers", group_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Should get group system servers");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let servers = body["servers"]
        .as_array()
        .expect("Should have servers array");

    // Should have fetch server (assigned in migration)
    let has_fetch = servers.iter().any(|s| s["name"] == "fetch");
    assert!(
        has_fetch,
        "Should include fetch server assigned in migration"
    );
}

#[tokio::test]
async fn test_update_group_system_servers_bulk() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "mcp_servers_admin::read",
            "mcp_servers_admin::create",
            "mcp_servers_admin::edit",
        ],
    )
    .await;

    // Get default group ID and create test system servers
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let group = sqlx::query!("SELECT id FROM groups WHERE is_default = true LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("Failed to get default group");
    let group_id = group.id;

    pool.close().await;

    // Create three test system servers
    let server1 = create_test_system_server(&server, &admin.token, "test_server_1").await;
    let server2 = create_test_system_server(&server, &admin.token, "test_server_2").await;
    let server3 = create_test_system_server(&server, &admin.token, "test_server_3").await;

    let server_id1 = server1["id"].as_str().unwrap();
    let server_id2 = server2["id"].as_str().unwrap();
    let server_id3 = server3["id"].as_str().unwrap();

    // Assign two servers to group
    let payload = json!({
        "server_ids": [server_id1, server_id2]
    });

    let url = server.api_url(&format!("/groups/{}/system-servers", group_id));
    let response = reqwest::Client::new()
        .put(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Should update group system servers");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let servers = body["servers"]
        .as_array()
        .expect("Should have servers array");

    // Should have 2 assigned servers (plus fetch from migration = 3 total)
    assert!(
        servers.len() >= 2,
        "Should have at least 2 assigned servers"
    );

    // Verify correct servers are assigned
    let server_names: Vec<String> = servers
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_string())
        .collect();
    assert!(server_names.contains(&"test_server_1".to_string()));
    assert!(server_names.contains(&"test_server_2".to_string()));

    // Update assignment - remove server1, keep server2, add server3
    let payload = json!({
        "server_ids": [server_id2, server_id3]
    });

    let response = reqwest::Client::new()
        .put(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Should update group system servers");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let servers = body["servers"]
        .as_array()
        .expect("Should have servers array");

    // Verify correct servers are now assigned
    let server_names: Vec<String> = servers
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_string())
        .collect();
    assert!(server_names.contains(&"test_server_2".to_string()));
    assert!(server_names.contains(&"test_server_3".to_string()));
    assert!(!server_names.contains(&"test_server_1".to_string()));
}

#[tokio::test]
async fn test_update_group_system_servers_empty_list() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "mcp_servers_admin::read",
            "mcp_servers_admin::create",
            "mcp_servers_admin::edit",
        ],
    )
    .await;

    // Get default group ID
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let group = sqlx::query!("SELECT id FROM groups WHERE is_default = true LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("Failed to get default group");
    let group_id = group.id;

    pool.close().await;

    // Create and assign a server
    let test_server = create_test_system_server(&server, &admin.token, "temp_server").await;
    let server_id = test_server["id"].as_str().unwrap();

    let payload = json!({
        "server_ids": [server_id]
    });

    let url = server.api_url(&format!("/groups/{}/system-servers", group_id));
    reqwest::Client::new()
        .put(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    // Clear all assignments with empty list (except the ones from migration)
    let payload = json!({
        "server_ids": []
    });

    let response = reqwest::Client::new()
        .put(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Should clear group assignments");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let servers = body["servers"]
        .as_array()
        .expect("Should have servers array");

    // Should not contain the test server
    let has_temp = servers.iter().any(|s| s["name"] == "temp_server");
    assert!(!has_temp, "Should not have temp_server after clearing");
}

#[tokio::test]
async fn test_update_group_system_servers_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let user =
        test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers_admin::read"])
            .await;

    let group_id = Uuid::new_v4();
    let payload = json!({
        "server_ids": []
    });

    let url = server.api_url(&format!("/groups/{}/system-servers", group_id));
    let response = reqwest::Client::new()
        .put(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should require edit permission");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["error_code"], "INSUFFICIENT_PERMISSIONS");
}

#[tokio::test]
async fn test_get_group_system_servers_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_no_permissions(&server, "user").await;

    let group_id = Uuid::new_v4();
    let url = server.api_url(&format!("/groups/{}/system-servers", group_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should require read permission");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["error_code"], "INSUFFICIENT_PERMISSIONS");
}

// ============================================================================
// Helper Functions
// ============================================================================

async fn create_test_system_server(
    server: &crate::common::TestServer,
    token: &str,
    name: &str,
) -> serde_json::Value {
    let payload = json!({
        "name": name,
        "display_name": format!("Test Server {}", name),
        "transport_type": "stdio",
        "command": "node",
        "args": ["server.js"],
        "enabled": true
    });

    let url = server.api_url("/mcp/system-servers");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201, "Should create test system server");
    response.json().await.expect("Failed to parse JSON")
}

// ============================================================================
// Validation Tests
// ============================================================================

#[tokio::test]
async fn test_stdio_transport_requires_command() {
    // Uses the SYSTEM create endpoint — user-mode stdio is gated by
    // the MCP user-policy (force-sandboxed; rejects when sandbox is
    // disabled in tests), which would 422 before this validation
    // runs. The 400-on-missing-command invariant is the same for
    // both endpoints; system path is the cleaner one to exercise.
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;

    // Try to create stdio system server without command
    let payload = json!({
        "name": "invalid_stdio",
        "display_name": "Invalid Stdio",
        "transport_type": "stdio",
        "args": ["server.js"]
        // Missing command
    });

    let url = server.api_url("/mcp/system-servers");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        400,
        "Should reject stdio without command"
    );
}

#[tokio::test]
async fn test_http_transport_requires_url() {
    let server = crate::common::TestServer::start().await;
    let user =
        test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::create"]).await;

    // Try to create http server without url
    let payload = json!({
        "name": "invalid_http",
        "display_name": "Invalid HTTP",
        "transport_type": "http",
        "headers": {"Authorization": "Bearer token"}
        // Missing url
    });

    let url = server.api_url("/mcp/servers");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 400, "Should reject http without url");
}

#[tokio::test]
async fn test_duplicate_server_name_allowed() {
    let server = crate::common::TestServer::start().await;
    let user =
        test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::create"]).await;

    // Uses http transport — user-policy force-sandboxes user stdio
    // (sandbox is off in tests). Duplicate-name semantics are the
    // same for both transports.
    let payload = json!({
        "name": "duplicate_server",
        "display_name": "Duplicate Server",
        "transport_type": "http",
        "url": "http://127.0.0.1:9/mcp"
    });

    let url = server.api_url("/mcp/servers");

    // Create first server
    let response1 = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response1.status(), 201, "First server should be created");

    // Create second server with same name (should now be allowed)
    let response2 = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response2.status(),
        201,
        "Second server with duplicate name should be allowed"
    );
}

// mcp_tool_calls recording + list/scope + retention setting

// ============================================================================
// Sampling Field CRUD Tests
// ============================================================================

#[tokio::test]
async fn test_create_mcp_server_with_sampling_fields() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;

    let payload = json!({
        "name": "sampling-http-server",
        "display_name": "Sampling HTTP Server",
        "description": "Server with sampling enabled",
        "enabled": true,
        "transport_type": "http",
        "url": "https://example.com/mcp",
        "supports_sampling": true,
        "usage_mode": "always",
        "max_concurrent_sessions": 3
    });

    let url = server.api_url("/mcp/system-servers");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");

    assert_eq!(status, 201, "Should create system server with sampling fields");
    assert_eq!(body["supports_sampling"], true, "supports_sampling should be true");
    assert_eq!(body["usage_mode"], "always", "usage_mode should be always");
    assert_eq!(body["max_concurrent_sessions"], 3, "max_concurrent_sessions should be 3");
}

#[tokio::test]
async fn test_sampling_field_defaults() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;

    // Create without any sampling fields
    let payload = json!({
        "name": "default-fields-server",
        "display_name": "Default Fields Server",
        "transport_type": "stdio",
        "command": "node",
        "args": ["server.js"]
    });

    let url = server.api_url("/mcp/system-servers");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201, "Should create server");
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");

    assert_eq!(body["supports_sampling"], false, "Default supports_sampling should be false");
    assert_eq!(body["usage_mode"], "auto", "Default usage_mode should be auto");
    assert!(
        body["max_concurrent_sessions"].is_null(),
        "Default max_concurrent_sessions should be null"
    );
}

#[tokio::test]
async fn test_update_mcp_server_sampling_fields() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &[
            "mcp_servers_admin::create",
            "mcp_servers_admin::read",
            "mcp_servers_admin::edit",
        ],
    )
    .await;

    // Create server without sampling
    let create_payload = json!({
        "name": "update-sampling-test",
        "display_name": "Update Sampling Test",
        "transport_type": "http",
        "url": "https://example.com/mcp"
    });

    let create_url = server.api_url("/mcp/system-servers");
    let create_response = reqwest::Client::new()
        .post(&create_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&create_payload)
        .send()
        .await
        .expect("Request failed");

    let created: serde_json::Value = create_response.json().await.expect("Failed to parse JSON");
    let server_id = created["id"].as_str().expect("Should have server ID");

    // Update sampling fields
    let update_payload = json!({
        "display_name": "Update Sampling Test",
        "transport_type": "http",
        "url": "https://example.com/mcp",
        "supports_sampling": true,
        "usage_mode": "always",
        "max_concurrent_sessions": 5
    });

    let update_url = server.api_url(&format!("/mcp/system-servers/{}", server_id));
    let response = reqwest::Client::new()
        .put(&update_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&update_payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Should update server");
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["supports_sampling"], true);
    assert_eq!(body["usage_mode"], "always");
    assert_eq!(body["max_concurrent_sessions"], 5);

    // Confirm via GET
    let get_url = server.api_url(&format!("/mcp/system-servers/{}", server_id));
    let get_body: serde_json::Value = reqwest::Client::new()
        .get(&get_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed")
        .json()
        .await
        .expect("Failed to parse JSON");

    assert_eq!(get_body["supports_sampling"], true);
    assert_eq!(get_body["usage_mode"], "always");
    assert_eq!(get_body["max_concurrent_sessions"], 5);
}

// ============================================================================
// SSE Sampling Roundtrip Tests (no DB, use HttpMcpClient directly)
// ============================================================================

fn make_sampling_server_config(url: String, timeout_seconds: i32) -> ziee::McpServer {
    ziee::McpServer {
        id: uuid::Uuid::new_v4(),
        user_id: None,
        name: "test-mock".to_string(),
        display_name: "Test Mock".to_string(),
        description: None,
        enabled: true,
        is_system: false,
        transport_type: ziee::TransportType::Http,
        command: None,
        args: serde_json::json!([]),
        environment_variables: serde_json::json!({}),
        environment_variables_entries: vec![],
        url: Some(url),
        headers: serde_json::json!({}),
        headers_entries: vec![],
        timeout_seconds,
        supports_sampling: true,
        usage_mode: ziee::UsageMode::Auto,
        max_concurrent_sessions: None,
        is_built_in: false,
        run_in_sandbox: false,
        sandbox_flavor: "full".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        last_health_check_at: None,
        last_health_check_status: "untested".to_string(),
        last_health_check_reason: None,
    }
}

/// Verifies SSE streaming mechanics end-to-end with `HttpMcpClient`:
///   1. Opens SSE stream to the mock MCP server
///   2. Reads `sampling/createMessage` events (2 rounds)
///   3. Calls handler and POSTs results back
///   4. Reads the final tool result
///
/// No DB, no real LLM — `InstantHandler` returns "Mock answer" immediately.
/// If this test hangs, the bug is in `call_tool_with_sampling`'s SSE loop.
#[tokio::test]
async fn test_call_tool_with_sampling_sse_roundtrip() {
    use async_trait::async_trait;
    use std::sync::Arc;
    use std::time::Duration;
    use ziee::{
        AppError, HttpMcpClient, McpClient, SamplingContent, SamplingCreateMessageRequest,
        SamplingCreateMessageResult, SamplingHandler,
    };

    struct InstantHandler;

    #[async_trait]
    impl SamplingHandler for InstantHandler {
        async fn create_message(
            &self,
            _req: SamplingCreateMessageRequest,
        ) -> Result<SamplingCreateMessageResult, AppError> {
            Ok(SamplingCreateMessageResult {
                role: "assistant".to_string(),
                content: SamplingContent::Text {
                    text: "Mock answer".to_string(),
                },
                model: "mock-model".to_string(),
                stop_reason: Some("end_turn".to_string()),
            })
        }
    }

    let mock = mock_sampling_server::MockSamplingServer::start().await;
    let handler = Arc::new(InstantHandler);
    let server_config = make_sampling_server_config(mock.url(), 30);
    let mut client =
        HttpMcpClient::new_with_sampling(server_config, handler).expect("create client");
    client.connect().await.expect("connect");

    let result = tokio::time::timeout(
        Duration::from_secs(15),
        client.call_tool(
            "research",
            serde_json::json!({"query": "What is the capital of Germany?"}),
            None, // message_id (sampling test doesn't need it)
            None, // sse_tx (no Axum SSE forwarding)
            None, // elicit_notify_tx (no elicitation notifications)
        ),
    )
    .await
    .expect("TIMEOUT: SSE byte_stream.next() never returned — bug in call_tool_with_sampling")
    .expect("Tool call failed");

    assert!(!result.content.is_empty(), "Should have tool result content");
    eprintln!("SSE roundtrip test passed: {:?}", result.content);
}

/// Group-cascade access: assigning a SYSTEM MCP server to a group must cascade
/// to that group's members — a user in the group then sees the server in their
/// own `GET /mcp/servers` (which joins user_group_mcp_servers). The existing
/// assignment tests only assert the admin-side group list, never that a member
/// actually gains access.
#[tokio::test]
async fn test_system_server_assignment_cascades_to_group_members() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "cascade_admin",
        &["mcp_servers_admin::read", "mcp_servers_admin::edit"],
    )
    .await;
    // A member (auto-added to the default group) who can list their servers.
    let member = test_helpers::create_user_with_permissions(
        &server,
        "cascade_member",
        &["mcp_servers::read"],
    )
    .await;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&server.database_url)
        .await
        .unwrap();
    let default_group_id = sqlx::query!("SELECT id FROM groups WHERE is_default = true LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap()
        .id;
    // An UNASSIGNED system server to cascade. Created here rather than keyed off
    // a seeded row: migration 157 removed the unused seeded servers, and the one
    // that survives (`fetch`) ships already assigned to the default group — which
    // would make the "before assignment" half of this test vacuous.
    let cascade_server_id = sqlx::query!(
        r#"INSERT INTO mcp_servers (name, display_name, description, transport_type, is_system, enabled, url)
           VALUES ('cascade_target', 'Cascade Target', 'Cascade fixture', 'http', true, true, 'http://127.0.0.1:9/mcp')
           RETURNING id"#
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .id;
    pool.close().await;

    let member_server_names = || async {
        let body: serde_json::Value = reqwest::Client::new()
            .get(server.api_url("/mcp/servers"))
            .header("Authorization", format!("Bearer {}", member.token))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        body["servers"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|s| s["name"].as_str().map(String::from))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };

    // Before assignment: the member's default group has no grant for it.
    let before = member_server_names().await;
    assert!(
        !before.contains(&"cascade_target".to_string()),
        "cascade_target must NOT be visible before the group assignment: {before:?}"
    );

    // Admin assigns the system server to the default group.
    let assign = reqwest::Client::new()
        .post(server.api_url(&format!("/mcp/system-servers/{cascade_server_id}/groups")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "group_ids": [default_group_id] }))
        .send()
        .await
        .unwrap();
    assert_eq!(assign.status(), 204, "assign system server to group");

    // After assignment: the member now sees the cascaded system server.
    let after = member_server_names().await;
    assert!(
        after.contains(&"cascade_target".to_string()),
        "cascade_target must cascade to the group member after assignment: {after:?}"
    );
}

