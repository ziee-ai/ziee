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
use uuid::Uuid;

use super::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};
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

// =====================================================
// mcp_tool_call — OWNER audience (recorded tool invocation)
// =====================================================

#[tokio::test]
async fn tool_call_create_is_delivered_to_owner_not_to_other_users() {
    let server = crate::common::TestServer::start().await;
    let mock = MockMcpServer::start().await;

    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_tc_owner",
        &["mcp_servers::create", "mcp_servers::read"],
    )
    .await;
    // Default-group baseline (profile::read) — enough to subscribe, never sees
    // the owner-scoped tool-call frame.
    let other = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_tc_other",
        &[],
    )
    .await;

    mock.on_method(
        "tools/call",
        MockResponse::JsonOk(json!({ "content": [{ "type": "text", "text": "ok" }] })),
    );

    // Register the mock as the owner's server BEFORE opening the probes, so the
    // `mcp_server` create frame isn't in the captured stream — the only frame
    // we should observe is the tool-call recording.
    let server_id = {
        let res = reqwest::Client::new()
            .post(server.api_url("/mcp/servers"))
            .header("Authorization", format!("Bearer {}", owner.token))
            .json(&json!({
                "name": "sync_tc_mock",
                "display_name": "Sync tool-call mock",
                "transport_type": "http",
                "url": mock.base_url(),
                "enabled": true,
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 201);
        let row: serde_json::Value = res.json().await.unwrap();
        row["id"].as_str().unwrap().to_string()
    };

    let mut owner_probe = SyncProbe::open(&server, &owner.token).await;
    let mut other_probe = SyncProbe::open(&server, &other.token).await;

    // Invoke a tool — the detached recorder writes the row and emits the frame.
    let status = reqwest::Client::new()
        .post(server.api_url(&format!("/mcp/servers/{server_id}/tools/echo/call")))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&json!({ "arguments": {} }))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, 200);

    let frame = owner_probe
        .expect_event("mcp_tool_call", "create", EVENT_TIMEOUT)
        .await;
    // SyncFrame.id is the row's UUID as a string; assert it's a real id.
    assert!(
        Uuid::parse_str(&frame.id).is_ok() && frame.id != Uuid::nil().to_string(),
        "frame must carry the new row's id, got {:?}",
        frame.id
    );

    // Owner-scoped: an unrelated user observes nothing.
    other_probe.expect_silence(SILENCE_WINDOW).await;
}

// =====================================================
// file — OWNER audience (workflow tool-step resource_link → persist → publish_file_changed)
// =====================================================

#[tokio::test]
async fn tool_call_resource_link_persists_file_and_emits_sync() {
    // A workflow with a `tool` step whose mock returns a `resource_link
    // is_saved:false` → persist_links → ingest_bytes → publish_file_changed
    // → the owner observes `file`/`update` on their sync stream.
    let server = crate::common::TestServer::start().await;
    let user = crate::workflow::workflow_tool_user(&server, "sync_rl_file_owner").await;
    let (_stub, model_id) = crate::workflow::stub_model_for(&server, &user.user_id).await;

    let other = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "sync_rl_file_other",
        &[],
    )
    .await;

    let mock = MockMcpServer::start().await;
    mock.on_download("result.csv", "text/csv", b"a,b\n1,2\n");
    let dl_url = mock.download_url("result.csv");
    mock.on_method(
        "tools/call",
        MockResponse::JsonOk(json!({
            "content": [
                { "type": "text", "text": "produced data" },
                {
                    "type": "resource_link",
                    "uri": dl_url,
                    "name": "result.csv",
                    "mimeType": "text/csv",
                    "is_saved": false,
                }
            ],
            "isError": false,
        })),
    );
    let (_sid, sname) = crate::workflow::register_mock_as_user_server(
        &server,
        &user.token,
        "sync_rl_mock",
        &mock.base_url(),
    )
    .await;

    // Import a dev workflow with a single tool step that calls the mock.
    let yaml = format!(
        r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs: []
steps:
  - id: call
    kind: tool
    server: {sname}
    tool: produce
    arguments: {{}}
outputs:
  - name: result
    from: "{{{{ call.output }}}}"
    expose: full
"#
    );
    let wf = crate::workflow::import_dev_workflow(&server, &user.token, "sync-rl-file", &yaml).await;
    let wf_id = wf["id"].as_str().unwrap();

    // Open probes BEFORE the run so the file-create sync frame isn't missed.
    let mut owner_probe = SyncProbe::open(&server, &user.token).await;
    let mut other_probe = SyncProbe::open(&server, &other.token).await;

    let run = crate::workflow::run_workflow(
        &server,
        &user.token,
        wf_id,
        json!({ "inputs": {}, "model_id": model_id.to_string() }),
    )
    .await;
    let run_id = uuid::Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();
    let final_run = crate::workflow::poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "resource_link tool step should complete: {final_run}"
    );

    // The owner observes the `file`/`update` sync frame (from publish_file_changed).
    let frame = owner_probe
        .expect_event("file", "update", EVENT_TIMEOUT)
        .await;
    assert!(
        uuid::Uuid::parse_str(&frame.id).is_ok() && frame.id != uuid::Uuid::nil().to_string(),
        "frame must carry the new file's id, got {:?}",
        frame.id
    );

    // Owner-scoped: an unrelated user observes nothing.
    other_probe.expect_silence(SILENCE_WINDOW).await;
}
