//! Realtime-sync emission coverage for the MCP-server entities.
//!
//! Proves that a real REST mutation through the production handler emits the
//! right `sync` frame to the right audience, end-to-end (handler →
//! `sync_publish` → registry → SSE), via `SyncProbe`. Per the routing table in
//! `modules/sync/event.rs`, `SyncEntity` + `SyncAction` serialize `snake_case`,
//! so the wire strings are `mcp_server` / `mcp_server_system` /
//! `user_mcp_server` and `create` / `update`.
//!
//! Three audiences are exercised:
//!
//! - `mcp_server` (OWNER): a user creating their OWN (user-scoped) MCP server
//!   sees the frame on their stream; a different user stays silent.
//!   (`handlers/user.rs::create_user_server` → `SyncEntity::McpServer` scoped to
//!   `Some(auth.user.id)`.)
//! - `mcp_server_system` (PERMISSION `mcp_servers_admin::read`) and
//!   `user_mcp_server` (PERMISSION `mcp_servers::read`): a system-server
//!   mutation is DUAL-AUDIENCE — `handlers/system.rs` emits BOTH frames on the
//!   same mutation. The admin actor (holding `mcp_servers_admin::read`) observes
//!   `mcp_server_system`; a SEPARATE user holding only `mcp_servers::read`
//!   observes `user_mcp_server`; a user holding NEITHER stays fully silent.
//!
//! Mutations use a plain reqwest request (NO `X-Sync-Connection-Id` header), so
//! the actor's own stream is NOT origin-skipped and receives its audience's
//! frame just like any other subscriber.

use std::time::Duration;

use serde_json::json;

use crate::common::sync_probe::SyncProbe;

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);
const SILENCE_WINDOW: Duration = Duration::from_secs(1);

/// POST /mcp/servers as `token` (a user's OWN HTTP MCP server), returning
/// the new server id.
///
/// HTTP (not stdio) on purpose: the MCP user policy default from
/// migration 84 (`allowed_transports: ['http', 'stdio']`) forces stdio
/// user-server creates into the sandbox, which the test deployment has
/// disabled (`code_sandbox.enabled = false`). That makes a stdio create
/// 422 with `MCP_SANDBOX_DISABLED` BEFORE the create handler ever calls
/// `sync_publish`, so the sync emit we want to observe never fires.
/// Mirrors the same fix in `catalog_hermetic.rs::user_mcp_install_*`.
async fn create_user_mcp_server(
    server: &crate::common::TestServer,
    token: &str,
    name: &str,
) -> String {
    let res = reqwest::Client::new()
        .post(server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": name,
            "display_name": "Sync User Server",
            "transport_type": "http",
            "url": "https://example.invalid/mcp",
        }))
        .send()
        .await
        .unwrap();
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    assert_eq!(
        status, 201,
        "user MCP server create should return 201, got {status}: {body}"
    );
    let row: serde_json::Value = serde_json::from_str(&body).unwrap();
    row["id"].as_str().unwrap().to_string()
}

/// POST /mcp/system-servers as `token` (an admin SYSTEM stdio server),
/// returning the new server id. Body mirrors
/// `tests/mcp/mod.rs::create_test_system_server`.
async fn create_system_mcp_server(
    server: &crate::common::TestServer,
    token: &str,
    name: &str,
) -> String {
    let res = reqwest::Client::new()
        .post(server.api_url("/mcp/system-servers"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": name,
            "display_name": "Sync System Server",
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
        201,
        "system MCP server create should return 201"
    );
    let row: serde_json::Value = res.json().await.unwrap();
    row["id"].as_str().unwrap().to_string()
}

// =====================================================
// mcp_server — OWNER audience
// =====================================================

#[tokio::test]
async fn user_mcp_server_create_is_delivered_to_owner_not_to_other_users() {
    let server = crate::common::TestServer::start().await;

    // The owner needs `mcp_servers::create` to create their own server.
    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_mcp_owner",
        &["mcp_servers::create"],
    )
    .await;
    // A different user holds only the baseline (default group → profile::read);
    // enough to subscribe, but they must NEVER see the owner-scoped frame.
    let other = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_mcp_other",
        &[],
    )
    .await;

    let mut owner_probe = SyncProbe::open(&server, &owner.token).await;
    let mut other_probe = SyncProbe::open(&server, &other.token).await;

    let id = create_user_mcp_server(&server, &owner.token, "my_local_server").await;

    let frame = owner_probe
        .expect_event("mcp_server", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(frame.id, id, "the frame must carry the new server's id");

    // Owner-scoped: the unrelated user must observe nothing.
    other_probe.expect_silence(SILENCE_WINDOW).await;
}

