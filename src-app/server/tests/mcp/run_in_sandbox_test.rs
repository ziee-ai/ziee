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

    // No run_in_sandbox key at all. `enabled: false` so the
    // create-time connection probe (from my connection_health gate)
    // doesn't fire — a real python3 server isn't running here and
    // the probe would time out at 408.
    let payload = json!({
        "name": "default_server",
        "display_name": "Default",
        "enabled": false,
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
        "enabled": false,
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
            "enabled": false,
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
async fn user_mode_stdio_create_is_gated_by_user_policy() {
    // Replaces the OLD `user_mode_create_silently_ignores_run_in_sandbox_flag`
    // which asserted the legacy contract: user-server INSERT hard-
    // coded run_in_sandbox=false. New contract (migration 84 +
    // user_policy::enforce_on_user_create): user stdio is
    // FORCE-sandboxed, requires `code_sandbox.enabled` at deployment
    // time. With sandbox disabled in tests, the policy projection in
    // `user_policy::load` filters `'stdio'` out of `allowed_transports`,
    // so the user create rejects with 422 MCP_TRANSPORT_NOT_ALLOWED
    // (the upstream gate) before reaching MCP_SANDBOX_DISABLED.
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
            "enabled": false,
            "transport_type": "stdio",
            "command": "python3",
            "args": [],
            "environment_variables": {},
            "timeout_seconds": 30,
            // run_in_sandbox flag is ignored by the user-create
            // handler — policy force-sets it to true on stdio.
            "run_in_sandbox": false,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        422,
        "user stdio requires sandbox per user-policy; tests run without sandbox"
    );
    let body: serde_json::Value = response.json().await.unwrap();
    // With sandbox disabled, the policy load filters stdio out of
    // `allowed_transports` so the upstream transport gate fires
    // before the sandbox-required gate. Either is correct; the
    // upstream code is what the user actually hits.
    assert_eq!(body["error_code"], "MCP_TRANSPORT_NOT_ALLOWED");
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

// ---------------------------------------------------------------------
// sandbox_flavor column + tiered command allowlist
// ---------------------------------------------------------------------

async fn admin_with(server: &TestServer, perms: &[&str]) -> test_helpers::TestUser {
    test_helpers::create_user_with_permissions(server, "admin", perms).await
}

#[tokio::test]
async fn create_system_server_defaults_sandbox_flavor_to_full() {
    let server = TestServer::start().await;
    let admin = admin_with(&server, &["mcp_servers_admin::create"]).await;

    // No sandbox_flavor key → column default 'full'.
    let body: serde_json::Value = reqwest::Client::new()
        .post(&server.api_url("/mcp/system-servers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "flavor_default", "display_name": "Default Flavor",
            "enabled": false, "transport_type": "stdio", "command": "uvx",
            "args": [], "environment_variables": {}, "timeout_seconds": 30,
            "run_in_sandbox": true,
        }))
        .send().await.unwrap()
        .json().await.unwrap();
    // create returns the McpServer flattened with optional
    // `connection_warning` sibling (see McpServerWithHealthWarning).
    assert_eq!(body["sandbox_flavor"], "full");
}

#[tokio::test]
async fn create_system_server_persists_and_validates_sandbox_flavor() {
    let server = TestServer::start().await;
    let admin = admin_with(&server, &["mcp_servers_admin::create"]).await;
    let client = reqwest::Client::new();

    // Explicit 'minimal' round-trips.
    let body: serde_json::Value = client
        .post(&server.api_url("/mcp/system-servers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "flavor_minimal", "display_name": "Min", "enabled": false,
            "transport_type": "stdio", "command": "python3", "args": [],
            "environment_variables": {}, "timeout_seconds": 30,
            "run_in_sandbox": true, "sandbox_flavor": "minimal",
        }))
        .send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(body["sandbox_flavor"], "minimal");

    // Unknown flavor → 400.
    let resp = client
        .post(&server.api_url("/mcp/system-servers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "flavor_bogus", "display_name": "Bogus", "enabled": false,
            "transport_type": "stdio", "command": "python3", "args": [],
            "environment_variables": {}, "timeout_seconds": 30,
            "run_in_sandbox": true, "sandbox_flavor": "does-not-exist",
        }))
        .send().await.unwrap();
    assert_eq!(resp.status(), 400, "unknown flavor must be rejected");
}

