//! Combined flow: project MCP-defaults snapshot + group MCP-server assignment.
//!
//! Audit gap `all-b1bef8dcbd84`. Neither existing test covers the two sources
//! TOGETHER:
//!   - `project::mcp_test` exercises the project-defaults snapshot, but only
//!     ever references a USER-OWNED server (accessible through ownership), and
//!     never involves a group.
//!   - `mcp::group_cascade_test` exercises group→server cascade, but never a
//!     project, a conversation, or the defaults snapshot.
//!
//! The two dimensions are orthogonal — a group assignment controls *which*
//! system servers a member can access; project MCP defaults control *how* a
//! conversation treats servers (approval mode + per-server auto-approved
//! tools). They MEET at the project-defaults validator
//! (`project_extension/handlers.rs::validate_mcp_server_access` →
//! `Repos.mcp.can_user_access_server`): a member may only reference a server in
//! their project defaults if they can access it — and for a SYSTEM server that
//! access comes solely from the group assignment.
//!
//! This test pins the combined contract end to end:
//!   1. A member CANNOT set project defaults referencing a system server before
//!      the group grants access (422 `MCP_SERVER_NOT_ACCESSIBLE`) — proving the
//!      project default genuinely depends on the group assignment.
//!   2. After the group assignment, the member sees the server via
//!      `GET /mcp/servers` (cascade) AND the same server-id is now an
//!      acceptable project default; that default then SNAPSHOTS onto a
//!      conversation created in the project.
//! No mocks — real admin + member tokens, real handlers, real DB.

use reqwest::StatusCode;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::TestServer;
use crate::common::test_helpers::{self, TestUser};

const ADMIN_PERMS: &[&str] = &[
    "mcp_servers_admin::create",
    "mcp_servers_admin::edit",
    "mcp_servers_admin::read",
    "mcp_servers::read",
    "groups::read",
    "groups::create",
    "groups::edit",
    "groups::assign_users",
];

/// A member needs to use Projects + read their accessible MCP servers.
const MEMBER_PERMS: &[&str] = &[
    "projects::create",
    "projects::read",
    "projects::edit",
    "conversations::create",
    "conversations::read",
    "conversations::edit",
    "mcp_servers::read",
];

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

async fn create_system_server(server: &TestServer, admin: &TestUser) -> Uuid {
    let unique = Uuid::new_v4().to_string();
    let resp = client()
        .post(server.api_url("/mcp/system-servers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": format!("combined_sys_{}", &unique[..8]),
            "display_name": "Combined System MCP Server",
            "description": "system server for project+group combined test",
            "enabled": true,
            "transport_type": "stdio",
            "command": "uvx",
            "args": ["mcp-server-fetch"],
            "timeout_seconds": 30
        }))
        .send()
        .await
        .expect("create system server");
    assert_eq!(resp.status(), StatusCode::CREATED, "create system server");
    let body: Value = resp.json().await.expect("parse server");
    Uuid::parse_str(body["id"].as_str().expect("server id")).expect("uuid")
}

async fn create_group(server: &TestServer, admin: &TestUser) -> Uuid {
    let resp = client()
        .post(server.api_url("/groups"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": format!("combined_grp_{}", &Uuid::new_v4().to_string()[..8]),
            "description": "project+group combined test group",
            "permissions": []
        }))
        .send()
        .await
        .expect("create group");
    assert_eq!(resp.status(), StatusCode::CREATED, "create group");
    let body: Value = resp.json().await.expect("parse group");
    Uuid::parse_str(body["id"].as_str().expect("group id")).expect("uuid")
}

async fn add_user_to_group(server: &TestServer, admin: &TestUser, user_id: &str, group_id: Uuid) {
    let resp = client()
        .post(server.api_url("/groups/assign"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "user_id": user_id, "group_id": group_id }))
        .send()
        .await
        .expect("assign user to group");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "add user to group");
}

async fn assign_server_to_group(
    server: &TestServer,
    admin: &TestUser,
    server_id: Uuid,
    group_id: Uuid,
) {
    let resp = client()
        .post(server.api_url(&format!("/mcp/system-servers/{}/groups", server_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "group_ids": [group_id] }))
        .send()
        .await
        .expect("assign server to group");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "assign server to group");
}