// =====================================================
// mcp_server_system + user_mcp_server — DUAL-AUDIENCE
// (PERMISSION "mcp_servers_admin::read" + "mcp_servers::read")
// =====================================================

#[tokio::test]
async fn system_mcp_server_mutation_delivers_to_admin_read_and_user_read_holders_only() {
    let server = crate::common::TestServer::start().await;

    // The actor manages system servers (create + edit) AND holds
    // `mcp_servers_admin::read` so it sits in the `mcp_server_system` audience.
    let actor = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_sys_admin",
        &[
            "mcp_servers_admin::create",
            "mcp_servers_admin::edit",
            "mcp_servers_admin::read",
        ],
    )
    .await;
    // A user holding ONLY `mcp_servers::read` (+ `profile::read` so it can open
    // the sync stream): receives the dual-audience `user_mcp_server` frame,
    // never `mcp_server_system`. `only_permissions` strips the default group so
    // no other baseline read smuggles in.
    let user_reader = crate::common::test_helpers::create_user_with_only_permissions(
        &server,
        "sync_sys_user_reader",
        &["mcp_servers::read", "profile::read"],
    )
    .await;
    // A user holding ONLY `profile::read` (enough to subscribe) but no
    // `mcp_servers_admin::read` and no `mcp_servers::read` (stripped from the
    // default group). Must stay fully silent on BOTH frames.
    let bystander = crate::common::test_helpers::create_user_with_only_permissions(
        &server,
        "sync_sys_bystander",
        &["profile::read"],
    )
    .await;

    let mut actor_probe = SyncProbe::open(&server, &actor.token).await;
    let mut user_reader_probe = SyncProbe::open(&server, &user_reader.token).await;
    let mut bystander_probe = SyncProbe::open(&server, &bystander.token).await;

    // --- Create: dual-audience emit ---
    let id = create_system_mcp_server(&server, &actor.token, "sync_system_server").await;

    // The admin actor (mcp_servers_admin::read) observes mcp_server_system.
    let admin_frame = actor_probe
        .expect_event("mcp_server_system", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(
        admin_frame.id, id,
        "admin frame must carry the new system server's id"
    );

    // A separate mcp_servers::read holder observes the dual-audience
    // user_mcp_server frame for the SAME mutation.
    let user_frame = user_reader_probe
        .expect_event("user_mcp_server", "create", EVENT_TIMEOUT)
        .await;
    assert_eq!(
        user_frame.id, id,
        "user_mcp_server frame must carry the same system server's id"
    );

    // A user lacking BOTH perms stays silent on the create.
    bystander_probe.expect_silence(SILENCE_WINDOW).await;

    // --- Update: dual-audience emit on the same server ---
    let update_resp = reqwest::Client::new()
        .put(server.api_url(&format!("/mcp/system-servers/{}", id)))
        .header("Authorization", format!("Bearer {}", actor.token))
        .json(&json!({
            "display_name": "Sync System Server (renamed)",
            "transport_type": "stdio",
            "command": "node",
            "args": ["updated.js"],
            "enabled": false,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(update_resp.status(), 200, "system server update should be 200");

    let admin_update = actor_probe
        .expect_event("mcp_server_system", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(admin_update.id, id);

    let user_update = user_reader_probe
        .expect_event("user_mcp_server", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(user_update.id, id);

    bystander_probe.expect_silence(SILENCE_WINDOW).await;
}
