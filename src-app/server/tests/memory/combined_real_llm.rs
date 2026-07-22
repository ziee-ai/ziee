//! Tier-5 real-LLM CROSS-SUBSYSTEM test: assistant + memory + MCP tool call
//! all participating in a SINGLE chat turn.
//!
//! Audit gap `all-9ecd91ccfbfa`: the project `injection_test` covers
//! assistant + project stacking, and the memory/citations real-LLM suites
//! each exercise their own subsystem in isolation — but nothing proves the
//! three independent `before_llm_call` participants compose correctly in one
//! request:
//!
//!   1. ASSISTANT — `assistant/chat_extension/assistant.rs` injects the
//!      assistant's instructions (asserted via a mandatory beacon token in
//!      the reply).
//!   2. MEMORY — a real, embedded user memory is retrievable (real Gemini
//!      embeddings via `setup_real_providers`).
//!   3. MCP TOOL CALL — the privileged built-in memory MCP server
//!      (`memory.ziee.internal`) is attached and the tool-capable model
//!      actually invokes its `recall` tool (asserted via `mcpToolStart`),
//!      surfacing the remembered fact in the answer.
//!
//! Gated: soft-skips unless `ANTHROPIC_API_KEY` (chat) AND
//! `GEMINI_API_KEY` + `GROQ_API_KEY` (memory embeddings/extraction) are set
//! — the union of the chat + memory real-LLM key requirements. Mirrors the
//! soft-skip convention of `citations/real_llm.rs` and `memory/real_llm_test.rs`.
//!
//! ```bash
//! source tests/.env.test
//! cargo test --test integration_tests memory::combined_real_llm \
//!     -- --test-threads=1 --nocapture
//! ```

#![allow(dead_code)]

use serde_json::{Value, json};
use uuid::Uuid;

use super::real_llm_helpers as h;
use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;

/// Deterministic id of the built-in memory MCP server row
/// (`memory_mcp::memory_mcp_server_id()` — `Uuid::new_v5(URL, "memory.ziee.internal")`).
fn memory_mcp_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"memory.ziee.internal")
}