async fn accessible_server_ids(server: &TestServer, user: &TestUser) -> Vec<String> {
    let resp = client()
        .get(server.api_url("/mcp/servers?page=1&per_page=1000"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("list accessible servers");
    assert_eq!(resp.status(), StatusCode::OK, "list accessible");
    let body: Value = resp.json().await.expect("parse accessible list");
    body["servers"]
        .as_array()
        .expect("servers array")
        .iter()
        .map(|s| s["id"].as_str().unwrap_or_default().to_string())
        .collect()
}

async fn create_project(server: &TestServer, user: &TestUser, name: &str) -> String {
    let resp = client()
        .post(server.api_url("/projects"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "name": name }))
        .send()
        .await
        .expect("create project");
    assert_eq!(resp.status(), StatusCode::CREATED, "create project");
    let body: Value = resp.json().await.expect("parse project");
    body["id"].as_str().expect("project id").to_string()
}

/// PUT project MCP defaults referencing `server_id` in auto_approved_tools.
async fn put_project_mcp_defaults(
    server: &TestServer,
    user: &TestUser,
    project_id: &str,
    server_id: Uuid,
) -> reqwest::Response {
    client()
        .put(server.api_url(&format!("/projects/{}/mcp-settings", project_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "approval_mode": "auto_approve",
            "auto_approved_tools": [{ "server_id": server_id, "tools": ["fetch"] }],
            "disabled_servers": [],
        }))
        .send()
        .await
        .expect("put project mcp settings")
}

/// Create a conversation and attach it to the project — the attach triggers the
/// MCP-defaults snapshot (mirrors `project::helpers::create_project_conversation`).
async fn create_conversation_in_project(
    server: &TestServer,
    user: &TestUser,
    project_id: &str,
) -> String {
    let create = client()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({}))
        .send()
        .await
        .expect("create conversation");
    assert_eq!(create.status(), StatusCode::CREATED, "create conversation");
    let conv_id = create.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let attach = client()
        .post(server.api_url(&format!("/projects/{}/conversations/{}", project_id, conv_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("attach conversation");
    assert_eq!(attach.status(), StatusCode::OK, "attach conversation to project");
    conv_id
}

/// The combined flow: a member's project MCP defaults can only reference a
/// SYSTEM server once their GROUP is assigned that server, and the resulting
/// default then snapshots onto a conversation in that project.
#[tokio::test]
async fn project_mcp_defaults_reference_group_assigned_server_and_snapshot_onto_conversation() {
    let server = TestServer::start().await;
    let admin =
        test_helpers::create_user_with_permissions(&server, "combo_admin", ADMIN_PERMS).await;
    let member =
        test_helpers::create_user_with_permissions(&server, "combo_member", MEMBER_PERMS).await;

    let server_id = create_system_server(&server, &admin).await;
    let group_id = create_group(&server, &admin).await;

    let project_id = create_project(&server, &member, "Combined Hub").await;

    // (1) BEFORE the group grants access: the member is not in the group, so the
    // system server is not accessible — referencing it in project defaults must
    // be rejected by `validate_mcp_server_access` (the project default genuinely
    // depends on the group assignment, not just on the server existing).
    assert!(
        !accessible_server_ids(&server, &member)
            .await
            .contains(&server_id.to_string()),
        "member must NOT see the system server before any group grants it"
    );
    let denied = put_project_mcp_defaults(&server, &member, &project_id, server_id).await;
    assert_eq!(
        denied.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "project defaults referencing an inaccessible system server must be rejected"
    );
    let denied_body: Value = denied.json().await.unwrap();
    assert_eq!(
        denied_body["error"]
            .as_str()
            .or_else(|| denied_body["code"].as_str())
            .or_else(|| denied_body["error_code"].as_str()),
        Some("MCP_SERVER_NOT_ACCESSIBLE"),
        "rejection must be the access-validator error, got: {denied_body}"
    );

    // (2) The admin assigns the system server to the member's group (cascade
    // grant) — the two sources now both apply.
    add_user_to_group(&server, &admin, &member.user_id, group_id).await;
    assign_server_to_group(&server, &admin, server_id, group_id).await;

    // Group cascade: the member now sees the system server via list_accessible.
    assert!(
        accessible_server_ids(&server, &member)
            .await
            .contains(&server_id.to_string()),
        "member must see the system server after the group assignment (cascade grant)"
    );

    // The same project-defaults PUT now succeeds — valid only because the group
    // assignment granted access to the system server.
    let ok = put_project_mcp_defaults(&server, &member, &project_id, server_id).await;
    assert_eq!(
        ok.status(),
        StatusCode::OK,
        "project defaults referencing the now-group-accessible server must be accepted"
    );

    // (3) A conversation created in the project snapshots those project defaults
    // — so the conversation's MCP settings carry BOTH the project approval mode
    // AND the group-assigned server id (the combination neither single-source
    // test reaches).
    let conv_id = create_conversation_in_project(&server, &member, &project_id).await;
    let conv_resp = client()
        .get(server.api_url(&format!("/conversations/{}/mcp-settings", conv_id)))
        .header("Authorization", format!("Bearer {}", member.token))
        .send()
        .await
        .expect("get conversation mcp settings");
    assert_eq!(
        conv_resp.status(),
        StatusCode::OK,
        "the conversation must carry a snapshotted MCP-settings row"
    );
    let conv_body: Value = conv_resp.json().await.unwrap();
    let settings = &conv_body["settings"];
    assert!(
        !settings.is_null(),
        "snapshotted settings must be present: {conv_body}"
    );
    assert_eq!(
        settings["approval_mode"], "auto_approve",
        "conversation must inherit the project's approval mode (project-default source)"
    );
    // The group-assigned server id flowed through the project default into the
    // conversation snapshot (group source meeting project source).
    assert!(
        settings.to_string().contains(&server_id.to_string()),
        "conversation snapshot must reference the group-assigned server id: {settings}"
    );

    // And the member still independently sees the server via the group cascade —
    // both sources coexist for the conversation's owner.
    assert!(
        accessible_server_ids(&server, &member)
            .await
            .contains(&server_id.to_string()),
        "the group-assigned server remains accessible alongside the project snapshot"
    );
}
