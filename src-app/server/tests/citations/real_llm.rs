//! Real-LLM end-to-end: a tool-capable model, given a draft reference, actually
//! invokes the citations MCP tools. Runs when `ANTHROPIC_API_KEY` is set
//! (tests/.env.test) — a SOFT-SKIP, NOT `#[ignore]`, so a sourced suite
//! exercises it. The tool DATA comes from the loopback resolver mocks so the
//! assertions are deterministic (no live-API flake). Mirrors
//! `lit_search/real_llm_test.rs`.

use serde_json::json;
use uuid::Uuid;

use crate::common::TestServer;
use crate::common::TestServerOptions;
use crate::common::test_helpers::create_user_with_permissions;

fn citations_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"citations.ziee.internal")
}

#[tokio::test]
async fn real_llm_invokes_verify_citations() {
    let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") else {
        eprintln!("skipping citations::real_llm — ANTHROPIC_API_KEY unset");
        return;
    };

    // Deterministic resolver mocks (the model decides to call the tool; the
    // tool's data is canned).
    let doi = crate::citations::start_mock_doi_resolver().await;
    let idconv = crate::citations::start_mock_idconv().await;
    let crossref = crate::citations::start_mock_crossref().await;
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            ("ANTHROPIC_API_KEY".to_string(), api_key.clone()),
            ("CITATIONS_RESOLVER_ENDPOINT".to_string(), doi),
            ("CITATIONS_IDCONV_ENDPOINT".to_string(), idconv),
            ("CITATIONS_CROSSREF_ENDPOINT".to_string(), crossref),
            ("CITATIONS_ALLOW_LOOPBACK".to_string(), "1".to_string()),
        ],
        ..Default::default()
    })
    .await;

    let user = create_user_with_permissions(
        &server,
        "cit_real_llm",
        &[
            "conversations::create",
            "conversations::read",
            "conversations::edit",
            "messages::create",
            "messages::read",
            "llm_models::read",
            "citations::use",
            "citations::manage",
        ],
    )
    .await;

    // Wait for the boot upsert of the citations row, then grant the default group access.
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

    let model = create_tool_capable_anthropic_model(&server, &user.user_id, &api_key).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);
    let conversation =
        crate::chat::helpers::create_conversation(&server, &user.token, Some(model_id), None).await;
    let conversation_id = crate::chat::helpers::parse_uuid(&conversation["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conversation["active_branch_id"]);

    let payload = json!({
        "content": "Use the verify_citations tool to check whether DOI 10.5555/known \
                    resolves to a real record. You MUST call the tool — do not answer from memory.",
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

    let tool_start = events.iter().filter(|e| e.event == "mcpToolStart").count();
    let tool_complete = events.iter().filter(|e| e.event == "mcpToolComplete").count();
    assert!(tool_start > 0, "the model should have called a citations tool (no mcpToolStart)");
    assert!(tool_complete > 0, "the citations tool call should have completed (no mcpToolComplete)");
}

/// Configure the built-in Anthropic provider with the test key + create a
/// tool-capable model, then grant `user_id` access. Mirrors lit_search's helper.
async fn create_tool_capable_anthropic_model(
    server: &TestServer,
    user_id: &str,
    api_key: &str,
) -> serde_json::Value {
    let admin = create_user_with_permissions(
        server,
        "cit_llm_admin",
        &["llm_providers::read", "llm_providers::edit", "llm_models::read", "llm_models::create"],
    )
    .await;

    let body: serde_json::Value = reqwest::Client::new()
        .get(server.api_url("/llm-providers?per_page=100"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send().await.unwrap().json().await.unwrap();
    let provider_id = body["providers"].as_array().expect("providers")
        .iter().find(|p| p["name"].as_str() == Some("Anthropic"))
        .expect("Anthropic provider")["id"].as_str().unwrap().to_string();

    let r = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{provider_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled": true, "api_key": api_key }))
        .send().await.unwrap();
    assert!(r.status().is_success(), "configure Anthropic → {}", r.status());

    let r = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "provider_id": provider_id,
            "name": "claude-opus-4-1-20250805",
            "display_name": "Claude (citations tools)",
            "description": "citations real-LLM tool-capable model",
            "enabled": true,
            "engine_type": "none",
            "file_format": "gguf",
            "capabilities": { "chat": true, "completion": true, "tools": true }
        }))
        .send().await.unwrap();
    let status = r.status();
    let model: serde_json::Value = r.json().await.unwrap();
    assert_eq!(status, reqwest::StatusCode::CREATED, "create model → {status}: {model}");
    crate::chat::helpers::ensure_user_has_model_access(server, user_id, &model).await;
    model
}

/// Multi-turn: the citations MCP tool is usable across more than one chat turn
/// in the SAME conversation — turn 1 adds a citation, turn 2 lists it back.
/// Asserts the tool is invoked on BOTH turns (the built-in stays attached and
/// the conversation context carries across turns). Soft-skips without a key.
#[tokio::test]
async fn real_llm_multi_turn_citations() {
    let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") else {
        eprintln!("skipping citations::real_llm_multi_turn — ANTHROPIC_API_KEY unset");
        return;
    };

    let doi = crate::citations::start_mock_doi_resolver().await;
    let idconv = crate::citations::start_mock_idconv().await;
    let crossref = crate::citations::start_mock_crossref().await;
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            ("ANTHROPIC_API_KEY".to_string(), api_key.clone()),
            ("CITATIONS_RESOLVER_ENDPOINT".to_string(), doi),
            ("CITATIONS_IDCONV_ENDPOINT".to_string(), idconv),
            ("CITATIONS_CROSSREF_ENDPOINT".to_string(), crossref),
            ("CITATIONS_ALLOW_LOOPBACK".to_string(), "1".to_string()),
        ],
        ..Default::default()
    })
    .await;

    let user = create_user_with_permissions(
        &server,
        "cit_real_llm_mt",
        &[
            "conversations::create", "conversations::read", "conversations::edit",
            "messages::create", "messages::read", "llm_models::read",
            "citations::use", "citations::manage",
        ],
    )
    .await;

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

    let model = create_tool_capable_anthropic_model(&server, &user.user_id, &api_key).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);
    let conversation =
        crate::chat::helpers::create_conversation(&server, &user.token, Some(model_id), None).await;
    let conversation_id = crate::chat::helpers::parse_uuid(&conversation["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conversation["active_branch_id"]);

    let mcp_config = json!({ "mcp_servers": [ { "server_id": cit_id.to_string(), "tools": [] } ] });

    // Turn 1 — add a citation.
    let turn1 = json!({
        "content": "Use the add_citations tool to add DOI 10.5555/known to my library. \
                    You MUST call the tool — do not answer from memory.",
        "model_id": model_id.to_string(),
        "branch_id": branch_id.to_string(),
        "enable_mcp": true,
        "mcp_config": mcp_config,
    });
    let ev1 = crate::chat::helpers::send_body_and_collect_events(
        &server, &user.token, conversation_id, turn1, &["complete"],
    )
    .await;
    assert!(
        ev1.iter().any(|e| e.event == "mcpToolStart"),
        "turn 1 should invoke a citations tool"
    );

    // Turn 2 — same conversation/branch — list citations back.
    let turn2 = json!({
        "content": "Now use the list_citations tool to show what is saved in my library. \
                    You MUST call the tool.",
        "model_id": model_id.to_string(),
        "branch_id": branch_id.to_string(),
        "enable_mcp": true,
        "mcp_config": mcp_config,
    });
    let ev2 = crate::chat::helpers::send_body_and_collect_events(
        &server, &user.token, conversation_id, turn2, &["complete"],
    )
    .await;
    assert!(
        ev2.iter().any(|e| e.event == "mcpToolStart"),
        "turn 2 should also invoke a citations tool (built-in stays attached across turns)"
    );
}