#[tokio::test]
async fn assistant_memory_and_mcp_tool_combine_in_one_turn() {
    // Union gating: chat key + memory (embedding/extraction) keys.
    let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") else {
        eprintln!("skipping memory::combined_real_llm — ANTHROPIC_API_KEY unset");
        return;
    };
    if h::skip_if_no_keys("combined_real_llm") {
        return;
    }

    let server = TestServer::start().await;

    // MEMORY subsystem: real Gemini embeddings + Groq extraction, memory
    // enabled deployment-wide (also wires the embedding model used for
    // retrieval at chat time).
    let _ids = h::setup_real_providers(&server).await;

    // The chat user owns the memory AND drives the conversation. Needs
    // memory + conversation + message + assistant-create + model-read perms.
    let user = create_user_with_permissions(
        &server,
        "combined_chat_user",
        &[
            "conversations::create",
            "conversations::read",
            "conversations::edit",
            "messages::create",
            "messages::read",
            "llm_models::read",
            "memory::read",
            "memory::write",
            "assistants::create",
            "assistants::read",
        ],
    )
    .await;

    // Grant the user's default group access to the built-in memory MCP row so
    // it is attachable in chat (mirrors citations/real_llm.rs). Wait for the
    // boot upsert of the row first.
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let mem_id = memory_mcp_server_id();
    for _ in 0..50 {
        let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM mcp_servers WHERE id = $1")
            .bind(mem_id)
            .fetch_optional(&pool)
            .await
            .unwrap();
        if exists.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let default_group: Uuid =
        sqlx::query_scalar("SELECT id FROM groups WHERE is_default = true LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "INSERT INTO user_group_mcp_servers (group_id, mcp_server_id, assigned_at) \
         VALUES ($1, $2, NOW()) ON CONFLICT DO NOTHING",
    )
    .bind(default_group)
    .bind(mem_id)
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    // Tool-capable Anthropic Haiku model (so the model can invoke the memory
    // recall tool), granted to the chat user.
    let model = create_tool_capable_anthropic_model(&server, &user.user_id, &api_key).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);

    // Seed a real, embedded memory under the CHAT user (per-user retrieval).
    // The codename is unguessable so it can only reach the answer via the
    // memory subsystem (injection or the recall tool).
    let codename = "NIMBUS_DELTA_7";
    let memory_id = h::mcp_remember(
        &server,
        &user.token,
        &format!("The classified project codename is {codename}."),
    )
    .await;
    h::wait_for_embedding(&server, &user.token, memory_id).await;

    // ASSISTANT subsystem: a beacon instruction the reply must echo.
    let beacon = "COMBINED_TAG_Q7";
    let assistant: Value = reqwest::Client::new()
        .post(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "Combined Beacon",
            "instructions": format!(
                "You MUST end every response with the literal token '{beacon}'. Mandatory."
            ),
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let assistant_id = assistant["id"].as_str().expect("assistant id");

    let conversation = crate::chat::helpers::create_conversation(
        &server,
        &user.token,
        Some(model_id),
        None,
    )
    .await;
    let conversation_id = crate::chat::helpers::parse_uuid(&conversation["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conversation["active_branch_id"]);

    // One turn that should engage all three: the assistant beacon shapes the
    // reply; the memory MCP server is attached and the model is told to call
    // its `recall` tool to fetch the codename (forcing the MCP tool call); the
    // recalled fact must then appear in the answer.
    let payload = json!({
        "content": "Use your `recall` tool to look up the classified project codename, \
                    then state it verbatim in your answer. You MUST call the recall tool \
                    — do not answer from prior knowledge.",
        "model_id": model_id.to_string(),
        "branch_id": branch_id.to_string(),
        "assistant_id": assistant_id,
        "enable_mcp": true,
        "mcp_config": { "mcp_servers": [ { "server_id": mem_id.to_string(), "tools": [] } ] }
    });

    let events = crate::chat::helpers::send_body_and_collect_events(
        &server,
        &user.token,
        conversation_id,
        payload,
        &["complete"],
    )
    .await;

    let response_text = assemble_text(&events);
    let tool_start = events.iter().filter(|e| e.event == "mcpToolStart").count();
    eprintln!("combined reply: {response_text}\nmcpToolStart count: {tool_start}");

    // (3) MCP TOOL CALL — the model invoked an MCP tool (the memory recall).
    assert!(
        tool_start > 0,
        "the model should have invoked the memory MCP recall tool (no mcpToolStart); got: {response_text:?}"
    );
    // (1) ASSISTANT — the beacon instruction reached the model and shaped the reply.
    assert!(
        response_text.contains(beacon),
        "reply must carry the ASSISTANT beacon '{beacon}'; got: {response_text:?}"
    );
    // (2) MEMORY — the embedded codename surfaced (only reachable via the
    // memory subsystem: injection or the recall tool result).
    assert!(
        response_text.contains(codename),
        "reply must surface the MEMORY codename '{codename}'; got: {response_text:?}"
    );
}

/// Concatenate `text_delta`s from streamed `content` events (the
/// `send_body_and_collect_events` shape).
fn assemble_text(events: &[crate::chat::helpers::SSEEvent]) -> String {
    let mut text = String::new();
    for e in events {
        if e.event == "content"
            && let Some(arr) = e.data.get("content").and_then(|v| v.as_array())
        {
            for delta in arr {
                if delta.get("type").and_then(|v| v.as_str()) == Some("text_delta")
                    && let Some(s) = delta.get("delta").and_then(|v| v.as_str())
                {
                    text.push_str(s);
                }
            }
        }
    }
    text
}

/// Configure the built-in Anthropic provider with the test key + create a
/// tool-capable Haiku model, then grant `user_id` access. Mirrors
/// `citations/real_llm.rs::create_tool_capable_anthropic_model` but with the
/// cheap Haiku 4.5 snapshot the project injection suite uses.
async fn create_tool_capable_anthropic_model(
    server: &TestServer,
    user_id: &str,
    api_key: &str,
) -> Value {
    let admin = create_user_with_permissions(
        server,
        "combined_llm_admin",
        &[
            "llm_providers::read",
            "llm_providers::edit",
            "llm_models::read",
            "llm_models::create",
        ],
    )
    .await;

    let body: Value = reqwest::Client::new()
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
        .expect("providers")
        .iter()
        .find(|p| p["name"].as_str() == Some("Anthropic"))
        .expect("Anthropic provider")["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Redirect at the local LLM bridge (ANTHROPIC_BASE_URL / ZIEE_TEST_LLM_BASE_URL)
    // — else the provider hits real api.anthropic.com with a placeholder key.
    let mut provider_payload = json!({ "enabled": true, "api_key": api_key });
    if let Some(base_url) = crate::chat::helpers::test_provider_base_url("ANTHROPIC_API_KEY") {
        provider_payload["base_url"] = json!(base_url);
    }
    let r = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{provider_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&provider_payload)
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "configure Anthropic → {}", r.status());

    let r = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "provider_id": provider_id,
            "name": "claude-haiku-4-5-20251001",
            "display_name": "Claude Haiku 4.5 (combined tools)",
            "description": "combined real-LLM tool-capable model",
            "enabled": true,
            "engine_type": "none",
            "file_format": "gguf",
            "capabilities": { "chat": true, "completion": true, "tools": true }
        }))
        .send()
        .await
        .unwrap();
    let status = r.status();
    let model: Value = r.json().await.unwrap();
    assert_eq!(
        status,
        reqwest::StatusCode::CREATED,
        "create model → {status}: {model}"
    );
    crate::chat::helpers::ensure_user_has_model_access(server, user_id, &model).await;
    model
}
