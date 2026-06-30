//! Tier-2 integration tests for the desktop-only MCP auto-assign path.
//!
//! Two covered paths:
//!   1. **Boot backfill** (`backfill_system_mcp_assignments`) — the
//!      *visible* system MCP servers land in every group's
//!      `user_group_mcp_servers` row when the desktop server boots.
//!      Built-ins configured elsewhere (memory / files / elicitation /
//!      …) are deliberately EXCLUDED from that list and instead reach
//!      tool-capable chats via the chat-extension auto-attach path
//!      (`auto_attach_builtin_ids`), NOT via group assignment — so the
//!      test verifies the memory built-in registers at boot yet is
//!      never group-assigned.
//!   2. **Per-event handler** (`Desktop::AutoAssignMcpServer`) — a
//!      newly POSTed system MCP server gets auto-assigned to every
//!      group that exists at the moment of creation.
//!
//! Both paths rely on `Repos.mcp.assign_to_group` being idempotent
//! (`ON CONFLICT DO NOTHING`).
//!
//! Reads go through a direct sqlx pool against `server.database_url`
//! (NOT the HTTP API) because:
//!   - The event handler is fire-and-forget (`EventBus::emit_async`
//!     spawns a tokio task), so the POST returns before the handler's
//!     INSERTs complete. The test polls the join table until it sees
//!     the expected rows or times out.
//!   - The HTTP `GET /api/mcp/system-servers/{id}/groups` reads the
//!     same row set; using sqlx directly avoids re-litigating any
//!     handler-side filtering and isolates the test to the data
//!     contract.
//!
//! Runs against `ziee-desktop --headless` (same binary the production
//! Tauri shell spawns) so the AutoAssign handler + backfill are wired
//! up exactly as they are at runtime.

use serde_json::{json, Value};
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;
use uuid::Uuid;

const ADMIN_PERMS: &[&str] = &[
    // Create a system MCP server (POST /api/mcp/system-servers)
    "mcp_servers_admin::create",
];

/// Connect to the per-test database. Reused by both tests.
async fn test_pool(database_url: &str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .connect(database_url)
        .await
        .expect("connect to test db")
}

/// All group ids in the `groups` table (raw read, no auth).
async fn all_group_ids(pool: &PgPool) -> Vec<Uuid> {
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM groups ORDER BY name")
        .fetch_all(pool)
        .await
        .expect("SELECT groups")
}

/// The group_ids the given system MCP server is currently assigned to.
async fn assigned_group_ids(pool: &PgPool, server_id: Uuid) -> Vec<Uuid> {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT group_id FROM user_group_mcp_servers WHERE mcp_server_id = $1",
    )
    .bind(server_id)
    .fetch_all(pool)
    .await
    .expect("SELECT user_group_mcp_servers")
}

/// Find the built-in memory MCP server's id by name substring.
async fn find_memory_mcp_id(pool: &PgPool) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM mcp_servers WHERE is_system = TRUE AND name ILIKE '%memory%' LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .expect("built-in memory MCP server must be registered at desktop boot")
}

/// Whether the given MCP server row is flagged as a built-in.
async fn is_builtin(pool: &PgPool, server_id: Uuid) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT is_built_in FROM mcp_servers WHERE id = $1")
        .bind(server_id)
        .fetch_one(pool)
        .await
        .expect("SELECT is_built_in")
}

