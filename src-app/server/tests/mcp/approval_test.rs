//! MCP approval workflow integration tests

use serde_json::json;

use crate::common::test_helpers::{create_test_user, TestServer};

/// Test creating and getting MCP settings for a conversation
#[tokio::test]
async fn test_create_and_get_mcp_settings() {
    let server = TestServer::start().await;
    let user = create_test_user(&server, true).await;

    // Create a conversation
    let create_resp = server
        .post("/api/chat/conversations")
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "title": "Test MCP Settings",
        }))
        .send()
        .await
        .expect("Failed to create conversation");

    assert_eq!(create_resp.status(), 200);
    let conversation: serde_json::Value = create_resp.json().await.unwrap();
    let conversation_id = conversation["id"].as_str().unwrap();

    // Get MCP settings (should be none initially)
    let get_resp = server
        .get(&format!("/api/chat/conversations/{}/mcp-settings", conversation_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get MCP settings");

    assert_eq!(get_resp.status(), 200);
    let settings: serde_json::Value = get_resp.json().await.unwrap();
    assert!(settings["settings"].is_null());

    // Create MCP settings with auto_approve mode
    let create_settings_resp = server
        .put(&format!("/api/chat/conversations/{}/mcp-settings", conversation_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "approval_mode": "auto_approve",
            "auto_approved_tools": []
        }))
        .send()
        .await
        .expect("Failed to create MCP settings");

    assert_eq!(create_settings_resp.status(), 200);
    let created_settings: serde_json::Value = create_settings_resp.json().await.unwrap();
    assert_eq!(created_settings["approval_mode"], "auto_approve");

    // Get MCP settings again (should exist now)
    let get_resp2 = server
        .get(&format!("/api/chat/conversations/{}/mcp-settings", conversation_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get MCP settings");

    assert_eq!(get_resp2.status(), 200);
    let settings2: serde_json::Value = get_resp2.json().await.unwrap();
    assert!(!settings2["settings"].is_null());
    assert_eq!(settings2["settings"]["approval_mode"], "auto_approve");
}

/// Test updating MCP settings
#[tokio::test]
async fn test_update_mcp_settings() {
    let server = TestServer::start().await;
    let user = create_test_user(&server, true).await;

    // Create a conversation
    let create_resp = server
        .post("/api/chat/conversations")
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "title": "Test Update Settings",
        }))
        .send()
        .await
        .expect("Failed to create conversation");

    assert_eq!(create_resp.status(), 200);
    let conversation: serde_json::Value = create_resp.json().await.unwrap();
    let conversation_id = conversation["id"].as_str().unwrap();

    // Create settings with auto_approve
    let create_resp = server
        .put(&format!("/api/chat/conversations/{}/mcp-settings", conversation_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "approval_mode": "auto_approve",
            "auto_approved_tools": []
        }))
        .send()
        .await
        .expect("Failed to create settings");

    assert_eq!(create_resp.status(), 200);

    // Update to manual_approve with auto-approved tools
    // New format: [{server_id: "uuid", tools: ["tool1", "tool2"]}, ...]
    let update_resp = server
        .put(&format!("/api/chat/conversations/{}/mcp-settings", conversation_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "approval_mode": "manual_approve",
            "auto_approved_tools": [
                {"server_id": "00000000-0000-0000-0000-000000000001", "tools": ["get", "list"]},
                {"server_id": "00000000-0000-0000-0000-000000000002", "tools": ["read_file"]}
            ]
        }))
        .send()
        .await
        .expect("Failed to update settings");

    assert_eq!(update_resp.status(), 200);
    let updated_settings: serde_json::Value = update_resp.json().await.unwrap();
    assert_eq!(updated_settings["approval_mode"], "manual_approve");
    assert_eq!(updated_settings["auto_approved_tools"].as_array().unwrap().len(), 2);

    // Verify structure of first server entry
    let first_server = &updated_settings["auto_approved_tools"][0];
    assert_eq!(first_server["server_id"], "00000000-0000-0000-0000-000000000001");
    assert!(first_server["tools"].as_array().unwrap().contains(&json!("get")));
    assert!(first_server["tools"].as_array().unwrap().contains(&json!("list")));
}

