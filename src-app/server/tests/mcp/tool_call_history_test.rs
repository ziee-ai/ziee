//! Integration coverage for the MCP tool-call history (`mcp_tool_calls`).
//!
//! Drives the deterministic recording path: register the in-process
//! `MockMcpServer` as a user HTTP MCP server, call a tool through the REST
//! endpoint `POST /api/mcp/servers/{id}/tools/{name}/call` (the same
//! `McpSession::call_tool` chokepoint every path uses), then assert the row
//! that the fire-and-forget recorder wrote. No LLM needed.

use std::time::Duration;

use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use super::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};

/// Register `mock` as a user-owned HTTP MCP server, returning the new id.
async fn register_mock_server(
    server: &crate::common::TestServer,
    token: &str,
    name: &str,
    url: &str,
) -> String {
    let res = reqwest::Client::new()
        .post(server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": name,
            "display_name": "Tool-call mock",
            "transport_type": "http",
            "url": url,
            "enabled": true,
        }))
        .send()
        .await
        .unwrap();
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    assert_eq!(status, 201, "register mock server: {status}: {body}");
    let row: serde_json::Value = serde_json::from_str(&body).unwrap();
    row["id"].as_str().unwrap().to_string()
}

/// Invoke a tool on `server_id` via REST, returning the HTTP status.
async fn call_tool(
    server: &crate::common::TestServer,
    token: &str,
    server_id: &str,
    tool: &str,
    arguments: serde_json::Value,
) -> reqwest::StatusCode {
    reqwest::Client::new()
        .post(server.api_url(&format!("/mcp/servers/{server_id}/tools/{tool}/call")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "arguments": arguments }))
        .send()
        .await
        .unwrap()
        .status()
}

/// Open a pool on the per-test DB.
async fn pool(server: &crate::common::TestServer) -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap()
}