#[tokio::test]
async fn host_tier_rejects_disallowed_command_but_sandbox_tier_allows_any() {
    let server = TestServer::start().await;
    let admin = admin_with(&server, &["mcp_servers_admin::create"]).await;
    let client = reqwest::Client::new();
    let url = server.api_url("/mcp/system-servers");

    // Host tier (run_in_sandbox=false): a non-allowlisted command is rejected.
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "host_deno", "display_name": "Host Deno", "enabled": false,
            "transport_type": "stdio", "command": "deno", "args": [],
            "environment_variables": {}, "timeout_seconds": 30,
            "run_in_sandbox": false,
        }))
        .send().await.unwrap();
    assert_eq!(resp.status(), 400, "deno not allowed on the host path");

    // Same command, but sandboxed → accepted (bwrap isolation is the guard).
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "sandbox_deno", "display_name": "Sandbox Deno", "enabled": false,
            "transport_type": "stdio", "command": "deno", "args": [],
            "environment_variables": {}, "timeout_seconds": 30,
            "run_in_sandbox": true, "sandbox_flavor": "full",
        }))
        .send().await.unwrap();
    assert_eq!(resp.status(), 201, "any command allowed when sandboxed");

    // An allowlisted host command (uvx) is fine without sandbox.
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "host_uvx", "display_name": "Host uvx", "enabled": false,
            "transport_type": "stdio", "command": "uvx", "args": [],
            "environment_variables": {}, "timeout_seconds": 30,
            "run_in_sandbox": false,
        }))
        .send().await.unwrap();
    assert_eq!(resp.status(), 201);
}

#[tokio::test]
async fn user_create_stdio_is_gated_by_sandbox_policy_not_host_allowlist() {
    // Replaces the OLD `user_create_rejects_disallowed_host_command`.
    // The legacy contract had user servers always running on the
    // host (run_in_sandbox ignored), so a non-allowlisted command
    // like `deno` was 400-rejected by the HOST tier. The new
    // contract (migration 84) FORCE-sandboxes user stdio, lifting
    // the host allowlist for users entirely (bwrap isolation is
    // the guard). What blocks the user create now is whether the
    // sandbox is enabled at all — 422 MCP_SANDBOX_DISABLED in
    // sandbox-off test environments, regardless of command.
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server, "user", &["mcp_servers::create"],
    ).await;

    let resp = reqwest::Client::new()
        .post(&server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "user_deno", "display_name": "User Deno", "enabled": false,
            "transport_type": "stdio", "command": "deno", "args": [],
            "environment_variables": {}, "timeout_seconds": 30,
            "run_in_sandbox": true,
        }))
        .send().await.unwrap();
    assert_eq!(resp.status(), 422, "user stdio gated by sandbox policy, not host allowlist");
    let body: serde_json::Value = resp.json().await.unwrap();
    // With sandbox disabled, the policy load filters stdio out of
    // `allowed_transports` so the upstream transport gate fires
    // before the sandbox-required gate. Either is correct; the
    // point is that the user gets a 422 — NOT a host allowlist hit.
    assert_eq!(body["error_code"], "MCP_TRANSPORT_NOT_ALLOWED");
}

#[tokio::test]
async fn update_system_server_can_change_sandbox_flavor() {
    let server = TestServer::start().await;
    let admin = admin_with(
        &server,
        &["mcp_servers_admin::create", "mcp_servers_admin::edit"],
    ).await;
    let client = reqwest::Client::new();

    let created: serde_json::Value = client
        .post(&server.api_url("/mcp/system-servers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "flavor_update", "display_name": "FU", "enabled": false,
            "transport_type": "stdio", "command": "uvx", "args": [],
            "environment_variables": {}, "timeout_seconds": 30,
            "run_in_sandbox": true, "sandbox_flavor": "full",
        }))
        .send().await.unwrap()
        .json().await.unwrap();
    // Response shape is McpServerWithHealthWarning — McpServer
    // fields flattened with optional `connection_warning` sibling.
    let id = created["id"].as_str().unwrap();
    assert_eq!(created["sandbox_flavor"], "full");

    let updated: serde_json::Value = client
        .put(&server.api_url(&format!("/mcp/system-servers/{}", id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "sandbox_flavor": "minimal" }))
        .send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(updated["sandbox_flavor"], "minimal");
}

/// User-policy force-sandbox enforcement, through the REAL user-create handler.
/// The default MCP user policy allows the `stdio` transport, and
/// `enforce_on_user_create` then FORCES the new user server into the sandbox —
/// which `require_sandbox_state` gates on `code_sandbox` being enabled. The
/// test deployment runs with `code_sandbox.enabled = false`, so a user trying
/// to create a stdio server is rejected pre-persist with 422
/// `MCP_SANDBOX_DISABLED` (rather than silently creating an un-sandboxed stdio
/// server). This exercises the force-sandbox path the chat consumer relies on.
#[tokio::test]
async fn user_stdio_server_create_is_force_sandbox_gated_when_sandbox_disabled() {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "force_sandbox_user",
        &["mcp_servers::create", "mcp_servers::read"],
    )
    .await;

    let res = reqwest::Client::new()
        .post(server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "user_stdio_srv",
            "display_name": "User stdio server",
            "transport_type": "stdio",
            "command": "node",
            "args": ["server.js"],
            "enabled": true,
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.status(),
        422,
        "force-sandbox enforcement must reject a stdio user server while sandbox is disabled"
    );
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(
        body["error_code"], "MCP_SANDBOX_DISABLED",
        "the rejection must be the force-sandbox guard, not a generic error: {body}"
    );
}
