//! Real-LLM end-to-end: a tool-capable model, told to load a skill, actually
//! invokes the `skill_mcp` `load_skill` tool in a chat — and the returned
//! tool result carries the skill body. Runs when `ANTHROPIC_API_KEY` is set
//! (tests/.env.test) — a SOFT-SKIP, NOT `#[ignore]`, so a sourced suite
//! exercises it. The skill DATA comes from the in-test mock Pages hub (the
//! download → sha256 → extract path runs for real), so the assertions are
//! deterministic; only the LLM provider is live. Mirrors
//! `citations/real_llm.rs`.

use serde_json::{Value, json};
use uuid::Uuid;

use super::{FIXTURE_SKILL_NAME, install_fixture_skill, refresh_catalog};
use crate::common::TestServer;
use crate::common::TestServerOptions;
use crate::common::test_helpers::create_user_with_permissions;
use crate::hub::mock_release_server::spawn_mock_hub;

/// Deterministic id of the built-in `skill_mcp` server row, derived exactly
/// as `skill_mcp::skill_mcp_server_id()` (which is private to the crate).
fn skill_mcp_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"skill.ziee.internal")
}

#[tokio::test]
async fn real_llm_invokes_load_skill_tool() {
    let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") else {
        eprintln!("skipping skill::real_llm — ANTHROPIC_API_KEY unset");
        return;
    };

    // The skill bundle is served by the in-test mock Pages hub; merge its
    // `ZIEE_HUB_PAGES_BASE` override with the live Anthropic key.
    let mock = spawn_mock_hub(super::skill_catalog()).await;
    let mut extra_env = mock.test_env();
    extra_env.push(("ANTHROPIC_API_KEY".to_string(), api_key.clone()));
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env,
        ..Default::default()
    })
    .await;

    // One user that can refresh the catalog + install a skill AND run a chat.
    let user = create_user_with_permissions(
        &server,
        "skill_real_llm",
        &[
            "hub::catalog::read",
            "hub::catalog::manage",
            "skills::read",
            "skills::install",
            "skills::manage",
            "conversations::create",
            "conversations::read",
            "conversations::edit",
            "messages::create",
            "messages::read",
            "llm_models::read",
        ],
    )
    .await;

    // Activate the mock catalog, then install the fixture skill so the user
    // has ≥1 available skill (the `load_skill` target).
    refresh_catalog(&server, &user.token).await;
    install_fixture_skill(&server, &user.token).await;

    // Wait for the boot upsert of the skill_mcp row, then grant the default
    // group access (the user's read of the built-in server is group-scoped).
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
            "Use the load_skill tool to load the skill named \"{FIXTURE_SKILL_NAME}\", \
             then tell me what its body says. You MUST call the tool — do not answer from memory."
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

    let tool_start = events.iter().filter(|e| e.event == "mcpToolStart").count();
    let tool_complete = events.iter().filter(|e| e.event == "mcpToolComplete").count();
    assert!(
        tool_start > 0,
        "the model should have called the skill_mcp load_skill tool (no mcpToolStart)"
    );
    assert!(
        tool_complete > 0,
        "the skill_mcp tool call should have completed (no mcpToolComplete)"
    );

    // Stronger than counts: a tool event must name `load_skill`, and the
    // SKILL.md body marker must surface in the collected stream (the real
    // bundle was loaded through the real tool, not hallucinated).
    let blob = events
        .iter()
        .map(|e| e.data.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        blob.contains("load_skill"),
        "a tool event should name the load_skill tool; events: {blob}"
    );
    assert!(
        blob.contains("THIS_IS_THE_SKILL_BODY_MARKER"),
        "the loaded skill body should surface via the real tool result; events: {blob}"
    );
}

/// Configure the built-in Anthropic provider with the test key + create a
/// tool-capable model, then grant `user_id` access. Mirrors the
/// citations/lit_search real-LLM helper.
async fn create_tool_capable_anthropic_model(
    server: &TestServer,
    user_id: &str,
    api_key: &str,
) -> Value {
    let admin = create_user_with_permissions(
        server,
        "skill_llm_admin",
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

    let r = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{provider_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled": true, "api_key": api_key }))
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
            "description": "skill_mcp real-LLM tool-capable model",
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
