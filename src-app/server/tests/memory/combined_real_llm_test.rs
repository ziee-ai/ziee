//! Real-LLM CROSS-SUBSYSTEM combinations the per-subsystem suites never
//! exercise together. The existing memory real-LLM tests run memory in
//! isolation; these prove memory coexists, in a SINGLE real chat turn, with
//! (a) a custom assistant + a built-in MCP tool call, and (b) a built-in MCP
//! tool call — i.e. the chat extension chain (assistant → memory → MCP) fires
//! end-to-end without one subsystem clobbering another.
//!
//! Gating: deployment memory needs the embedding (Gemini) + extraction (Groq)
//! keys; the chat turn needs a tool-capable Anthropic model. Soft-skips (NOT
//! `#[ignore]`) when any key is missing, matching the rest of the real-LLM
//! tier. The citations tool's DATA is served by loopback resolver mocks so the
//! tool assertions stay deterministic; only the model's decision-to-call is
//! real.

use serde_json::{Value, json};
use uuid::Uuid;

use super::real_llm_helpers as h;
use crate::common::{TestServer, TestServerOptions};
use crate::common::test_helpers::create_user_with_permissions;

fn citations_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"citations.ziee.internal")
}

/// Collect assistant text deltas from the streamed events.
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

/// Boot a server with memory + the citations loopback resolver mocks + the
/// Anthropic key, enable deployment memory (Gemini embed + Groq extraction),
/// and grant the default group the citations MCP server. Returns the started
/// server. Caller creates the chat user + model.
async fn boot_memory_plus_citations(api_key: &str) -> TestServer {
    let doi = crate::citations::start_mock_doi_resolver().await;
    let idconv = crate::citations::start_mock_idconv().await;
    let crossref = crate::citations::start_mock_crossref().await;
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            ("ANTHROPIC_API_KEY".to_string(), api_key.to_string()),
            ("CITATIONS_RESOLVER_ENDPOINT".to_string(), doi),
            ("CITATIONS_IDCONV_ENDPOINT".to_string(), idconv),
            ("CITATIONS_CROSSREF_ENDPOINT".to_string(), crossref),
            ("CITATIONS_ALLOW_LOOPBACK".to_string(), "1".to_string()),
        ],
        ..Default::default()
    })
    .await;

    // Deployment memory ON (Gemini embedding + Groq extraction models wired).
    h::setup_real_providers(&server).await;

    // Grant the default group the citations built-in (auto-attached in chat).
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let cit_id = citations_server_id();
    for _ in 0..50 {
        let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM mcp_servers WHERE id = $1")
            .bind(cit_id)
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
    .bind(cit_id)
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    server
}

/// A chat user that can drive memory, chat, and the citations tool.
async fn combined_chat_user(server: &TestServer, name: &str) -> crate::common::test_helpers::TestUser {
    create_user_with_permissions(
        server,
        name,
        &[
            "memory::read",
            "memory::write",
            "conversations::create",
            "conversations::read",
            "conversations::edit",
            "messages::create",
            "messages::read",
            "llm_models::read",
            "mcp_servers::read",
            "citations::use",
            "citations::manage",
        ],
    )
    .await
}

