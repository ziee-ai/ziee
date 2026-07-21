//! TEST-23 — the "never invent a citation" rule holds on the agent-core loop:
//! an agent given a FABRICATED DOI surfaces `not_found` (the resolver 404s) rather
//! than fabricating a record. Runs on the chat path with the cutover flag ON, so
//! the tool round-trip goes through the shared `AgentCore` loop. The DETERMINISTIC
//! anchor is the loopback resolver returning 404 for any DOI except the one known
//! record → `verify_citations` must classify `not_found`; we assert the tool fired
//! AND the persisted tool result carries `not_found` (never a fabricated hit).
//!
//! Bridge-gated: soft-skips unless `ZIEE_TEST_LLM_BASE_URL` is set.

use serde_json::{json, Value};
use uuid::Uuid;

use crate::common::test_helpers::create_user_with_permissions;
use crate::common::{TestServer, TestServerOptions};

fn citations_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"citations.ziee.internal")
}

#[tokio::test]
async fn agent_core_fabricated_citation_is_not_found_not_invented() {
    let base = match std::env::var("ZIEE_TEST_LLM_BASE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!("SKIP agent_core_fabricated_citation — ZIEE_TEST_LLM_BASE_URL unset");
            return;
        }
    };
    let key = std::env::var("ZIEE_TEST_LLM_KEY").unwrap_or_else(|_| "sk-local-audit".into());
    let model_name = std::env::var("ZIEE_TEST_LLM_MODEL").unwrap_or_else(|_| "qwen3.6-35b-a3b".into());

    // Loopback resolver mocks: any DOI except 10.5555/known → 404 (fabricated case).
    let doi = crate::citations::start_mock_doi_resolver().await;
    let idconv = crate::citations::start_mock_idconv().await;
    let crossref = crate::citations::start_mock_crossref().await;
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            ("ZIEE_CHAT_AGENT_CORE".to_string(), "1".to_string()),
            ("CITATIONS_RESOLVER_ENDPOINT".to_string(), doi),
            ("CITATIONS_IDCONV_ENDPOINT".to_string(), idconv),
            ("CITATIONS_CROSSREF_ENDPOINT".to_string(), crossref),
            ("CITATIONS_ALLOW_LOOPBACK".to_string(), "1".to_string()),
        ],
        ..Default::default()
    })
    .await;

    let user = create_user_with_permissions(&server, "ac_verify", &["*"]).await;

    // Wait for the citations built-in row, then grant the default group access.
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

    // Bridge-backed tool-capable model.
    let provider: Value = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": format!("V {}", &Uuid::new_v4().to_string()[..8]),
            "provider_type": "custom", "enabled": true, "api_key": key, "base_url": base,
        }))
        .send().await.unwrap().json().await.unwrap();
    let model: Value = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "provider_id": provider["id"], "name": model_name, "display_name": "V Qwen",
            "enabled": true, "engine_type": "none", "file_format": "gguf",
            "capabilities": { "chat": true, "completion": true, "tools": true, "embedding": false }
        }))
        .send().await.unwrap().json().await.unwrap();
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);
    crate::chat::helpers::ensure_user_has_model_access(&server, &user.user_id, &model).await;

    let conv = crate::chat::helpers::create_conversation(&server, &user.token, Some(model_id), None).await;
    let conv_id = crate::chat::helpers::parse_uuid(&conv["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conv["active_branch_id"]);

    // A fabricated DOI the resolver has never heard of → 404 → not_found.
    let fabricated = "10.9999/this-paper-does-not-exist-zx42";
    let payload = json!({
        "content": format!(
            "Use the verify_citations tool to check whether DOI {fabricated} resolves \
             to a real record. You MUST call the tool — do not answer from memory."),
        "model_id": model_id.to_string(),
        "branch_id": branch_id.to_string(),
        "enable_mcp": true,
        "mcp_config": { "mcp_servers": [ { "server_id": cit_id.to_string(), "tools": [] } ] }
    });
    let events = crate::chat::helpers::send_body_and_collect_events(
        &server, &user.token, conv_id, payload, &["complete"],
    )
    .await;

    // The agent-core loop actually invoked a citations tool.
    assert!(
        events.iter().any(|e| e.event == "mcpToolStart"),
        "the model must call a citations tool on the agent-core path (no mcpToolStart)"
    );

    // DETERMINISTIC anchor: the fabricated DOI is NOT invented — it lands `not_found`
    // in the persisted transcript (the resolver 404s, so verify_citations classifies
    // not_found). A fabricated hit would have shown `verified`/a real title instead.
    let history = crate::chat::helpers::get_conversation_history(&server, &user.token, conv_id).await;
    let dump = history.to_string();
    assert!(
        dump.contains("not_found"),
        "the fabricated DOI must surface not_found (never invented) in the transcript; got: {dump}"
    );

    // And it was NOT persisted as a real library entry (nothing to invent).
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM bibliography_entries WHERE user_id = $1 AND doi = $2",
    )
    .bind(Uuid::parse_str(&user.user_id).unwrap())
    .bind(fabricated)
    .fetch_one(&pool)
    .await
    .unwrap();
    pool.close().await;
    assert_eq!(count, 0, "a fabricated DOI must never be stored as a real entry");
}