/// Test invalid auto_approved_tools format validation
/// The new format requires structured objects: [{server_id: "uuid", tools: ["tool1"]}]
#[tokio::test]
async fn test_invalid_auto_approved_tools_format() {
    let server = TestServer::start().await;
    let user = create_test_user(&server, true).await;

    // Create a conversation
    let create_resp = server
        .post("/api/chat/conversations")
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "title": "Test Invalid Tool Names",
        }))
        .send()
        .await
        .expect("Failed to create conversation");

    assert_eq!(create_resp.status(), 200);
    let conversation: serde_json::Value = create_resp.json().await.unwrap();
    let conversation_id = conversation["id"].as_str().unwrap();

    // Try to create settings with old string format (should fail)
    let create_resp = server
        .put(&format!("/api/chat/conversations/{}/mcp-settings", conversation_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "approval_mode": "manual_approve",
            "auto_approved_tools": ["server_id__tool_name"]  // Old format is no longer valid
        }))
        .send()
        .await
        .expect("Failed to send request");

    // Should fail because string format is not valid - expects array of objects
    assert_eq!(create_resp.status(), 422);

    // Try with missing required field
    let create_resp2 = server
        .put(&format!("/api/chat/conversations/{}/mcp-settings", conversation_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "approval_mode": "manual_approve",
            "auto_approved_tools": [{"server_id": "00000000-0000-0000-0000-000000000001"}]  // Missing "tools" field
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(create_resp2.status(), 422);

    // Try with invalid UUID
    let create_resp3 = server
        .put(&format!("/api/chat/conversations/{}/mcp-settings", conversation_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "approval_mode": "manual_approve",
            "auto_approved_tools": [{"server_id": "not-a-uuid", "tools": ["get"]}]
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(create_resp3.status(), 422);
}

/// Test approval mode disabled
#[tokio::test]
async fn test_approval_mode_disabled() {
    let server = TestServer::start().await;
    let user = create_test_user(&server, true).await;

    // Create a conversation
    let create_resp = server
        .post("/api/chat/conversations")
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "title": "Test Disabled Mode",
        }))
        .send()
        .await
        .expect("Failed to create conversation");

    assert_eq!(create_resp.status(), 200);
    let conversation: serde_json::Value = create_resp.json().await.unwrap();
    let conversation_id = conversation["id"].as_str().unwrap();

    // Create settings with disabled mode
    let create_resp = server
        .put(&format!("/api/chat/conversations/{}/mcp-settings", conversation_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "approval_mode": "disabled",
            "auto_approved_tools": []
        }))
        .send()
        .await
        .expect("Failed to create settings");

    assert_eq!(create_resp.status(), 200);
    let settings: serde_json::Value = create_resp.json().await.unwrap();
    assert_eq!(settings["approval_mode"], "disabled");
}

/// Test non-existent conversation returns 404
#[tokio::test]
async fn test_settings_for_nonexistent_conversation() {
    let server = TestServer::start().await;
    let user = create_test_user(&server, true).await;
    let fake_conversation_id = "00000000-0000-0000-0000-000000000000";

    // Try to get settings for non-existent conversation
    let get_resp = server
        .get(&format!("/api/chat/conversations/{}/mcp-settings", fake_conversation_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(get_resp.status(), 404);

    // Try to create settings for non-existent conversation
    let create_resp = server
        .put(&format!("/api/chat/conversations/{}/mcp-settings", fake_conversation_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "approval_mode": "auto_approve",
            "auto_approved_tools": []
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(create_resp.status(), 404);
}

/// Test pending approvals - branch level
#[tokio::test]
async fn test_pending_approvals_branch_level() {
    let server = TestServer::start().await;
    let user = create_test_user(&server, true).await;

    // Create a conversation (which creates a default branch)
    let create_resp = server
        .post("/api/chat/conversations")
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "title": "Test Branch Approvals",
        }))
        .send()
        .await
        .expect("Failed to create conversation");

    assert_eq!(create_resp.status(), 200);
    let conversation: serde_json::Value = create_resp.json().await.unwrap();
    let branch_id = conversation["active_branch_id"].as_str().unwrap();

    // Get pending approvals for branch (should be empty)
    let get_resp = server
        .get(&format!(
            "/api/chat/branches/{}/pending-approvals",
            branch_id
        ))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get pending approvals");

    assert_eq!(get_resp.status(), 200);
    let approvals: serde_json::Value = get_resp.json().await.unwrap();
    assert_eq!(approvals["approvals"].as_array().unwrap().len(), 0);
}