/// Configure the built-in Anthropic provider + a tool-capable model, grant access.
async fn tool_capable_anthropic_model(server: &TestServer, user_id: &str, api_key: &str) -> Value {
    let admin = create_user_with_permissions(
        server,
        &format!("combo_model_admin_{}", &user_id[..8.min(user_id.len())]),
        &["llm_providers::read", "llm_providers::edit", "llm_models::read", "llm_models::create"],
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
    // — else the provider hits real api.anthropic.com with a placeholder key and
    // the model never emits a tool call.
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
            "name": "claude-opus-4-1-20250805",
            "display_name": "Claude (combined tools)",
            "description": "cross-subsystem real-LLM tool-capable model",
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
    assert_eq!(status, reqwest::StatusCode::CREATED, "create model → {status}: {model}");
    crate::chat::helpers::ensure_user_has_model_access(server, user_id, &model).await;
    model
}

/// Cross-subsystem: assistant (custom instructions) + memory (enabled & seeded)
/// + a built-in MCP tool call, all in ONE real chat turn. Asserts the assistant
/// instruction landed in the reply, the model invoked the citations tool, and
/// the memory subsystem is live (recall returns the seeded fact) — none of the
/// three subsystems suppresses the others.
#[tokio::test]
async fn real_llm_assistant_plus_memory_plus_mcp_tool() {
    let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") else {
        eprintln!("skipping memory::combined assistant+memory+mcp — ANTHROPIC_API_KEY unset");
        return;
    };
    if h::skip_if_no_keys("assistant_plus_memory_plus_mcp") {
        return;
    }

    let server = boot_memory_plus_citations(&api_key).await;
    let user = combined_chat_user(&server, "combo_amm").await;

    // Seed a memory; recall confirms the memory subsystem is active.
    let mem_id = h::mcp_remember(&server, &user.token, "The user's project codename is ORCHID.").await;
    h::wait_for_embedding(&server, &user.token, mem_id).await;
    let recalled = h::mcp_recall(&server, &user.token, "project codename", 5).await;
    assert!(
        recalled.iter().any(|m| m.contains("ORCHID")),
        "memory recall must surface the seeded fact; got {recalled:?}"
    );

    // A custom assistant that forces a marker token in every reply.
    let assistant: Value = reqwest::Client::new()
        .post(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "Marker Assistant",
            "instructions": "You MUST end every response with the literal token 'TAG_COMBO_Z7'."
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let assistant_id = assistant["id"].as_str().unwrap().to_string();

    let model = tool_capable_anthropic_model(&server, &user.user_id, &api_key).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);
    let conversation =
        crate::chat::helpers::create_conversation(&server, &user.token, Some(model_id), None).await;
    let conversation_id = crate::chat::helpers::parse_uuid(&conversation["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conversation["active_branch_id"]);

    let cit_id = citations_server_id();
    let payload = json!({
        "content": "Use the verify_citations tool to check whether DOI 10.5555/known resolves \
                    to a real record. You MUST call the tool — do not answer from memory.",
        "model_id": model_id.to_string(),
        "branch_id": branch_id.to_string(),
        "assistant_id": assistant_id,
        "enable_mcp": true,
        "mcp_config": { "mcp_servers": [ { "server_id": cit_id.to_string(), "tools": [] } ] }
    });
    let events = crate::chat::helpers::send_body_and_collect_events(
        &server,
        &user.token,
        conversation_id,
        payload,
        &["complete"],
    )
    .await;

    assert!(
        events.iter().any(|e| e.event == "mcpToolStart"),
        "the MCP tool must be invoked alongside assistant + memory context"
    );
    let text = assemble_text(&events);
    assert!(
        text.contains("TAG_COMBO_Z7"),
        "the assistant instruction must survive the combined turn; got {text:?}"
    );
}

/// Cross-subsystem: memory (enabled & seeded) combined with a built-in MCP tool
/// call in one real chat turn — the audit's "memory is only ever tested in
/// isolation" gap. Asserts recall AND the tool call coexist for the same user/
/// conversation. (code_sandbox/lit_search combinations need a mounted rootfs /
/// connector mocks and are exercised by their own real-LLM suites; this covers
/// the infra-light memory×citations pairing.)
#[tokio::test]
async fn real_llm_memory_plus_mcp_tool_not_isolated() {
    let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") else {
        eprintln!("skipping memory::combined memory+mcp — ANTHROPIC_API_KEY unset");
        return;
    };
    if h::skip_if_no_keys("memory_plus_mcp_tool") {
        return;
    }

    let server = boot_memory_plus_citations(&api_key).await;
    let user = combined_chat_user(&server, "combo_mm").await;

    let mem_id = h::mcp_remember(&server, &user.token, "The user prefers APA citation style.").await;
    h::wait_for_embedding(&server, &user.token, mem_id).await;
    let recalled = h::mcp_recall(&server, &user.token, "citation style preference", 5).await;
    assert!(
        recalled.iter().any(|m| m.contains("APA")),
        "memory recall must surface the seeded fact; got {recalled:?}"
    );

    let model = tool_capable_anthropic_model(&server, &user.user_id, &api_key).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);
    let conversation =
        crate::chat::helpers::create_conversation(&server, &user.token, Some(model_id), None).await;
    let conversation_id = crate::chat::helpers::parse_uuid(&conversation["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conversation["active_branch_id"]);

    let cit_id = citations_server_id();
    let payload = json!({
        "content": "Use the verify_citations tool to check whether DOI 10.5555/known resolves \
                    to a real record. You MUST call the tool — do not answer from memory.",
        "model_id": model_id.to_string(),
        "branch_id": branch_id.to_string(),
        "enable_mcp": true,
        "mcp_config": { "mcp_servers": [ { "server_id": cit_id.to_string(), "tools": [] } ] }
    });
    let events = crate::chat::helpers::send_body_and_collect_events(
        &server,
        &user.token,
        conversation_id,
        payload,
        &["complete"],
    )
    .await;

    assert!(
        events.iter().any(|e| e.event == "mcpToolStart"),
        "the MCP tool must be invoked while memory is active for the same user"
    );
    assert!(
        events.iter().any(|e| e.event == "mcpToolComplete"),
        "the tool call must complete in the combined turn"
    );
}