/// Poll for the most recent recorded tool-call row for `user_id` (the insert is
/// fire-and-forget, so it may land a beat after the HTTP response).
async fn wait_for_latest_row(pool: &sqlx::PgPool, user_id: Uuid) -> sqlx::postgres::PgRow {
    for _ in 0..40 {
        let row = sqlx::query(
            "SELECT tool_name, server_name, is_built_in, source, status, is_error,
                    arguments_json, result_json, content_kinds, error_message, duration_ms
             FROM mcp_tool_calls WHERE user_id = $1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .unwrap();
        if let Some(r) = row {
            return r;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("no mcp_tool_calls row recorded for user {user_id} within timeout");
}

async fn count_rows(pool: &sqlx::PgPool, user_id: Uuid) -> i64 {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM mcp_tool_calls WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn tool_call_via_rest_records_a_completed_row() {
    let server = crate::common::TestServer::start().await;
    let mock = MockMcpServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tc_records",
        &["mcp_servers::create", "mcp_servers::read"],
    )
    .await;
    let uid = Uuid::parse_str(&user.user_id).unwrap();

    mock.on_method(
        "tools/call",
        MockResponse::JsonOk(json!({
            "content": [{ "type": "text", "text": "hello world" }],
            "isError": false,
        })),
    );
    let id = register_mock_server(&server, &user.token, "tc_mock_ok", &mock.base_url()).await;

    let status = call_tool(
        &server,
        &user.token,
        &id,
        "echo",
        json!({ "msg": "hi", "n": 7 }),
    )
    .await;
    assert_eq!(status, 200, "tool call should return 200");

    // The mock saw exactly one tools/call with the args we sent (typed).
    let tool_calls: Vec<_> = mock
        .received()
        .into_iter()
        .filter(|r| r.method == "tools/call")
        .collect();
    assert_eq!(tool_calls.len(), 1, "mock should see exactly one tools/call");
    assert_eq!(tool_calls[0].body["params"]["arguments"]["n"], json!(7));

    let pool = pool(&server).await;
    // Exactly one row (no double-recording).
    let row = wait_for_latest_row(&pool, uid).await;
    assert_eq!(count_rows(&pool, uid).await, 1, "exactly one row recorded");

    assert_eq!(row.get::<String, _>("tool_name"), "echo");
    assert_eq!(row.get::<String, _>("source"), "rest");
    assert_eq!(row.get::<String, _>("status"), "completed");
    assert!(!row.get::<bool, _>("is_error"));
    assert!(!row.get::<bool, _>("is_built_in"));
    assert_eq!(row.get::<String, _>("server_name"), "tc_mock_ok");
    let args: serde_json::Value = row.get("arguments_json");
    assert_eq!(args["n"], json!(7), "stored arguments preserve the number type");
    assert!(row.get::<Option<i64>, _>("duration_ms").is_some());

    // Full result JSON stored + content kinds extracted.
    let result_json: Option<serde_json::Value> = row.get("result_json");
    assert!(result_json.is_some(), "result_json stored");
    assert_eq!(
        result_json.unwrap()["content"][0]["text"],
        json!("hello world"),
        "result_json carries the tool's text content"
    );
    let kinds: Vec<String> = row.get("content_kinds");
    assert!(kinds.contains(&"text".to_string()), "content_kinds includes text");

    pool.close().await;
}

#[tokio::test]
async fn tool_call_error_records_failed_row() {
    let server = crate::common::TestServer::start().await;
    let mock = MockMcpServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tc_error",
        &["mcp_servers::create", "mcp_servers::read"],
    )
    .await;
    let uid = Uuid::parse_str(&user.user_id).unwrap();

    mock.on_method(
        "tools/call",
        MockResponse::JsonOk(json!({
            "content": [{ "type": "text", "text": "tool blew up" }],
            "isError": true,
        })),
    );
    let id = register_mock_server(&server, &user.token, "tc_mock_err", &mock.base_url()).await;

    call_tool(&server, &user.token, &id, "boom", json!({})).await;

    let pool = pool(&server).await;
    let row = wait_for_latest_row(&pool, uid).await;
    assert_eq!(row.get::<String, _>("status"), "failed");
    assert!(row.get::<bool, _>("is_error"));
    pool.close().await;
}

#[tokio::test]
async fn tool_call_transport_error_records_failed_with_message() {
    let server = crate::common::TestServer::start().await;
    let mock = MockMcpServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tc_transport_err",
        &["mcp_servers::create", "mcp_servers::read"],
    )
    .await;
    let uid = Uuid::parse_str(&user.user_id).unwrap();

    // A JSON-RPC protocol error (not a tool-level isError) surfaces as an
    // Err(AppError) from call_tool → classified to a terminal status.
    mock.on_method(
        "tools/call",
        MockResponse::JsonRpcError {
            code: -32603,
            message: "internal mock failure".to_string(),
        },
    );
    let id = register_mock_server(&server, &user.token, "tc_mock_rpc_err", &mock.base_url()).await;

    call_tool(&server, &user.token, &id, "boom", json!({})).await;

    let pool = pool(&server).await;
    let row = wait_for_latest_row(&pool, uid).await;
    assert_eq!(row.get::<String, _>("status"), "failed");
    assert!(row.get::<bool, _>("is_error"));
    assert!(
        row.get::<Option<String>, _>("error_message")
            .is_some_and(|m| !m.is_empty()),
        "transport error message is recorded"
    );
    pool.close().await;
}

#[tokio::test]
async fn tool_call_strips_inline_bytes_in_stored_result() {
    let server = crate::common::TestServer::start().await;
    let mock = MockMcpServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tc_strip",
        &["mcp_servers::create", "mcp_servers::read"],
    )
    .await;
    let uid = Uuid::parse_str(&user.user_id).unwrap();

    mock.on_method(
        "tools/call",
        MockResponse::JsonOk(json!({
            "content": [
                { "type": "text", "text": "see image" },
                { "type": "image", "mimeType": "image/png", "data": "QUJDREVGR0g=" }
            ],
            "isError": false
        })),
    );
    let id = register_mock_server(&server, &user.token, "tc_strip_mock", &mock.base_url()).await;
    call_tool(&server, &user.token, &id, "render", json!({})).await;

    let pool = pool(&server).await;
    let row = wait_for_latest_row(&pool, uid).await;
    let result: serde_json::Value = row
        .get::<Option<serde_json::Value>, _>("result_json")
        .expect("result_json present");
    // The base64 bytes on the image content block are stripped to a reference.
    assert_eq!(result["content"][1]["data"]["_stripped"], json!(true));
    // The text block is preserved verbatim.
    assert_eq!(result["content"][0]["text"], json!("see image"));
    let kinds: Vec<String> = row.get("content_kinds");
    assert!(kinds.contains(&"image".to_string()) && kinds.contains(&"text".to_string()));
    pool.close().await;
}

