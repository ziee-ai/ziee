//! audit id all-f44bdb26e811 — permission revocation must take effect on the
//! NEXT request. A user reaches a built-in MCP endpoint (gated by
//! mcp_servers::read) via group membership; once an admin removes them from the
//! granting group, the permission union no longer includes the perm and the
//! next call is refused (403). Each request re-resolves the user's permissions,
//! so a revoked grant can't keep working. (The "during execution" case for
//! live streams is backstopped by the sync re-check; this asserts the REST
//! enforcement path.)

use serde_json::json;

use crate::common::TestServer;
use crate::common::test_helpers::{create_user_with_no_permissions, create_user_with_permissions};

#[tokio::test]
async fn revoking_group_membership_denies_subsequent_mcp_calls() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(
        &server,
        "revoke_admin",
        &["groups::create", "groups::edit", "groups::read"],
    )
    .await;
    // A user with NO baseline perms (default group removed) — access comes
    // ONLY from the custom group below.
    let user = create_user_with_no_permissions(&server, "revoke_user").await;
    let client = reqwest::Client::new();

    // Admin creates a group granting mcp_servers::read and assigns the user.
    let group: serde_json::Value = client
        .post(server.api_url("/groups"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "name": "mcp-access", "description": "x", "permissions": ["mcp_servers::read"] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let gid = group["id"].as_str().unwrap();
    let assign = client
        .post(server.api_url("/groups/assign"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "user_id": user.user_id, "group_id": gid }))
        .send()
        .await
        .unwrap();
    assert_eq!(assign.status(), 204, "assign user to group");

    let call = || {
        let url = server.api_url("/tool-result/mcp");
        let token = user.token.clone();
        async move {
            reqwest::Client::new()
                .post(&url)
                .header("Authorization", format!("Bearer {token}"))
                .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} }))
                .send()
                .await
                .unwrap()
        }
    };

    // With the grant the call succeeds.
    assert_eq!(call().await.status(), 200, "granted user can call the MCP endpoint");

    // Revoke: remove the user from the granting group.
    let remove = client
        .delete(server.api_url(&format!("/groups/{}/{}/remove", user.user_id, gid)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert!(remove.status().is_success(), "remove from group: {}", remove.text().await.unwrap_or_default());

    // The very next call is denied — the revocation took effect.
    assert_eq!(call().await.status(), 403, "after revocation the MCP call must be 403");
}
