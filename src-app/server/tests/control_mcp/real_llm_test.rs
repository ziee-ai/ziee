//! Tier 4 — real-LLM end-to-end for the control MCP server. Soft-skips without
//! `ANTHROPIC_API_KEY`; runs against the local DeepSeek/Qwen bridge when
//! `ANTHROPIC_BASE_URL` is set (no paid keys). Proves two properties with a real
//! model driving the tools:
//!   1. discovery — the model calls `list_capabilities` (read-only, auto-runs).
//!   2. security — a write (`invoke_capability` of a mutating op) is FORCED
//!      through approval, so nothing is created until the user approves.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::TestServer;
use crate::common::TestServerOptions;
use crate::common::test_helpers::create_user_with_permissions;

fn control_mcp_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"control.ziee.internal")
}

/// Configure the built-in Anthropic provider (redirected at the local bridge via
/// the `test_provider_base_url` seam) + create a tool-capable model, then grant
/// `user_id` access.
async fn tool_capable_model(server: &TestServer, user_id: &str, api_key: &str) -> Value {
    let admin = create_user_with_permissions(
        server,
        "ctl_llm_admin",
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

    // Apply the DeepSeek/Qwen bridge base_url seam when present.
    let mut update = json!({ "enabled": true, "api_key": api_key });
    if let Some(base_url) = crate::chat::helpers::test_provider_base_url("ANTHROPIC_API_KEY") {
        update["base_url"] = json!(base_url);
    }
    let r = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{provider_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&update)
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
            "display_name": "Claude (control tools)",
            "description": "control real-LLM tool-capable model",
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

async fn setup(api_key: &str) -> (TestServer, crate::common::test_helpers::TestUser, Uuid, Uuid, Uuid) {
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("ANTHROPIC_API_KEY".to_string(), api_key.to_string())],
        ..Default::default()
    })
    .await;
    let user = create_user_with_permissions(
        &server,
        "ctl_real",
        &[
            "conversations::create",
            "conversations::read",
            "conversations::edit",
            "messages::create",
            "messages::read",
            "llm_models::read",
            "control::use",
            "assistants::create",
            "assistants::read",
        ],
    )
    .await;
    let model = tool_capable_model(&server, &user.user_id, api_key).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);
    let conversation =
        crate::chat::helpers::create_conversation(&server, &user.token, Some(model_id), None).await;
    let conversation_id = crate::chat::helpers::parse_uuid(&conversation["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conversation["active_branch_id"]);
    (server, user, conversation_id, branch_id, model_id)
}

#[tokio::test]
async fn real_llm_discovers_capabilities() {
    let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") else {
        eprintln!("skipping control_mcp::real_llm_discovers_capabilities — ANTHROPIC_API_KEY unset");
        return;
    };
    let (server, user, conversation_id, branch_id, model_id) = setup(&api_key).await;

    let payload = json!({
        "content": "What can you do to manage this application? You MUST call the \
                    list_capabilities tool to find out — do not answer from memory.",
        "model_id": model_id.to_string(),
        "branch_id": branch_id.to_string(),
        "enable_mcp": true
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
        "the model should have called a control tool (no mcpToolStart)"
    );

    // A control tool call must be recorded to mcp_tool_calls (fire-and-forget →
    // poll briefly).
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let mut found = false;
    for _ in 0..30 {
        let n = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM mcp_tool_calls WHERE server_id = $1 AND tool_name = 'list_capabilities'",
            control_mcp_server_id()
        )
        .fetch_one(&pool)
        .await
        .unwrap()
        .unwrap_or(0);
        if n > 0 {
            found = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    assert!(found, "a list_capabilities control call must be recorded in mcp_tool_calls");
}

#[tokio::test]
async fn real_llm_write_requires_approval() {
    let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") else {
        eprintln!("skipping control_mcp::real_llm_write_requires_approval — ANTHROPIC_API_KEY unset");
        return;
    };
    let (server, user, conversation_id, branch_id, model_id) = setup(&api_key).await;
    let name = format!("CtlLLM-{}", &Uuid::new_v4().to_string()[..8]);

    let payload = json!({
        "content": format!(
            "Create a new assistant named '{name}' using the app-control tools \
             (invoke_capability with Assistant.create). Do it now; do not ask me first."
        ),
        "model_id": model_id.to_string(),
        "branch_id": branch_id.to_string(),
        "enable_mcp": true
    });
    let events = crate::chat::helpers::send_body_and_collect_events(
        &server,
        &user.token,
        conversation_id,
        payload,
        &["complete"],
    )
    .await;

    // The mutating invoke must be gated: an approval was requested and the
    // assistant was NOT created (it waits behind the user's approval).
    let approval_requested = events.iter().any(|e| e.event == "mcpApprovalRequired");
    assert!(
        approval_requested,
        "a mutating control invoke must force an approval prompt (no mcpApprovalRequired seen)"
    );

    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let count = sqlx::query_scalar!("SELECT COUNT(*) FROM assistants WHERE name = $1", name)
        .fetch_one(&pool)
        .await
        .unwrap()
        .unwrap_or(0);
    assert_eq!(
        count, 0,
        "the assistant must NOT exist until the user approves the control write"
    );
}
