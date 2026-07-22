//! Real-LLM end-to-end for the `skill_mcp` built-in: a tool-capable model,
//! told to load an installed skill, actually invokes the `load_skill` MCP tool.
//! The existing skill_mcp tests only exercise the JSON-RPC dispatch directly;
//! this proves the model-driven path (auto-attach → tool call) works against a
//! real provider. Soft-skips (NOT `#[ignore]`) when `ANTHROPIC_API_KEY` is
//! unset, so a sourced suite (tests/.env.test) runs it. The skill catalog +
//! bundle are served by the in-test mock hub, so install is hermetic.

use serde_json::{Value, json};
use uuid::Uuid;

use super::{FIXTURE_SKILL_NAME, install_fixture_skill, refresh_catalog, skill_catalog};
use crate::common::TestServer;
use crate::common::TestServerOptions;
use crate::common::test_helpers::create_user_with_permissions;
use crate::hub::mock_release_server::spawn_mock_hub;

fn skill_mcp_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"skill.ziee.internal")
}

#[tokio::test]
async fn real_llm_invokes_load_skill() {
    let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") else {
        eprintln!("skipping skill::real_llm — ANTHROPIC_API_KEY unset");
        return;
    };

    // Mock hub serves the fixture skill catalog + bundle (hermetic install).
    let mock = spawn_mock_hub(skill_catalog()).await;
    let mut env = mock.test_env();
    env.push(("ANTHROPIC_API_KEY".to_string(), api_key.clone()));
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: env,
        ..Default::default()
    })
    .await;

    // Admin refreshes the catalog so the mock's skill-bearing catalog is active.
    let admin = create_user_with_permissions(
        &server,
        "skill_llm_admin",
        &[
            "hub::catalog::read",
            "hub::catalog::manage",
            "skills::read",
            "skills::install",
            "skills::manage",
        ],
    )
    .await;
    refresh_catalog(&server, &admin.token).await;

    // The chat user installs the skill for itself.
    let user = create_user_with_permissions(
        &server,
        "skill_llm_user",
        &[
            "conversations::create",
            "conversations::read",
            "conversations::edit",
            "messages::create",
            "messages::read",
            "llm_models::read",
            "mcp_servers::read",
            "skills::read",
            "skills::install",
        ],
    )
    .await;
    install_fixture_skill(&server, &user.token).await;

    // Grant the default group access to the skill MCP server (auto-attached).
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let skill_id = skill_mcp_server_id();
    for _ in 0..50 {
        let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM mcp_servers WHERE id = $1")
            .bind(skill_id)
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
    .bind(skill_id)
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    let model = create_tool_capable_anthropic_model(&server, &user.user_id, &api_key).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);
    let conversation =
        crate::chat::helpers::create_conversation(&server, &user.token, Some(model_id), None).await;
    let conversation_id = crate::chat::helpers::parse_uuid(&conversation["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conversation["active_branch_id"]);

    let payload = json!({
        "content": format!(
            "Use the load_skill tool to load the skill named '{FIXTURE_SKILL_NAME}', then \
             summarize its first step. You MUST call the tool — do not answer from memory."
        ),
        "model_id": model_id.to_string(),
        "branch_id": branch_id.to_string(),
        "enable_mcp": true,
        "mcp_config": { "mcp_servers": [ { "server_id": skill_id.to_string(), "tools": [] } ] }
    });
    let events = crate::chat::helpers::send_body_and_collect_events(
        &server,
        &user.token,
        conversation_id,
        payload,
        &["complete"],
    )
    .await;

    let tool_starts: Vec<&str> = events
        .iter()
        .filter(|e| e.event == "mcpToolStart")
        .filter_map(|e| e.data["tool_name"].as_str())
        .collect();
    assert!(
        !tool_starts.is_empty(),
        "the model should have invoked a skill MCP tool (no mcpToolStart)"
    );
    assert!(
        tool_starts.contains(&"load_skill"),
        "the model should have called load_skill; got {tool_starts:?}"
    );
    assert!(
        events.iter().any(|e| e.event == "mcpToolComplete"),
        "the load_skill call should have completed"
    );
}

/// Configure the built-in Anthropic provider with the test key + create a
/// tool-capable model, then grant `user_id` access. Mirrors the citations
/// real-LLM helper.
async fn create_tool_capable_anthropic_model(
    server: &TestServer,
    user_id: &str,
    api_key: &str,
) -> Value {
    let admin = create_user_with_permissions(
        server,
        "skill_llm_model_admin",
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
    // — without it the provider hits real api.anthropic.com with the placeholder
    // key and the model never emits a tool call ("no mcpToolStart").
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
            "display_name": "Claude (skill tools)",
            "description": "skill real-LLM tool-capable model",
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
