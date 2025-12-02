//! MCP user defaults integration tests
//!
//! Tests the user MCP defaults API:
//! - GET /api/mcp/defaults - Get user's default MCP settings
//! - PUT /api/mcp/defaults - Create/update user's default MCP settings
//! - Permission checks
//! - Default application to new conversations

use serde_json::json;

use crate::common::test_helpers::{self, TestUser};
use crate::common::TestServer;

// ============================================================================
// Helper Functions
// ============================================================================

/// Get user MCP defaults
async fn get_mcp_defaults(server: &TestServer, token: &str) -> reqwest::Response {
    let url = server.api_url("/mcp/defaults");
    reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to get MCP defaults")
}

/// Update user MCP defaults
async fn update_mcp_defaults(
    server: &TestServer,
    token: &str,
    payload: serde_json::Value,
) -> reqwest::Response {
    let url = server.api_url("/mcp/defaults");
    reqwest::Client::new()
        .put(&url)
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .expect("Failed to update MCP defaults")
}

// ============================================================================
// GET /api/mcp/defaults Tests
// ============================================================================

#[tokio::test]
async fn test_get_mcp_defaults_no_defaults_set() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::read"],
    )
    .await;

    let response = get_mcp_defaults(&server, &user.token).await;

    assert_eq!(response.status(), 200, "Should return 200 OK");

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["defaults"].is_null(), "Defaults should be null when not set");
}

#[tokio::test]
async fn test_get_mcp_defaults_with_defaults_set() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::read", "conversations::edit"],
    )
    .await;

    // First, set some defaults
    let payload = json!({
        "approval_mode": "auto_approve",
        "auto_approved_tools": [
            { "server_id": "00000000-0000-0000-0000-000000000001", "tools": ["tool1", "tool2"] }
        ],
        "disabled_servers": [
            { "server_id": "00000000-0000-0000-0000-000000000002", "tools": [] }
        ]
    });
    let update_response = update_mcp_defaults(&server, &user.token, payload).await;
    assert_eq!(update_response.status(), 200, "Should update defaults");

    // Now get them back
    let response = get_mcp_defaults(&server, &user.token).await;
    assert_eq!(response.status(), 200, "Should return 200 OK");

    let body: serde_json::Value = response.json().await.unwrap();
    let defaults = &body["defaults"];

    assert!(!defaults.is_null(), "Defaults should be set");
    assert_eq!(defaults["approval_mode"], "auto_approve");
    assert_eq!(defaults["auto_approved_tools"].as_array().unwrap().len(), 1);
    assert_eq!(defaults["disabled_servers"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_get_mcp_defaults_requires_permission() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[], // No permissions
    )
    .await;

    let response = get_mcp_defaults(&server, &user.token).await;

    assert_eq!(response.status(), 403, "Should return 403 Forbidden without conversations::read");
}

// ============================================================================
// PUT /api/mcp/defaults Tests
// ============================================================================

#[tokio::test]
async fn test_update_mcp_defaults_create_new() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::edit"],
    )
    .await;

    let payload = json!({
        "approval_mode": "manual_approve",
        "auto_approved_tools": [],
        "disabled_servers": []
    });

    let response = update_mcp_defaults(&server, &user.token, payload).await;

    assert_eq!(response.status(), 200, "Should create defaults");

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["approval_mode"], "manual_approve");
    assert!(body["auto_approved_tools"].as_array().unwrap().is_empty());
    assert!(body["disabled_servers"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_update_mcp_defaults_update_existing() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::edit"],
    )
    .await;

    // Create initial defaults
    let payload1 = json!({
        "approval_mode": "manual_approve",
        "auto_approved_tools": [],
        "disabled_servers": []
    });
    let response1 = update_mcp_defaults(&server, &user.token, payload1).await;
    assert_eq!(response1.status(), 200, "Should create defaults");

    // Update them
    let payload2 = json!({
        "approval_mode": "auto_approve",
        "auto_approved_tools": [
            { "server_id": "00000000-0000-0000-0000-000000000001", "tools": ["fetch"] }
        ],
        "disabled_servers": [
            { "server_id": "00000000-0000-0000-0000-000000000002", "tools": [] }
        ]
    });
    let response2 = update_mcp_defaults(&server, &user.token, payload2).await;
    assert_eq!(response2.status(), 200, "Should update defaults");

    let body: serde_json::Value = response2.json().await.unwrap();
    assert_eq!(body["approval_mode"], "auto_approve");
    assert_eq!(body["auto_approved_tools"].as_array().unwrap().len(), 1);
    assert_eq!(body["disabled_servers"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_update_mcp_defaults_all_approval_modes() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::edit"],
    )
    .await;

    // Test disabled mode
    let payload = json!({
        "approval_mode": "disabled",
        "auto_approved_tools": [],
        "disabled_servers": []
    });
    let response = update_mcp_defaults(&server, &user.token, payload).await;
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["approval_mode"], "disabled");

    // Test auto_approve mode
    let payload = json!({
        "approval_mode": "auto_approve",
        "auto_approved_tools": [],
        "disabled_servers": []
    });
    let response = update_mcp_defaults(&server, &user.token, payload).await;
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["approval_mode"], "auto_approve");

    // Test manual_approve mode
    let payload = json!({
        "approval_mode": "manual_approve",
        "auto_approved_tools": [],
        "disabled_servers": []
    });
    let response = update_mcp_defaults(&server, &user.token, payload).await;
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["approval_mode"], "manual_approve");
}

