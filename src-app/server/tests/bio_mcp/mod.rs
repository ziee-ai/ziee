// ============================================================================
// bio_mcp built-in MCP server tests.
//
// Tests the proxy route at /api/bio/mcp (the "thin wrapper we own"):
//   - Auth gate: no JWT → 401; a JWT without `bio::query` → 403.
//   - Graceful unavailability: an authorized caller, when BioMCP is not
//     configured/enabled (no sidecar), gets a clean 503 — never a 500 /
//     panic. (The default test server leaves `bio_mcp.enabled` false, so
//     the bio row is never upserted → `ensure_healthy` surfaces a clear
//     "row not found" error mapped to 503.)
//
// The full proxy-forward path (real biomcp serve-http sidecar) is covered
// by the Tier-4 real-sidecar test, which is environment-gated.
// ============================================================================

use serde_json::json;

fn jsonrpc_body() -> serde_json::Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} })
}

#[tokio::test]
async fn test_proxy_requires_auth() {
    let server = crate::common::TestServer::start().await;
    // No Authorization header → 401 before the handler runs.
    let res = reqwest::Client::new()
        .post(server.api_url("/bio/mcp"))
        .json(&jsonrpc_body())
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn test_proxy_requires_bio_query_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "bio_noperm",
        &["profile::read"],
    )
    .await;
    // Migration 96 grants `bio::query` to the default Users group, so a freshly
    // registered user has it by default. Revoke it from the Users group to make
    // this user genuinely lack the permission — proving the route is gated on
    // `bio::query` specifically (not merely authenticated). Permission checks
    // are live (re-read from the DB per request), so this takes effect at once.
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    sqlx::query(
        "UPDATE groups SET permissions = array_remove(permissions, 'bio::query') \
         WHERE name = 'Users' AND is_system = true AND is_default = true",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    let res = reqwest::Client::new()
        .post(server.api_url("/bio/mcp"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&jsonrpc_body())
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn test_proxy_unavailable_is_graceful_503() {
    let server = crate::common::TestServer::start().await;
    // Authorized caller, but BioMCP is not enabled in the default test
    // config → no bio row → `ensure_healthy` fails → 503 (NOT 500/panic).
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "bio_authorized",
        &["bio::query"],
    )
    .await;
    let res = reqwest::Client::new()
        .post(server.api_url("/bio/mcp"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&jsonrpc_body())
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 503);
    let body: serde_json::Value = res.json().await.unwrap();
    let err = body.get("error").and_then(|e| e.as_str()).unwrap_or("");
    assert!(
        err.contains("BioMCP") || err.contains("not found") || err.contains("disabled"),
        "503 should carry a bio-specific error message, got: {body}"
    );
}

/// Resolve the build-staged biomcp binary path for the host triple and
/// report whether a REAL (non-stub) binary is present. When the build had
/// no network (or hit an unsupported triple), `build_helper/biomcp.rs`
/// stages a zero-byte stub — there is then genuinely no sidecar to test,
/// so the real-sidecar test below skips (an environment gate, like the
/// rootfs-needed sandbox tiers — NOT an #[ignore] to go green).
fn staged_biomcp_is_real() -> bool {
    let triple = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "aarch64-unknown-linux-gnu"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "x86_64-pc-windows-msvc"
    } else {
        return false;
    };
    let name = if cfg!(windows) { "biomcp.exe" } else { "biomcp" };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join(triple)
        .join("biomcp")
        .join(name);
    std::fs::metadata(&path).map(|m| m.len() > 0).unwrap_or(false)
}

/// Tier-4: full production path — proxy → supervisor → REAL `biomcp
/// serve-http` sidecar → MCP initialize → streamed response. `initialize`
/// is local to biomcp (no upstream API call), so this runs offline once
/// the binary is staged.
#[tokio::test]
async fn test_real_sidecar_proxy_initialize() {
    if !staged_biomcp_is_real() {
        eprintln!(
            "skipping test_real_sidecar_proxy_initialize: biomcp binary not staged \
             for this build (zero-byte stub / unsupported triple)"
        );
        return;
    }

    // Enable bio_mcp in the test server so it upserts the row + the proxy
    // spawns the real sidecar on first call.
    let server = crate::common::TestServer::start_with_options(crate::common::TestServerOptions {
        bio_mcp_enabled: true,
        ..Default::default()
    })
    .await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "bio_real",
        &["bio::query"],
    )
    .await;

    let res = reqwest::Client::new()
        .post(server.api_url("/bio/mcp"))
        .header("Authorization", format!("Bearer {}", user.token))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "itest", "version": "1.0" }
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.status(),
        200,
        "real biomcp sidecar initialize should proxy 200"
    );
    // biomcp answers MCP streamable-HTTP (SSE) and the proxy must surface
    // its session id back to the client.
    assert!(
        res.headers().get("mcp-session-id").is_some(),
        "proxy should forward biomcp's Mcp-Session-Id header"
    );
    let ct = res
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.contains("text/event-stream") || ct.contains("application/json"),
        "expected an MCP streamable-http response, got content-type: {ct}"
    );
}

/// Deterministic id of the built-in bio server (must equal
/// `bio_mcp::bio_mcp_server_id()`).
fn bio_server_id() -> uuid::Uuid {
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, b"bio.ziee.internal")
}

