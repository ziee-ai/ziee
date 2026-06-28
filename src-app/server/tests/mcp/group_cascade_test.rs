//! Group-cascade MCP assignment integration tests.
//!
//! Covers the audit gap `all-ec9a157c175b`: assigning a system MCP server to a
//! user GROUP must cascade access to that group's members, and removing the
//! assignment (or the member) must revoke it.
//!
//! Distinct from the existing coverage:
//!   - `mcp_extension_test::test_mcp_user_can_access_group_servers` proves the
//!     GRANT but only through the chat-send (`enable_mcp`) path, never the
//!     user-facing `GET /mcp/servers` (`list_accessible`) endpoint.
//!   - `mcp_extension_test::test_mcp_access_revocation_is_reevaluated_per_request`
//!     revokes by deleting the `user_group_mcp_servers` row with raw SQL, never
//!     the real `remove_server_from_group` (DELETE) handler.
//!
//! These tests drive the REAL admin endpoints (POST `/mcp/system-servers/{id}/groups`
//! to assign, DELETE `/mcp/system-servers/{id}/groups/{group_id}` to unassign,
//! POST `/groups/assign` for membership) and assert the effect through the REAL
//! `GET /mcp/servers` list. No mocks.

use serde_json::json;
use uuid::Uuid;

use crate::common::TestServer;
use crate::common::test_helpers::{self, TestUser};

/// Admin permissions needed to create a system server, a group, assign
/// members, and manage the server↔group assignment.
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

/// A plain member only needs to be able to LIST their accessible servers.
const MEMBER_PERMS: &[&str] = &["mcp_servers::read"];

async fn create_system_server(server: &TestServer, admin: &TestUser) -> Uuid {
    let unique = Uuid::new_v4().to_string();
    let payload = json!({
        "name": format!("cascade_server_{}", &unique[..8]),
        "display_name": "Cascade System MCP Server",
        "description": "system server for group-cascade tests",
        "enabled": true,
        "transport_type": "stdio",
        "command": "uvx",
        "args": ["mcp-server-fetch"],
        "timeout_seconds": 30
    });
    let resp = reqwest::Client::new()
        .post(&server.api_url("/mcp/system-servers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("create system server");
    assert_eq!(resp.status(), 201, "should create system server");
    let body: serde_json::Value = resp.json().await.expect("parse server");
    Uuid::parse_str(body["id"].as_str().expect("server id")).expect("uuid")
}

async fn create_group(server: &TestServer, admin: &TestUser) -> Uuid {
    let resp = reqwest::Client::new()
        .post(&server.api_url("/groups"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": format!("cascade_group_{}", &Uuid::new_v4().to_string()[..8]),
            "description": "group-cascade test group",
            "permissions": []
        }))
        .send()
        .await
        .expect("create group");
    assert_eq!(resp.status(), 201, "should create group");
    let body: serde_json::Value = resp.json().await.expect("parse group");
    Uuid::parse_str(body["id"].as_str().expect("group id")).expect("uuid")
}

async fn add_user_to_group(server: &TestServer, admin: &TestUser, user_id: &str, group_id: Uuid) {
    let resp = reqwest::Client::new()
        .post(&server.api_url("/groups/assign"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "user_id": user_id, "group_id": group_id }))
        .send()
        .await
        .expect("assign user to group");
    assert_eq!(resp.status(), 204, "should add user to group");
}

/// POST `/mcp/system-servers/{id}/groups` — assign (replaces all assignments).
async fn assign_server_to_groups(
    server: &TestServer,
    admin: &TestUser,
    server_id: Uuid,
    group_ids: &[Uuid],
) {
    let resp = reqwest::Client::new()
        .post(&server.api_url(&format!("/mcp/system-servers/{}/groups", server_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "group_ids": group_ids }))
        .send()
        .await
        .expect("assign server to groups");
    assert_eq!(resp.status(), 204, "should assign server to groups");
}

