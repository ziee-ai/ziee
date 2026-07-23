//! Tier 4 — real-network + real-LLM smoke. The connector smokes hit the live
//! public APIs (opt-in via `ZIEE_LIT_REAL_NETWORK=1`, off in CI — catches API
//! drift). The real-LLM test is gated on `ANTHROPIC_API_KEY` (present in
//! `tests/.env.test`) so it RUNS when the suite is sourced, like web_search's.

use serde_json::json;
use uuid::Uuid;

use crate::common::test_helpers::create_user_with_permissions;
use crate::common::TestServer;
use crate::lit_search::{configure, jsonrpc, jsonrpc_conv};

fn lit_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"lit_search.ziee.internal")
}

fn admin_perms() -> &'static [&'static str] {
    &["lit_search::use", "lit_search::admin::read", "lit_search::admin::manage"]
}

async fn seed_conversation(server: &TestServer, user_id: &str) -> Uuid {
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let conv_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO conversations (id, user_id, title, active_branch_id, created_at, updated_at)
           VALUES ($1, $2, 'lit real', NULL, NOW(), NOW())"#,
    )
    .bind(conv_id)
    .bind(Uuid::parse_str(user_id).unwrap())
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;
    conv_id
}

#[tokio::test]
async fn real_keyless_sources_return_deduped_dois() {
    if std::env::var("ZIEE_LIT_REAL_NETWORK").is_err() {
        eprintln!("skipping real_keyless_sources_return_deduped_dois: ZIEE_LIT_REAL_NETWORK unset");
        return;
    }
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ls_real_src", admin_perms()).await;
    configure(&server, &admin.token, &["europepmc", "crossref"]).await;

    let res = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({ "name": "literature_search",
                "arguments": { "query": "CRISPR base editing off-target", "max_results": 10 } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let sc = &body["result"]["structuredContent"];
    let records = sc["records"].as_array().expect("records");
    assert!(!records.is_empty(), "live sources returned nothing: {body}");
    assert!(
        records.iter().any(|r| r["doi"].is_string()),
        "expected at least one DOI from the live sources: {body}"
    );
    assert!(sc["after_dedup"].as_u64().unwrap_or(0) > 0);
}

#[tokio::test]
async fn real_oa_fulltext_fetch_extracts_text() {
    if std::env::var("ZIEE_LIT_REAL_NETWORK").is_err() {
        eprintln!("skipping real_oa_fulltext_fetch_extracts_text: ZIEE_LIT_REAL_NETWORK unset");
        return;
    }
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ls_real_ft", admin_perms()).await;
    configure(&server, &admin.token, &["europepmc"]).await;
    let conv = seed_conversation(&server, &admin.user_id).await;

    // A stable, long-standing open-access PMC article (Europe PMC fullTextXML).
    let res = jsonrpc_conv(
        &server,
        &admin.token,
        &conv.to_string(),
        "tools/call",
        json!({ "name": "fetch_paper_fulltext", "arguments": { "ids": ["PMC5334499"] } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let paper = &body["result"]["structuredContent"]["papers"][0];
    // Live data may drift; assert the resolver shape held (full_text + content).
    assert_eq!(paper["status"], "full_text", "expected OA full text: {body}");
    assert!(paper["chars"].as_u64().unwrap_or(0) > 500, "expected extracted text: {body}");
}

#[tokio::test]
async fn real_llm_invokes_literature_search() {
    // Runs when ANTHROPIC_API_KEY is set (tests/.env.test) — NOT #[ignore]d, so a
    // sourced suite exercises it (mirrors web_search/real_llm_test.rs).
    let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") else {
        eprintln!("skipping real_llm_invokes_literature_search: ANTHROPIC_API_KEY unset");
        return;
    };
    // Deterministic upstream: a real LLM decides to call the tool, but the tool's
    // DATA comes from a loopback mock (not live Europe PMC) so the test can't
    // flake on API drift/outages. Mirrors web_search's real-LLM test.
    let epmc = crate::lit_search::start_mock_europepmc().await;
    let server = TestServer::start_with_options(crate::common::TestServerOptions {
        extra_env: vec![
            ("ANTHROPIC_API_KEY".to_string(), api_key),
            ("LIT_SEARCH_ALLOW_LOOPBACK".to_string(), "1".to_string()),
            ("LIT_SEARCH_EUROPEPMC_ENDPOINT".to_string(), format!("{epmc}/search")),
        ],
        ..Default::default()
    })
    .await;

    let user = create_user_with_permissions(
        &server,
        "ls_real_llm",
        &[
            "conversations::create",
            "conversations::read",
            "conversations::edit",
            "messages::create",
            "messages::read",
            "llm_models::read",
            "lit_search::use",
            "lit_search::admin::read",
            "lit_search::admin::manage",
        ],
    )
    .await;
    // Only the mocked source — keeps the tool result deterministic.
    configure(&server, &user.token, &["europepmc"]).await;

    // Wait for the boot upsert of the lit_search row, then make it group-accessible.
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let lit_id = lit_server_id();
    for _ in 0..50 {
        let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM mcp_servers WHERE id = $1")
            .bind(lit_id)
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
    .bind(lit_id)
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    // Tool-capable model (capabilities.tools=true) — `get_or_create_test_model`
    // omits the tools flag, which resolves the model NON-tool-capable and makes
    // the LLM hallucinate the call instead of really invoking it (see
    // [[project_real_llm_tool_test_capability]]). Mirrors web_search's helper.
    let model = create_tool_capable_anthropic_model(&server, &user.user_id).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);
    let conversation =
        crate::chat::helpers::create_conversation(&server, &user.token, Some(model_id), None).await;
    let conversation_id = crate::chat::helpers::parse_uuid(&conversation["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conversation["active_branch_id"]);

    let payload = json!({
        "content": "Use the literature_search tool to find papers on CRISPR base-editing \
                    off-target effects. You MUST call the tool — do not answer from memory.",
        "model_id": model_id.to_string(),
        "branch_id": branch_id.to_string(),
        "enable_mcp": true,
        "mcp_config": { "mcp_servers": [ { "server_id": lit_id.to_string(), "tools": [] } ] }
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
        "the model should have called literature_search (no mcpToolStart event)"
    );
    // Also assert the call COMPLETED — proves the mocked result actually flowed
    // back through the tool pipeline, not just that the model emitted a call.
    assert!(
        tool_complete > 0,
        "the literature_search call should have completed (no mcpToolComplete event)"
    );
}

/// Configure the built-in Anthropic provider with the test key and create a
/// chat model flagged `capabilities.tools = true` (the load-bearing bit), then
/// grant `user_id` access. Mirrors `web_search`'s tool-capable-model helper.
async fn create_tool_capable_anthropic_model(
    server: &TestServer,
    user_id: &str,
) -> serde_json::Value {
    let admin = create_user_with_permissions(
        server,
        "ls_llm_admin",
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
    let key = std::env::var("ANTHROPIC_API_KEY").unwrap();
    // Redirect at the local LLM bridge (ANTHROPIC_BASE_URL / ZIEE_TEST_LLM_BASE_URL)
    // — else the provider hits real api.anthropic.com with a placeholder key.
    let mut provider_payload = json!({ "enabled": true, "api_key": key });
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
    assert!(r.status().is_success(), "configure Anthropic provider → {}", r.status());

    let r = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "provider_id": provider_id,
            "name": "claude-opus-4-1-20250805",
            "display_name": "Claude Opus 4.1 (lit_search tools)",
            "description": "lit_search Tier-4 tool-capable model",
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
    assert_eq!(status, reqwest::StatusCode::CREATED, "create model → {status}: {model}");

    crate::chat::helpers::ensure_user_has_model_access(server, user_id, &model).await;
    model
}
