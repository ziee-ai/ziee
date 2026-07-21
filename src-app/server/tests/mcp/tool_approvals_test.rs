//! HTTP route tests for the admin per-(server, tool) approval-mode defaults
//! (ITEM-54 / DEC-112):
//!   GET  /api/mcp/servers/{id}/tool-approvals
//!   PUT  /api/mcp/servers/{id}/tool-approvals/{tool}
//!
//! Covers: 401 unauth + 403 wrong-perm on both endpoints, set→get roundtrip
//! (effective mode), clear, foreign-id 404, and that the override is removed
//! together with the server row on delete (jsonb-on-row storage → inherent
//! "cascade").

use crate::common::{TestServer, test_helpers};
use serde_json::json;
use uuid::Uuid;

/// Admin with the full system-MCP perm set.
async fn admin(server: &TestServer) -> test_helpers::TestUser {
    test_helpers::create_user_with_permissions(
        server,
        "admin",
        &[
            "mcp_servers_admin::create",
            "mcp_servers_admin::read",
            "mcp_servers_admin::edit",
            "mcp_servers_admin::delete",
        ],
    )
    .await
}

/// Create a SYSTEM http MCP server (enabled:false so no create-time probe fires;
/// the url is intentionally unreachable so the tool-approvals GET probe fails
/// fast and exercises the `tools_unreachable` fallback). Returns its id.
async fn create_system_http_server(server: &TestServer, token: &str) -> Uuid {
    let payload = json!({
        "name": "tool_approval_srv",
        "display_name": "Tool Approval Server",
        "enabled": false,
        "transport_type": "http",
        "url": "http://127.0.0.1:9/mcp",
        "timeout_seconds": 3,
    });
    let resp = reqwest::Client::new()
        .post(server.api_url("/mcp/system-servers"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&payload)
        .send()
        .await
        .expect("create system server");
    assert_eq!(resp.status(), 201, "system server create should succeed");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["is_system"], true);
    Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn get_tool_approvals_requires_auth() {
    let server = TestServer::start().await;
    let admin = admin(&server).await;
    let id = create_system_http_server(&server, &admin.token).await;

    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/mcp/servers/{id}/tool-approvals")))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401, "no token → 401");
}

#[tokio::test]
async fn get_tool_approvals_wrong_perm_forbidden() {
    let server = TestServer::start().await;
    let admin = admin(&server).await;
    let id = create_system_http_server(&server, &admin.token).await;

    // Authenticated user WITHOUT any mcp_servers_admin perm.
    let user =
        test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::read"]).await;
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/mcp/servers/{id}/tool-approvals")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403, "non-admin → 403");
}

#[tokio::test]
async fn put_tool_approval_requires_auth() {
    let server = TestServer::start().await;
    let admin = admin(&server).await;
    let id = create_system_http_server(&server, &admin.token).await;

    let resp = reqwest::Client::new()
        .put(server.api_url(&format!("/mcp/servers/{id}/tool-approvals/some_tool")))
        .json(&json!({ "mode": "auto_approve" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401, "no token → 401");
}

#[tokio::test]
async fn put_tool_approval_wrong_perm_forbidden() {
    let server = TestServer::start().await;
    let admin = admin(&server).await;
    let id = create_system_http_server(&server, &admin.token).await;

    // A user with only mcp_servers_admin::read cannot EDIT (PUT needs ::edit).
    let reader = test_helpers::create_user_with_permissions(
        &server,
        "reader",
        &["mcp_servers_admin::read"],
    )
    .await;
    let resp = reqwest::Client::new()
        .put(server.api_url(&format!("/mcp/servers/{id}/tool-approvals/some_tool")))
        .header("Authorization", format!("Bearer {}", reader.token))
        .json(&json!({ "mode": "auto_approve" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403, "read-only admin cannot set → 403");
}

#[tokio::test]
async fn set_then_get_shows_effective_mode_then_clear() {
    let server = TestServer::start().await;
    let admin = admin(&server).await;
    let id = create_system_http_server(&server, &admin.token).await;
    let client = reqwest::Client::new();

    // Set an auto_approve override for "my_tool".
    let put_resp = client
        .put(server.api_url(&format!("/mcp/servers/{id}/tool-approvals/my_tool")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "mode": "auto_approve" }))
        .send()
        .await
        .unwrap();
    assert_eq!(put_resp.status(), 200);
    let put_body: serde_json::Value = put_resp.json().await.unwrap();
    assert_eq!(put_body["tool_name"], "my_tool");
    assert_eq!(put_body["effective_mode"], "auto_approve");
    assert_eq!(put_body["has_override"], true);

    // GET reflects the override (server is unreachable, so the override-keyed
    // tool appears via the fallback with the correct effective mode).
    let get_resp = client
        .get(server.api_url(&format!("/mcp/servers/{id}/tool-approvals")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(get_resp.status(), 200);
    let body: serde_json::Value = get_resp.json().await.unwrap();
    assert_eq!(body["server_default_mode"], "manual_approve");
    let tools = body["tools"].as_array().expect("tools array");
    let my = tools
        .iter()
        .find(|t| t["tool_name"] == "my_tool")
        .expect("my_tool present in tool-approvals");
    assert_eq!(my["effective_mode"], "auto_approve");
    assert_eq!(my["has_override"], true);

    // Clear the override (mode: null) → GET no longer lists it as an override.
    let clear_resp = client
        .put(server.api_url(&format!("/mcp/servers/{id}/tool-approvals/my_tool")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "mode": serde_json::Value::Null }))
        .send()
        .await
        .unwrap();
    assert_eq!(clear_resp.status(), 200);
    let clear_body: serde_json::Value = clear_resp.json().await.unwrap();
    assert_eq!(clear_body["has_override"], false);
    assert_eq!(clear_body["effective_mode"], "manual_approve");

    let get2 = client
        .get(server.api_url(&format!("/mcp/servers/{id}/tool-approvals")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    let body2: serde_json::Value = get2.json().await.unwrap();
    let still = body2["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["tool_name"] == "my_tool");
    assert!(!still, "cleared override should no longer surface as an override tool");
}

#[tokio::test]
async fn foreign_or_missing_id_is_404() {
    let server = TestServer::start().await;
    let admin = admin(&server).await;
    let client = reqwest::Client::new();
    let bogus = Uuid::new_v4();

    let get_resp = client
        .get(server.api_url(&format!("/mcp/servers/{bogus}/tool-approvals")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(get_resp.status(), 404, "unknown id GET → 404");

    let put_resp = client
        .put(server.api_url(&format!("/mcp/servers/{bogus}/tool-approvals/some_tool")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "mode": "auto_approve" }))
        .send()
        .await
        .unwrap();
    assert_eq!(put_resp.status(), 404, "unknown id PUT → 404");
}

#[tokio::test]
async fn override_removed_when_server_deleted() {
    // jsonb-on-row storage: deleting the server row removes its tool-approval
    // overrides too (no separate table / FK cascade needed).
    let server = TestServer::start().await;
    let admin = admin(&server).await;
    let id = create_system_http_server(&server, &admin.token).await;
    let client = reqwest::Client::new();

    client
        .put(server.api_url(&format!("/mcp/servers/{id}/tool-approvals/doomed_tool")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "mode": "disabled" }))
        .send()
        .await
        .unwrap();

    let del = client
        .delete(server.api_url(&format!("/mcp/system-servers/{id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 204, "system server delete → 204");

    // The server (and its overrides) are gone → tool-approvals GET → 404.
    let get_resp = client
        .get(server.api_url(&format!("/mcp/servers/{id}/tool-approvals")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(get_resp.status(), 404, "deleted server → 404");
}