#[tokio::test]
async fn history_is_owner_scoped_and_cross_user_get_is_404() {
    let server = crate::common::TestServer::start().await;
    let mock = MockMcpServer::start().await;
    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tc_owner",
        &["mcp_servers::create", "mcp_servers::read"],
    )
    .await;
    let other = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tc_other",
        &["mcp_servers::read"],
    )
    .await;
    let owner_uid = Uuid::parse_str(&owner.user_id).unwrap();

    mock.on_method(
        "tools/call",
        MockResponse::JsonOk(json!({ "content": [{ "type": "text", "text": "ok" }] })),
    );
    let id = register_mock_server(&server, &owner.token, "tc_scope_mock", &mock.base_url()).await;
    call_tool(&server, &owner.token, &id, "echo", json!({})).await;

    let pool = pool(&server).await;
    wait_for_latest_row(&pool, owner_uid).await;
    let row_id: String = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM mcp_tool_calls WHERE user_id = $1 LIMIT 1",
    )
    .bind(owner_uid)
    .fetch_one(&pool)
    .await
    .unwrap()
    .to_string();
    pool.close().await;

    // Owner sees their own row in the list.
    let list = reqwest::Client::new()
        .get(server.api_url("/mcp/tool-calls"))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();
    assert_eq!(list.status(), 200);
    let body: serde_json::Value = list.json().await.unwrap();
    assert!(body["total"].as_i64().unwrap() >= 1);

    // A different user's list is empty (owner-scoped).
    let other_list = reqwest::Client::new()
        .get(server.api_url("/mcp/tool-calls"))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .unwrap();
    let other_body: serde_json::Value = other_list.json().await.unwrap();
    assert_eq!(other_body["total"].as_i64().unwrap(), 0);

    // And fetching the owner's row by id as the other user → 404 (MCP convention).
    let cross = reqwest::Client::new()
        .get(server.api_url(&format!("/mcp/tool-calls/{row_id}")))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .unwrap();
    assert_eq!(cross.status(), 404, "cross-user single-row read must be 404");
}

#[tokio::test]
async fn listing_requires_mcp_servers_read() {
    let server = crate::common::TestServer::start().await;
    // No mcp_servers::read — default group stripped so nothing smuggles it in.
    let nobody = crate::common::test_helpers::create_user_with_only_permissions(
        &server,
        "tc_no_read",
        &["profile::read"],
    )
    .await;

    let res = reqwest::Client::new()
        .get(server.api_url("/mcp/tool-calls"))
        .header("Authorization", format!("Bearer {}", nobody.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403, "listing without mcp_servers::read must be 403");
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["error_code"], "INSUFFICIENT_PERMISSIONS");
}