#[tokio::test]
async fn test_update_mcp_defaults_with_auto_approved_tools() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::edit"],
    )
    .await;

    let payload = json!({
        "approval_mode": "manual_approve",
        "auto_approved_tools": [
            { "server_id": "00000000-0000-0000-0000-000000000001", "tools": ["tool1", "tool2"] },
            { "server_id": "00000000-0000-0000-0000-000000000002", "tools": ["tool3"] }
        ],
        "disabled_servers": []
    });

    let response = update_mcp_defaults(&server, &user.token, payload).await;
    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    let auto_approved = body["auto_approved_tools"].as_array().unwrap();
    assert_eq!(auto_approved.len(), 2);

    // Verify first server
    let server1 = &auto_approved[0];
    assert_eq!(server1["server_id"], "00000000-0000-0000-0000-000000000001");
    assert_eq!(server1["tools"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_update_mcp_defaults_with_disabled_servers() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::edit"],
    )
    .await;

    let payload = json!({
        "approval_mode": "manual_approve",
        "auto_approved_tools": [],
        "disabled_servers": [
            { "server_id": "00000000-0000-0000-0000-000000000001", "tools": [] },
            { "server_id": "00000000-0000-0000-0000-000000000002", "tools": ["specific_tool"] }
        ]
    });

    let response = update_mcp_defaults(&server, &user.token, payload).await;
    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    let disabled = body["disabled_servers"].as_array().unwrap();
    assert_eq!(disabled.len(), 2);

    // Verify first server (entire server disabled)
    let server1 = &disabled[0];
    assert_eq!(server1["server_id"], "00000000-0000-0000-0000-000000000001");
    assert!(server1["tools"].as_array().unwrap().is_empty());

    // Verify second server (specific tools disabled)
    let server2 = &disabled[1];
    assert_eq!(server2["server_id"], "00000000-0000-0000-0000-000000000002");
    assert_eq!(server2["tools"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_update_mcp_defaults_requires_permission() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::read"], // Only read, not edit
    )
    .await;

    let payload = json!({
        "approval_mode": "manual_approve",
        "auto_approved_tools": [],
        "disabled_servers": []
    });

    let response = update_mcp_defaults(&server, &user.token, payload).await;

    assert_eq!(response.status(), 403, "Should return 403 without conversations::edit");
}

// ============================================================================
// User Isolation Tests
// ============================================================================

#[tokio::test]
async fn test_mcp_defaults_user_isolation() {
    let server = TestServer::start().await;

    let user1 = test_helpers::create_user_with_permissions(
        &server,
        "user1",
        &["conversations::read", "conversations::edit"],
    )
    .await;

    let user2 = test_helpers::create_user_with_permissions(
        &server,
        "user2",
        &["conversations::read", "conversations::edit"],
    )
    .await;

    // User1 sets defaults
    let payload1 = json!({
        "approval_mode": "auto_approve",
        "auto_approved_tools": [
            { "server_id": "00000000-0000-0000-0000-000000000001", "tools": ["user1_tool"] }
        ],
        "disabled_servers": []
    });
    let response1 = update_mcp_defaults(&server, &user1.token, payload1).await;
    assert_eq!(response1.status(), 200);

    // User2 sets different defaults
    let payload2 = json!({
        "approval_mode": "manual_approve",
        "auto_approved_tools": [],
        "disabled_servers": [
            { "server_id": "00000000-0000-0000-0000-000000000002", "tools": [] }
        ]
    });
    let response2 = update_mcp_defaults(&server, &user2.token, payload2).await;
    assert_eq!(response2.status(), 200);

    // Verify user1's defaults
    let get1 = get_mcp_defaults(&server, &user1.token).await;
    let body1: serde_json::Value = get1.json().await.unwrap();
    assert_eq!(body1["defaults"]["approval_mode"], "auto_approve");
    assert_eq!(body1["defaults"]["auto_approved_tools"].as_array().unwrap().len(), 1);
    assert!(body1["defaults"]["disabled_servers"].as_array().unwrap().is_empty());

    // Verify user2's defaults (should be different)
    let get2 = get_mcp_defaults(&server, &user2.token).await;
    let body2: serde_json::Value = get2.json().await.unwrap();
    assert_eq!(body2["defaults"]["approval_mode"], "manual_approve");
    assert!(body2["defaults"]["auto_approved_tools"].as_array().unwrap().is_empty());
    assert_eq!(body2["defaults"]["disabled_servers"].as_array().unwrap().len(), 1);
}
