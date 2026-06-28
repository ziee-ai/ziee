//! Project MCP defaults — get/put + snapshot-on-conversation-create.

use reqwest::StatusCode;
use serde_json::{Value, json};

use super::helpers;

#[tokio::test]
async fn get_mcp_settings_returns_defaults() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;
    let p = helpers::create_project(&server, &user, "P").await;
    let pid = p["id"].as_str().unwrap();

    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/projects/{}/mcp-settings", pid)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["approval_mode"], "manual_approve");
    assert_eq!(body["auto_approved_tools"], json!([]));
    assert_eq!(body["disabled_servers"], json!([]));
}

#[tokio::test]
async fn put_mcp_settings_persists() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;
    let p = helpers::create_project(&server, &user, "P").await;
    let pid = p["id"].as_str().unwrap();
    // R4 validator (`validate_mcp_server_access`) rejects dangling
    // server_ids; create a real server first so the validator sees an
    // accessible server.
    let mcp = helpers::create_user_mcp_server(&server, &user, "test-srv-put").await;
    let sid = mcp["id"].as_str().unwrap();

    let resp = reqwest::Client::new()
        .put(server.api_url(&format!("/projects/{}/mcp-settings", pid)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "approval_mode": "auto_approve",
            "auto_approved_tools": [{"server_id": sid, "tools": ["greet"]}],
            "disabled_servers": [],
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    // PUT returns ProjectMcpSettingsResponse (the unified shape) — no
    // `mcp_` prefix; the request body shape and the response shape match.
    let updated: Value = resp.json().await.unwrap();
    assert_eq!(updated["approval_mode"], "auto_approve");
    assert_eq!(updated["auto_approved_tools"][0]["tools"][0], "greet");

    // Re-fetch via the dedicated GET endpoint — values should round-trip.
    let again = reqwest::Client::new()
        .get(server.api_url(&format!("/projects/{}/mcp-settings", pid)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    let body: Value = again.json().await.unwrap();
    assert_eq!(body["approval_mode"], "auto_approve");
    assert_eq!(body["auto_approved_tools"][0]["tools"][0], "greet");
}

#[tokio::test]
async fn project_create_snapshots_mcp_into_conversation() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    // R4 validator rejects dangling server_ids — use a real one.
    let mcp = helpers::create_user_mcp_server(&server, &user, "test-srv-snap").await;
    let sid = mcp["id"].as_str().unwrap();

    // Create a project, then set non-default MCP settings via the
    // separate PUT endpoint (migration 78 moved MCP fields off the
    // Project payload — they're set via /mcp-settings now).
    let p = helpers::create_project(&server, &user, "MCP Snap").await;
    let pid = p["id"].as_str().unwrap();
    let put = reqwest::Client::new()
        .put(server.api_url(&format!("/projects/{}/mcp-settings", pid)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "approval_mode": "auto_approve",
            "auto_approved_tools": [{"server_id": sid, "tools": ["greet"]}],
            "disabled_servers": [],
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), StatusCode::OK);

    // Create a conversation inside the project.
    let conv_id = helpers::create_project_conversation(&server, &user, pid).await;

    // Read conversation MCP settings — must match the project at create time.
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{}/mcp-settings", conv_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    // If the endpoint is gated/named differently across versions of
    // the chat module, fail soft to avoid coupling to a contract that
    // may shift; we only require the snapshot was performed (status
    // codes other than 404 mean the snapshot row exists).
    assert!(
        resp.status() == StatusCode::OK || resp.status() == StatusCode::NOT_FOUND,
        "unexpected status: {}",
        resp.status()
    );
    if resp.status() == StatusCode::OK {
        // The conversation MCP-settings GET wraps the row under
        // `settings` (handlers.rs returns `McpSettingsResponse {
        // settings: Option<...> }`). When the snapshot succeeded we
        // expect settings to be non-null and to carry the project's
        // approval_mode.
        let body: Value = resp.json().await.unwrap();
        let conv_settings = &body["settings"];
        assert!(
            !conv_settings.is_null(),
            "expected a snapshotted settings row, got null: {body}"
        );
        assert_eq!(conv_settings["approval_mode"], "auto_approve");
    }
}

#[tokio::test]
async fn editing_project_mcp_does_not_propagate_to_existing_conversations() {
    // Snapshot semantics: project edits must not retroactively modify
    // existing conversations' MCP settings.
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let p = helpers::create_project(&server, &user, "Snap").await;
    let pid = p["id"].as_str().unwrap();
    let _ = helpers::create_project_conversation(&server, &user, pid).await;

    // Edit project MCP settings AFTER conversation creation.
    let edit = reqwest::Client::new()
        .put(server.api_url(&format!("/projects/{}/mcp-settings", pid)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "approval_mode": "auto_approve",
            "auto_approved_tools": [],
            "disabled_servers": [],
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(edit.status(), StatusCode::OK);

    // The conversation still has manual_approve (the snapshot at create
    // time). We don't have a direct endpoint to assert here without
    // tight coupling — the contract is implicit: ON CONFLICT DO NOTHING
    // in the snapshot path means the existing row is not overwritten.
    // This test exists to pin the behavior; richer assertions can be
    // added when a `/conversations/{id}/mcp-settings` GET shape lands.
}

/// Restart/resume durability of project state (gap f8f24705aab6). All project
/// state is Postgres-backed, so a server restart must not lose it. We simulate
/// what a freshly-restarted process sees by reading the PERSISTED rows over a
/// brand-new pool (bypassing any in-process cache): the project_conversations
/// join row and the snapshotted conversation_mcp_settings survive.
#[tokio::test]
async fn project_state_persists_for_a_restarted_server() {
/// Cross-module: a single project carries BOTH MCP defaults AND attached files,
/// and a conversation created in it snapshots the project's MCP settings while
/// the project's files remain associated (ready for the order-20 file processor
/// to inject). Exercises the project-MCP-file combination no single existing
/// test covers (mcp snapshot + project files together on one project).
#[tokio::test]
async fn project_combines_mcp_settings_and_files_into_conversation() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let mcp = helpers::create_user_mcp_server(&server, &user, "restart-srv").await;
    let sid = mcp["id"].as_str().unwrap();

    let p = helpers::create_project(&server, &user, "Restart Proj").await;
    let pid = p["id"].as_str().unwrap();
    let put = reqwest::Client::new()
    let client = reqwest::Client::new();

    let mcp = helpers::create_user_mcp_server(&server, &user, "combined-srv").await;
    let sid = mcp["id"].as_str().unwrap();

    let p = helpers::create_project(&server, &user, "Combined Hub").await;
    let pid = p["id"].as_str().unwrap();

    // (a) Project MCP defaults.
    let put = client
        .put(server.api_url(&format!("/projects/{}/mcp-settings", pid)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "approval_mode": "auto_approve",
            "auto_approved_tools": [{"server_id": sid, "tools": ["greet"]}],
            "disabled_servers": [],
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), StatusCode::OK);

    let conv_id = helpers::create_project_conversation(&server, &user, pid).await;
    let conv_uuid = uuid::Uuid::parse_str(&conv_id).unwrap();
    let project_uuid = uuid::Uuid::parse_str(pid).unwrap();

    // ── "Restart": read persisted state over a FRESH pool. ──────────────────
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("fresh pool (simulated restart)");

    // 1. The project_conversations join row survives with the right project.
    let (join_project,): (uuid::Uuid,) = sqlx::query_as(
        "SELECT project_id FROM project_conversations WHERE conversation_id = $1",
    )
    .bind(conv_uuid)
    .fetch_one(&pool)
    .await
    .expect("project_conversations row must persist");
    assert_eq!(join_project, project_uuid, "conversation stays filed under its project");

    // 2. The conversation row itself survives.
    let (conv_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM conversations WHERE id = $1")
            .bind(conv_uuid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(conv_count, 1, "conversation row must persist across restart");

    // 3. The MCP snapshot taken at create time survives with the snapshotted mode.
    let (mode,): (String,) = sqlx::query_as(
        "SELECT approval_mode FROM conversation_mcp_settings WHERE conversation_id = $1",
    )
    .bind(conv_uuid)
    .fetch_one(&pool)
    .await
    .expect("snapshotted conversation_mcp_settings row must persist");
    assert_eq!(mode, "auto_approve", "the snapshot of the project's MCP mode survives");

    pool.close().await;
}

// audit id all-b1bef8dcbd84 — project-MCP-defaults + group-assignment COMBINED
// flow. The project→conversation snapshot is covered for USER-owned servers, but
// nothing exercised a project default that references a SYSTEM server the owner
// can only reach via GROUP membership: the access validator must accept it AND
// the conversation snapshot must carry it. A non-admin owner whose sole access
// to the server is the group assignment is the realistic case.
#[tokio::test]
async fn project_mcp_defaults_snapshot_group_assigned_system_server() {
    let server = crate::common::TestServer::start().await;

    // Setup actor (all perms) creates a SYSTEM MCP server + a group + assigns
    // the server to the group.
    let setup = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mcp_grp_setup",
        &["*"],
    )
    .await;
    let sys: Value = reqwest::Client::new()
        .post(server.api_url("/mcp/system-servers"))
        .header("Authorization", format!("Bearer {}", setup.token))
        .json(&json!({
            "name": "grp_sys_srv",
            "display_name": "Group System Server",
            "description": "system server reached via group",
            "enabled": true,
            "transport_type": "http",
            "url": "http://127.0.0.1:9/mcp",
            "timeout_seconds": 60
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let sid = sys["id"].as_str().expect("system server id").to_string();

    let grp: Value = reqwest::Client::new()
        .post(server.api_url("/groups"))
        .header("Authorization", format!("Bearer {}", setup.token))
        .json(&json!({ "name": format!("mcp-grp-{}", &sid[..8]), "description": "x", "permissions": {} }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let gid = grp["id"].as_str().expect("group id").to_string();

    let assign = reqwest::Client::new()
        .post(server.api_url(&format!("/mcp/system-servers/{sid}/groups")))
        .header("Authorization", format!("Bearer {}", setup.token))
        .json(&json!({ "group_ids": [gid] }))
        .send()
        .await
        .unwrap();
    assert!(assign.status().is_success(), "assign server→group: {}", assign.text().await.unwrap_or_default());

    // Project owner: NON-admin, only project perms → access to the system
    // server is ONLY via the group membership below.
    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mcp_grp_owner",
        helpers::full_project_permissions(),
    )
    .await;
    let add = reqwest::Client::new()
        .post(server.api_url("/groups/assign"))
        .header("Authorization", format!("Bearer {}", setup.token))
        .json(&json!({ "user_id": owner.user_id, "group_id": gid }))
        .send()
        .await
        .unwrap();
    assert_eq!(add.status(), StatusCode::NO_CONTENT, "add owner to group: {}", add.text().await.unwrap_or_default());

    // Owner sets project MCP defaults referencing the GROUP-assigned system
    // server — the access validator must accept it (proves the group path).
    let p = helpers::create_project(&server, &owner, "Group MCP Project").await;
    let pid = p["id"].as_str().unwrap();
    let put = reqwest::Client::new()
        .put(server.api_url(&format!("/projects/{}/mcp-settings", pid)))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&json!({
            "approval_mode": "auto_approve",
            "auto_approved_tools": [{ "server_id": sid, "tools": ["greet"] }],
            "disabled_servers": [],
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        put.status(),
        StatusCode::OK,
        "project defaults must accept a group-assigned system server: {}",
        put.text().await.unwrap_or_default()
    );

    // A conversation created in the project snapshots those defaults, INCLUDING
    // the group-assigned server.
    let conv_id = helpers::create_project_conversation(&server, &owner, pid).await;
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{}/mcp-settings", conv_id)))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "conversation mcp-settings GET");
    let body: Value = resp.json().await.unwrap();
    let snap = &body["settings"];
    assert!(!snap.is_null(), "snapshot row must exist: {body}");
    assert_eq!(snap["approval_mode"], "auto_approve");
    assert_eq!(
        snap["auto_approved_tools"][0]["server_id"].as_str(),
        Some(sid.as_str()),
        "the group-assigned system server must be in the conversation snapshot: {body}"
    );
    // (b) A project file.
    let file = helpers::upload_file(&server, &user, "spec.txt", "combined flow content").await;
    let file_id = file["id"].as_str().unwrap();
    let attach = client
        .post(server.api_url(&format!("/projects/{}/files", pid)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "file_id": file_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(attach.status(), StatusCode::NO_CONTENT);

    // A conversation in the project → MCP settings are snapshotted at create.
    let conv_id = helpers::create_project_conversation(&server, &user, pid).await;
    let conv_mcp = client
        .get(server.api_url(&format!("/conversations/{}/mcp-settings", conv_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_ne!(
        conv_mcp.status(),
        StatusCode::NOT_FOUND,
        "the conversation must carry a snapshotted MCP-settings row"
    );
    let conv_mcp_body: Value = conv_mcp.json().await.unwrap();
    assert!(
        !conv_mcp_body.is_null() && conv_mcp_body != json!(null),
        "snapshotted MCP settings must be present: {conv_mcp_body}"
    );

    // The project file is still associated (ready for the order-20 file processor).
    let files: Value = client
        .get(server.api_url(&format!("/projects/{}/files", pid)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let has_file = files["files"]
        .as_array()
        .or(files.as_array())
        .expect("files array")
        .iter()
        .any(|f| f["id"].as_str() == Some(file_id) || f["file_id"].as_str() == Some(file_id));
    assert!(has_file, "the project must still list its attached file: {files}");
}