/// Poll `assigned_group_ids` until every expected group_id appears,
/// or `timeout` elapses. Returns the final assignment set so the
/// caller can produce a clear error message on timeout.
async fn poll_for_assignments(
    pool: &PgPool,
    server_id: Uuid,
    expected: &[Uuid],
    timeout: Duration,
) -> Vec<Uuid> {
    let start = std::time::Instant::now();
    loop {
        let assigned = assigned_group_ids(pool, server_id).await;
        let all_present = expected.iter().all(|e| assigned.contains(e));
        if all_present {
            return assigned;
        }
        if start.elapsed() >= timeout {
            return assigned;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn builtin_memory_mcp_is_registered_but_not_group_assigned() {
    // Memory is ON by default (memory_admin_settings.enabled defaults
    // TRUE), so the built-in memory MCP server registers at boot. But
    // built-ins (memory / files / elicitation / web_search / lit_search /
    // tool_result / citations) deliberately reach tool-capable chats via
    // the chat-extension AUTO-ATTACH path
    // (mcp::chat_extension::auto_attach_builtin_ids + approval-bypass via
    // is_builtin_server_id), NOT via group assignment. They are excluded
    // from list_system_mcp_servers ("hide the built-ins configured
    // elsewhere"), so the desktop boot backfill — which assigns the
    // *visible* system servers to every group — must NOT assign memory.
    let server = crate::common::TestServer::start_desktop().await;
    let pool = test_pool(&server.database_url).await;

    let boot_groups = all_group_ids(&pool).await;
    assert!(
        !boot_groups.is_empty(),
        "expected ≥1 group seeded by migrations at boot"
    );

    // The memory built-in registered at boot (memory-on-by-default).
    let memory_id = find_memory_mcp_id(&pool).await;
    assert!(
        is_builtin(&pool, memory_id).await,
        "memory MCP server {memory_id} should be flagged is_built_in"
    );

    // ...and is NOT group-assigned: auto-attach is its delivery path.
    let assigned = assigned_group_ids(&pool, memory_id).await;
    assert!(
        assigned.is_empty(),
        "built-in memory MCP {memory_id} must NOT be group-assigned \
         (built-ins auto-attach to tool-capable chats via \
         auto_attach_builtin_ids, not via group assignment); assigned={assigned:?}",
    );

    pool.close().await;
}

#[tokio::test]
async fn auto_assign_handler_attaches_new_system_server_to_every_group() {
    let server = crate::common::TestServer::start_desktop().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mcp_new_server_admin",
        ADMIN_PERMS,
    )
    .await;

    let pool = test_pool(&server.database_url).await;
    // Snapshot AFTER user creation — this is the set of groups the
    // handler should see when it fires for the POST below.
    let expected_groups = all_group_ids(&pool).await;
    assert!(
        !expected_groups.is_empty(),
        "expected ≥1 group to assign to"
    );

    // Create a fresh system MCP server. http transport so we don't
    // need a sandbox rootfs to land the row — the auto-assign path
    // fires on `SystemServerCreated` regardless of transport.
    let create_body = json!({
        "name": "test-auto-assign-http",
        "display_name": "Auto-Assign Test (HTTP)",
        "description": "Integration-test fixture; verifies desktop AutoAssignMcpServerHandler.",
        "enabled": false,
        "transport_type": "http",
        "url": "http://127.0.0.1:1/never-reached",
    });
    let res = reqwest::Client::new()
        .post(server.api_url("/mcp/system-servers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&create_body)
        .send()
        .await
        .expect("POST /api/mcp/system-servers failed");
    let status = res.status();
    let body_text = res.text().await.unwrap_or_default();
    assert!(
        status.is_success(),
        "create system MCP server failed: {} — {}",
        status,
        body_text
    );

    let body: Value =
        serde_json::from_str(&body_text).expect("create-response JSON parse");
    let new_id: Uuid = body
        .get("id")
        .and_then(|v| v.as_str())
        .expect("created server.id string")
        .parse()
        .expect("new server id parses");

    // The handler runs in a spawned task off `EventBus::emit_async`,
    // so the POST returns before the INSERTs commit. Poll the join
    // table until every expected assignment appears — 2s is generous
    // for a 3-row insert that the handler completes in <10ms.
    let assigned = poll_for_assignments(
        &pool,
        new_id,
        &expected_groups,
        Duration::from_secs(2),
    )
    .await;

    for group_id in &expected_groups {
        assert!(
            assigned.contains(group_id),
            "AutoAssignMcpServerHandler should have assigned new server \
             {} to group {} within 2s; assigned={:?}",
            new_id,
            group_id,
            assigned
        );
    }

    pool.close().await;
}
