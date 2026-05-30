//! Tier 2 + 3 tests for the `run_in_sandbox` flag on
//! `mcp_servers.run_in_sandbox`. Tier 2 = repository round-trip
//! through real Postgres; Tier 3 = HTTP handler round-trip through
//! TestServer. The sandbox is NOT enabled in either tier (no rootfs);
//! we're only verifying the column + API surface.

use crate::common::test_helpers;
use crate::common::TestServer;
use serde_json::json;

// ---------------------------------------------------------------------
// Tier 3 — HTTP handler integration
// ---------------------------------------------------------------------

#[tokio::test]
async fn create_system_server_persists_run_in_sandbox_true() {
    let server = TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers_admin::read"],
    )
    .await;

    let payload = json!({
        "name": "sandboxed_server",
        "display_name": "Sandboxed Server",
        "description": "Stdio + sandbox on",
        "enabled": true,
        "transport_type": "stdio",
        "command": "python3",
        "args": ["-m", "mcp_test_server"],
        "environment_variables": {},
        "timeout_seconds": 30,
        "run_in_sandbox": true,
    });

    let url = server.api_url("/mcp/system-servers");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("create system server request");

    assert_eq!(response.status(), 201);
    let body: serde_json::Value = response.json().await.expect("json body");
    assert_eq!(body["name"], "sandboxed_server");
    assert_eq!(
        body["run_in_sandbox"], true,
        "run_in_sandbox should round-trip from the create request"
    );
    assert_eq!(body["is_system"], true);

    // GET reflects the flag.
    let server_id = body["id"].as_str().expect("id");
    let get_url = server.api_url(&format!("/mcp/system-servers/{}", server_id));
    let get_resp = reqwest::Client::new()
        .get(&get_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("get system server");
    assert_eq!(get_resp.status(), 200);
    let get_body: serde_json::Value = get_resp.json().await.unwrap();
    assert_eq!(get_body["run_in_sandbox"], true);
}

#[tokio::test]
async fn create_system_server_defaults_run_in_sandbox_false_when_omitted() {
    let server = TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;

    // No run_in_sandbox key at all.
    let payload = json!({
        "name": "default_server",
        "display_name": "Default",
        "enabled": true,
        "transport_type": "stdio",
        "command": "python3",
        "args": [],
        "environment_variables": {},
        "timeout_seconds": 30,
    });

    let url = server.api_url("/mcp/system-servers");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body["run_in_sandbox"], false,
        "default must be false (column NOT NULL DEFAULT false)"
    );
}

#[tokio::test]
async fn update_system_server_can_toggle_run_in_sandbox() {
    let server = TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers_admin::edit", "mcp_servers_admin::read"],
    )
    .await;

    // Create with false.
    let create_payload = json!({
        "name": "toggle_server",
        "display_name": "Toggle",
        "enabled": true,
        "transport_type": "stdio",
        "command": "python3",
        "args": [],
        "environment_variables": {},
        "timeout_seconds": 30,
        "run_in_sandbox": false,
    });
    let create_url = server.api_url("/mcp/system-servers");
    let create_resp = reqwest::Client::new()
        .post(&create_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&create_payload)
        .send()
        .await
        .unwrap();
    let created: serde_json::Value = create_resp.json().await.unwrap();
    let server_id = created["id"].as_str().unwrap();
    assert_eq!(created["run_in_sandbox"], false);

    // PUT flips to true.
    let update_url = server.api_url(&format!("/mcp/system-servers/{}", server_id));
    let update_resp = reqwest::Client::new()
        .put(&update_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "run_in_sandbox": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(update_resp.status(), 200);
    let updated: serde_json::Value = update_resp.json().await.unwrap();
    assert_eq!(updated["run_in_sandbox"], true);

    // And back to false.
    let update_resp = reqwest::Client::new()
        .put(&update_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "run_in_sandbox": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(update_resp.status(), 200);
    let body: serde_json::Value = update_resp.json().await.unwrap();
    assert_eq!(body["run_in_sandbox"], false);
}

#[tokio::test]
async fn update_system_server_preserves_run_in_sandbox_when_omitted() {
    // PUT without the run_in_sandbox key MUST keep the existing value
    // (COALESCE($n, run_in_sandbox) in the UPDATE). Critical: admins
    // editing the description shouldn't accidentally re-enable or
    // re-disable the sandbox flag.
    let server = TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers_admin::edit"],
    )
    .await;

    let create_url = server.api_url("/mcp/system-servers");
    let created: serde_json::Value = reqwest::Client::new()
        .post(&create_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "preserve_server",
            "display_name": "Preserve",
            "enabled": true,
            "transport_type": "stdio",
            "command": "python3",
            "args": [],
            "environment_variables": {},
            "timeout_seconds": 30,
            "run_in_sandbox": true,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let server_id = created["id"].as_str().unwrap();

    // PUT with ONLY description changed — must preserve run_in_sandbox=true.
    let update_url = server.api_url(&format!("/mcp/system-servers/{}", server_id));
    let updated: serde_json::Value = reqwest::Client::new()
        .put(&update_url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "description": "Updated description only" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["description"], "Updated description only");
    assert_eq!(
        updated["run_in_sandbox"], true,
        "run_in_sandbox must be preserved when not in the PUT body"
    );
}

#[tokio::test]
async fn user_mode_create_silently_ignores_run_in_sandbox_flag() {
    // The user-server INSERT hard-codes run_in_sandbox=false (only
    // admin-owned system servers are sandbox-eligible). If a user
    // sends the flag, the API accepts the request (no 4xx) but the
    // persisted row has run_in_sandbox=false.
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["mcp_servers::create"],
    )
    .await;

    let create_url = server.api_url("/mcp/servers");
    let response = reqwest::Client::new()
        .post(&create_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "user_attempt",
            "display_name": "User Attempt",
            "enabled": true,
            "transport_type": "stdio",
            "command": "python3",
            "args": [],
            "environment_variables": {},
            "timeout_seconds": 30,
            "run_in_sandbox": true,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body["run_in_sandbox"], false,
        "user-mode create must silently force run_in_sandbox=false"
    );
}

#[tokio::test]
async fn list_system_servers_includes_run_in_sandbox() {
    let server = TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers_admin::read"],
    )
    .await;

    // Create two: one with sandbox, one without.
    let url = server.api_url("/mcp/system-servers");
    let _ = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "list_a", "display_name": "A", "enabled": true,
            "transport_type": "stdio", "command": "python3", "args": [],
            "environment_variables": {}, "timeout_seconds": 30,
            "run_in_sandbox": true,
        }))
        .send()
        .await
        .unwrap();
    let _ = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "list_b", "display_name": "B", "enabled": true,
            "transport_type": "stdio", "command": "python3", "args": [],
            "environment_variables": {}, "timeout_seconds": 30,
            "run_in_sandbox": false,
        }))
        .send()
        .await
        .unwrap();

    let resp = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let servers = body["servers"].as_array().expect("servers array");
    // Verify every server has the field (no nulls / no missing).
    for s in servers {
        assert!(
            s.get("run_in_sandbox").map(|v| v.is_boolean()).unwrap_or(false),
            "server {:?} missing or non-bool run_in_sandbox",
            s.get("name")
        );
    }
    let a = servers.iter().find(|s| s["name"] == "list_a").expect("list_a");
    let b = servers.iter().find(|s| s["name"] == "list_b").expect("list_b");
    assert_eq!(a["run_in_sandbox"], true);
    assert_eq!(b["run_in_sandbox"], false);
}