/// The user-facing accessible-server id list (`GET /mcp/servers`).
async fn accessible_server_ids(server: &TestServer, user: &TestUser) -> Vec<String> {
    let resp = reqwest::Client::new()
        .get(&server.api_url("/mcp/servers?page=1&per_page=1000"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("list accessible servers");
    assert_eq!(resp.status(), 200, "list accessible should be 200");
    let body: serde_json::Value = resp.json().await.expect("parse accessible list");
    body["servers"]
        .as_array()
        .expect("servers array")
        .iter()
        .map(|s| s["id"].as_str().unwrap_or_default().to_string())
        .collect()
}

/// Assign a system server to a group the user belongs to → it appears in the
/// user's accessible list; remove the assignment via the REAL DELETE endpoint
/// → it disappears. Both directions through the production handlers.
#[tokio::test]
async fn group_assignment_cascades_to_member_and_revokes_via_endpoint() {
    let server = TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(&server, "cascade_admin", ADMIN_PERMS).await;
    let user = test_helpers::create_user_with_permissions(&server, "cascade_member", MEMBER_PERMS).await;

    let server_id = create_system_server(&server, &admin).await;
    let group_id = create_group(&server, &admin).await;
    add_user_to_group(&server, &admin, &user.user_id, group_id).await;

    // Before assignment: the member cannot see the system server.
    assert!(
        !accessible_server_ids(&server, &user).await.contains(&server_id.to_string()),
        "member must NOT see the server before it is assigned to their group"
    );

    // Assign the server to the group (real POST endpoint).
    assign_server_to_groups(&server, &admin, server_id, &[group_id]).await;

    // Cascade GRANT: the member now sees it through list_accessible.
    assert!(
        accessible_server_ids(&server, &user).await.contains(&server_id.to_string()),
        "member must see the server after it is assigned to their group (cascade grant)"
    );

    // Remove the server from the group via the REAL DELETE endpoint.
    let del = reqwest::Client::new()
        .delete(&server.api_url(&format!("/mcp/system-servers/{}/groups/{}", server_id, group_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("remove server from group");
    assert_eq!(del.status(), 204, "should remove server from group");

    // Cascade REVOKE: the member can no longer see it.
    assert!(
        !accessible_server_ids(&server, &user).await.contains(&server_id.to_string()),
        "member must lose access after the server is removed from their group (cascade revoke)"
    );
}

/// The cascade follows GROUP MEMBERSHIP: a non-member of the assigned group
/// never sees the server, and the empty-`group_ids` assignment (un-assign all)
/// also revokes — exercising `assign_server_to_groups`'s replace semantics.
#[tokio::test]
async fn group_cascade_respects_membership_and_replace_semantics() {
    let server = TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(&server, "cascade_admin2", ADMIN_PERMS).await;
    let member = test_helpers::create_user_with_permissions(&server, "cascade_in", MEMBER_PERMS).await;
    let outsider = test_helpers::create_user_with_permissions(&server, "cascade_out", MEMBER_PERMS).await;

    let server_id = create_system_server(&server, &admin).await;
    let group_id = create_group(&server, &admin).await;
    add_user_to_group(&server, &admin, &member.user_id, group_id).await;
    assign_server_to_groups(&server, &admin, server_id, &[group_id]).await;

    // Member sees it; outsider (not in the group) does not.
    assert!(
        accessible_server_ids(&server, &member).await.contains(&server_id.to_string()),
        "group member should see the assigned server"
    );
    assert!(
        !accessible_server_ids(&server, &outsider).await.contains(&server_id.to_string()),
        "a non-member must never see a server assigned only to another group"
    );

    // Replace assignments with an empty set → un-assigns from every group.
    assign_server_to_groups(&server, &admin, server_id, &[]).await;
    assert!(
        !accessible_server_ids(&server, &member).await.contains(&server_id.to_string()),
        "member must lose access after the server's group assignments are cleared"
    );
}