/// Tier-2 DB integration: when enabled, the boot upsert registers bio as an
/// ADMIN-CONFIGURABLE built-in (http, is_built_in, is_system, enabled) whose
/// id is NOT one of the zero-config edit-deny-list ids (so admins can edit its
/// Headers). Gated on the staged binary (the upsert is skipped for a stub).
#[tokio::test]
async fn test_bio_row_registered_as_editable_builtin() {
    if !staged_biomcp_is_real() {
        eprintln!("skipping test_bio_row_registered_as_editable_builtin: biomcp not staged");
        return;
    }
    let server = crate::common::TestServer::start_with_options(crate::common::TestServerOptions {
        bio_mcp_enabled: true,
        ..Default::default()
    })
    .await;

    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let bio_id = bio_server_id();

    // The upsert runs in a spawned task at init — poll briefly for the row.
    let mut row: Option<(String, String, bool, bool, bool)> = None;
    for _ in 0..50 {
        row = sqlx::query_as(
            "SELECT name, transport_type, enabled, is_built_in, is_system \
             FROM mcp_servers WHERE id = $1",
        )
        .bind(bio_id)
        .fetch_optional(&pool)
        .await
        .unwrap();
        if row.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    let (name, transport, enabled, is_built_in, is_system) =
        row.expect("bio row should be upserted when bio_mcp is enabled + binary staged");
    assert_eq!(name, "bio");
    assert_eq!(transport, "http");
    assert!(enabled, "bio row should default enabled");
    assert!(is_built_in, "bio is a built-in server");
    assert!(is_system, "bio is a system server");

    // Editability: bio must NOT collide with the zero-config edit-deny-list
    // ids (files/memory/elicitation) — that's what keeps its Headers editable.
    let files = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, b"files.ziee.internal");
    let memory = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, b"memory.ziee.internal");
    let elicit = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, b"elicitation.ziee.internal");
    assert_ne!(bio_id, files);
    assert_ne!(bio_id, memory);
    assert_ne!(bio_id, elicit);
}

/// Tier-5 real-LLM smoke test: a tool-capable Anthropic model, instructed to
/// use the biomcp tool, actually calls it end-to-end — LLM → `/api/bio/mcp`
/// proxy → real `biomcp` sidecar → result → LLM. Proves the full production
/// path that the lower tiers exercise piecewise.
///
/// Gated on `ANTHROPIC_API_KEY` (from `tests/.env.test`) + the staged binary.
/// Costs real LLM tokens and hits live upstream APIs (PubMed), so it is
/// `#[ignore]`d — verified green, but not run on every `cargo test`. Run via:
///   source tests/.env.test && cargo test --test integration_tests \
///     bio_mcp::test_bio_mcp_real_llm -- --ignored --test-threads=1 --nocapture
#[tokio::test]
#[ignore = "real LLM tokens + live PubMed egress; verified-green, run explicitly with --ignored after sourcing tests/.env.test"]
async fn test_bio_mcp_real_llm_tool_call() {
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("skipping test_bio_mcp_real_llm_tool_call: ANTHROPIC_API_KEY unset");
        return;
    }
    if !staged_biomcp_is_real() {
        eprintln!("skipping test_bio_mcp_real_llm_tool_call: biomcp binary not staged");
        return;
    }

    let server = crate::common::TestServer::start_with_options(crate::common::TestServerOptions {
        bio_mcp_enabled: true,
        ..Default::default()
    })
    .await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "bio_llm",
        &[
            "conversations::create",
            "conversations::read",
            "conversations::edit",
            "messages::create",
            "messages::read",
            "llm_models::read",
            "bio::query",
        ],
    )
    .await;

    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let bio_id = bio_server_id();

    // Wait for the boot upsert (spawned task) to land the bio row, then make it
    // group-accessible so the explicit `mcp_config` request resolves it
    // (mirrors how the mcp streaming test grants its server).
    for _ in 0..50 {
        let exists: Option<(uuid::Uuid,)> =
            sqlx::query_as("SELECT id FROM mcp_servers WHERE id = $1")
                .bind(bio_id)
                .fetch_optional(&pool)
                .await
                .unwrap();
        if exists.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let default_group: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM groups WHERE is_default = true LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "INSERT INTO user_group_mcp_servers (group_id, mcp_server_id, assigned_at) \
         VALUES ($1, $2, NOW()) ON CONFLICT DO NOTHING",
    )
    .bind(default_group)
    .bind(bio_id)
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    // Real tool-capable Anthropic model (the key is present per the gate above).
    let model = crate::chat::helpers::get_or_create_test_model(&server, &user.user_id).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);

    let conversation =
        crate::chat::helpers::create_conversation(&server, &user.token, Some(model_id), None).await;
    let conversation_id = crate::chat::helpers::parse_uuid(&conversation["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conversation["active_branch_id"]);

    let payload = json!({
        "content": "Use the biomcp tool to search PubMed for recent articles about CRISPR gene \
                    editing. You MUST call the available biomcp tool — do not answer from memory.",
        "model_id": model_id.to_string(),
        "branch_id": branch_id.to_string(),
        "enable_mcp": true,
        "mcp_config": { "mcp_servers": [ { "server_id": bio_id.to_string(), "tools": [] } ] }
    });

    let events = crate::chat::helpers::send_body_and_collect_events(
        &server,
        &user.token,
        conversation_id,
        payload,
        &["complete"],
    )
    .await;

    let tool_start = events.iter().filter(|e| e.event == "mcpToolStart").count();
    let tool_complete = events
        .iter()
        .filter(|e| e.event == "mcpToolComplete")
        .count();
    eprintln!(
        "bio real-LLM: {} events, mcpToolStart={}, mcpToolComplete={}",
        events.len(),
        tool_start,
        tool_complete
    );
    assert!(
        tool_start > 0,
        "the model should have called the biomcp tool (no mcpToolStart event)"
    );
    assert!(
        tool_complete > 0,
        "the biomcp tool call should have completed (no mcpToolComplete event)"
    );
}