#[tokio::test]
async fn list_pagination_and_server_filter() {
    let server = crate::common::TestServer::start().await;
    let mock = MockMcpServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tc_page",
        &["mcp_servers::create", "mcp_servers::read"],
    )
    .await;
    let uid = Uuid::parse_str(&user.user_id).unwrap();

    // Two canned tool results (one per call).
    for _ in 0..2 {
        mock.on_method(
            "tools/call",
            MockResponse::JsonOk(json!({ "content": [{ "type": "text", "text": "ok" }] })),
        );
    }
    // Same mock, registered as two distinct servers, so we can test the filter.
    let id_a = register_mock_server(&server, &user.token, "tc_page_a", &mock.base_url()).await;
    let id_b = register_mock_server(&server, &user.token, "tc_page_b", &mock.base_url()).await;
    call_tool(&server, &user.token, &id_a, "echo", json!({})).await;
    call_tool(&server, &user.token, &id_b, "echo", json!({})).await;

    let pool = pool(&server).await;
    for _ in 0..40 {
        if count_rows(&pool, uid).await >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(count_rows(&pool, uid).await, 2, "both calls recorded");
    pool.close().await;

    // Pagination: per_page=1 → total=2, total_pages=2, one row on the page.
    let page1: serde_json::Value = reqwest::Client::new()
        .get(server.api_url("/mcp/tool-calls?page=1&per_page=1"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page1["total"], json!(2));
    assert_eq!(page1["total_pages"], json!(2));
    assert_eq!(page1["per_page"], json!(1));
    assert_eq!(page1["calls"].as_array().unwrap().len(), 1);

    // server_id filter: only id_a's call.
    let filtered: serde_json::Value = reqwest::Client::new()
        .get(server.api_url(&format!("/mcp/tool-calls?server_id={id_a}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(filtered["total"], json!(1));
    let calls = filtered["calls"].as_array().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["server_name"], json!("tc_page_a"));
}

/// Real-LLM chat-path coverage: proves the chat tool loop records a row with
/// `source='chat'` + the conversation/message ids (which the deterministic REST
/// path can't exercise). Uses the in-process `MockMcpServer` as the tool source
/// — only the LLM call needs a key, so this is far more reliable than a uvx
/// server. Runs in the real-LLM tier (source `tests/.env.test`); like the rest
/// of the chat-path MCP suite it hard-fails without a provider key rather than
/// silently skipping (per the no-`#[ignore]` policy).
#[tokio::test]
async fn chat_path_tool_call_records_source_chat() {
    let server = crate::common::TestServer::start().await;
    let mock = MockMcpServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tc_chat",
        &[
            "conversations::create",
            "conversations::edit",
            "messages::create",
            "mcp_servers::create",
            "mcp_servers::read",
        ],
    )
    .await;
    let uid = Uuid::parse_str(&user.user_id).unwrap();

    // Auto-approve the user's MCP tools so the LLM-driven call EXECUTES (and is
    // recorded) instead of pausing for manual approval. User servers — unlike
    // built-ins — default to manual_approve, so without this the tool call
    // creates an approval record and never reaches call_tool.
    let approve = reqwest::Client::new()
        .put(server.api_url("/mcp/defaults"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "approval_mode": "auto_approve",
            "auto_approved_tools": [],
            "disabled_servers": []
        }))
        .send()
        .await
        .unwrap();
    assert!(
        approve.status().is_success(),
        "set auto-approve defaults: {}",
        approve.status()
    );

    // A tool-capable Anthropic model (capabilities.tools=true at CREATE — the
    // load-bearing bit). The shared get_or_create_test_model omits the tools
    // flag, resolving the model NON-tool-capable so the chat extension never
    // offers the MCP tools and the LLM can't call them. Mirrors the proven
    // lit_search/web_search real-LLM helpers. See
    // [[project_real_llm_tool_test_capability]].
    let model = create_tool_capable_model(&server, &user.user_id).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);

    // The Rust mock is a FIFO queue (no sticky defaults): the create-time health
    // probe consumes one `tools/list`, and EACH before_llm_call round consumes
    // another. Over-queue so the probe + the chat loop's re-collection rounds are
    // all covered (a single tool turn is ~2 rounds; 10 is generous headroom).
    let tool_def = json!({
        "tools": [{
            "name": "get_secret_word",
            "description": "Returns today's secret word. Call this when asked for the secret word.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        }]
    });
    for _ in 0..10 {
        mock.on_method("tools/list", MockResponse::JsonOk(tool_def.clone()));
    }
    for _ in 0..5 {
        mock.on_method(
            "tools/call",
            MockResponse::JsonOk(json!({
                "content": [{ "type": "text", "text": "The secret word is banana." }],
                "isError": false
            })),
        );
    }
    let mcp_id = register_mock_server(&server, &user.token, "tc_chat_mock", &mock.base_url()).await;

    let conversation =
        crate::chat::helpers::create_conversation(&server, &user.token, None, None).await;
    let conversation_id = crate::chat::helpers::parse_uuid(&conversation["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conversation["active_branch_id"]);

    let body = json!({
        "content": "You MUST call the get_secret_word tool — do not answer from \
                    memory — then tell me the secret word.",
        "model_id": model_id,
        "branch_id": branch_id,
        "enable_mcp": true,
        "mcp_config": { "mcp_servers": [{ "server_id": mcp_id, "tools": [] }] },
    });
    let events = crate::chat::helpers::send_body_and_collect_events(
        &server,
        &user.token,
        conversation_id,
        body,
        &["complete"],
    )
    .await;

    // Guard: the model actually invoked the tool. Separating this from the DB
    // assertion below distinguishes an LLM/setup miss (no mcpToolStart) from a
    // recording bug (tool fired but no row).
    let tool_starts = events.iter().filter(|e| e.event == "mcpToolStart").count();
    assert!(
        tool_starts > 0,
        "model should have called get_secret_word (no mcpToolStart event)"
    );

    // The chat tool loop should have recorded a `source='chat'` row with the
    // conversation + message ids stamped by after_llm_call.
    let pool = pool(&server).await;
    let mut found = None;
    for _ in 0..60 {
        let row = sqlx::query(
            "SELECT source, conversation_id, branch_id, message_id, tool_name
             FROM mcp_tool_calls
             WHERE user_id = $1 AND source = 'chat'
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(uid)
        .fetch_optional(&pool)
        .await
        .unwrap();
        if let Some(r) = row {
            found = Some(r);
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    pool.close().await;

    let row = found.expect("a chat-path tool call should record a row with source='chat'");
    assert_eq!(row.get::<String, _>("source"), "chat");
    assert_eq!(row.get::<String, _>("tool_name"), "get_secret_word");
    assert_eq!(
        row.get::<Option<Uuid>, _>("conversation_id"),
        Some(conversation_id),
        "chat-path row carries the conversation id"
    );
    assert!(
        row.get::<Option<Uuid>, _>("branch_id").is_some(),
        "chat-path row carries the branch id"
    );
    assert!(
        row.get::<Option<Uuid>, _>("message_id").is_some(),
        "chat-path row carries the assistant message id"
    );
}

#[tokio::test]
async fn retention_setting_roundtrips_and_omission_keeps_current() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tc_admin",
        &["mcp_servers::read", "mcp_user_policy::edit"],
    )
    .await;

    // Set retention to 30 days.
    let put = reqwest::Client::new()
        .put(server.api_url("/mcp/user-policy"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "allowed_transports": ["http"],
            "tool_call_retention_days": 30,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), 200);
    let body: serde_json::Value = put.json().await.unwrap();
    assert_eq!(body["tool_call_retention_days"], json!(30));

    // Omitting the field on a later PUT keeps the current value (COALESCE).
    let put2 = reqwest::Client::new()
        .put(server.api_url("/mcp/user-policy"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "allowed_transports": ["http"] }))
        .send()
        .await
        .unwrap();
    let body2: serde_json::Value = put2.json().await.unwrap();
    assert_eq!(
        body2["tool_call_retention_days"],
        json!(30),
        "omitting retention keeps the prior value"
    );
}

/// Configure the built-in Anthropic provider with the test key and create a chat
/// model flagged `capabilities.tools = true` (the load-bearing bit for the LLM
/// to actually invoke tools), then grant `user_id` access. Mirrors the proven
/// lit_search/web_search real-LLM helpers. Requires `ANTHROPIC_API_KEY` (ships in
/// `tests/.env.test`); panics rather than silently skipping, per the no-`#[ignore]`
/// policy for real-LLM tests.
async fn create_tool_capable_model(
    server: &crate::common::TestServer,
    user_id: &str,
) -> serde_json::Value {
    let admin = crate::common::test_helpers::create_user_with_permissions(
        server,
        "tc_llm_admin",
        &[
            "llm_providers::read",
            "llm_providers::edit",
            "llm_models::read",
            "llm_models::create",
        ],
    )
    .await;

    let body: serde_json::Value = reqwest::Client::new()
        .get(server.api_url("/llm-providers?per_page=100"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let provider_id = body["providers"]
        .as_array()
        .expect("providers array")
        .iter()
        .find(|p| p["name"].as_str() == Some("Anthropic"))
        .expect("built-in Anthropic provider")["id"]
        .as_str()
        .unwrap()
        .to_string();

    let key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY required (source tests/.env.test)");
    let r = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{provider_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled": true, "api_key": key }))
        .send()
        .await
        .unwrap();
    assert!(
        r.status().is_success(),
        "configure Anthropic provider → {}",
        r.status()
    );

    let r = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "provider_id": provider_id,
            "name": "claude-opus-4-1-20250805",
            "display_name": "Claude Opus 4.1 (tool-call history)",
            "description": "tool-call history real-LLM model",
            "enabled": true,
            "engine_type": "none",
            "file_format": "gguf",
            "capabilities": { "chat": true, "completion": true, "tools": true }
        }))
        .send()
        .await
        .unwrap();
    let status = r.status();
    let model: serde_json::Value = r.json().await.unwrap();
    assert_eq!(
        status,
        reqwest::StatusCode::CREATED,
        "create model → {status}: {model}"
    );

    crate::chat::helpers::ensure_user_has_model_access(server, user_id, &model).await;
    model
}

/// Retention prune (mcp/tool_calls/prune.rs → McpRepository::prune_tool_calls →
/// `DELETE FROM mcp_tool_calls WHERE created_at < cutoff`). The existing
/// suite only round-trips the retention SETTING; this exercises the actual
/// deletion: an old row is pruned, a recent row is kept.
#[tokio::test]
async fn prune_deletes_rows_older_than_cutoff_and_keeps_recent() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "tc_prune", &[]).await;
    let user_id = Uuid::parse_str(&user.user_id).unwrap();

    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();

    // Two owner-scoped tool-call rows: one ~100 days old, one fresh.
    let old_id = Uuid::new_v4();
    let recent_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO mcp_tool_calls (id, user_id, server_name, tool_name, created_at) \
         VALUES ($1, $2, 'srv', 'echo', now() - interval '100 days')",
    )
    .bind(old_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO mcp_tool_calls (id, user_id, server_name, tool_name, created_at) \
         VALUES ($1, $2, 'srv', 'echo', now())",
    )
    .bind(recent_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    // Prune everything older than 50 days via the REAL repository method.
    let cutoff = time::OffsetDateTime::now_utc() - time::Duration::days(50);
    let pruned = ziee::mcp::McpRepository::new(pool.clone())
        .prune_tool_calls(cutoff)
        .await
        .expect("prune should succeed");
    assert_eq!(pruned, 1, "exactly the >50d-old row is pruned");

    let old_gone: i64 =
        sqlx::query_scalar("SELECT count(*) FROM mcp_tool_calls WHERE id = $1")
            .bind(old_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(old_gone, 0, "the old row must be deleted");
    let recent_kept: i64 =
        sqlx::query_scalar("SELECT count(*) FROM mcp_tool_calls WHERE id = $1")
            .bind(recent_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(recent_kept, 1, "the recent row must be kept");
    pool.close().await;
}
