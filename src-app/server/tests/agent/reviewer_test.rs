//! TEST-22 — the reviewer (ITEM-12): a mutating tool call under a headless
//! (`OnRequest`) policy is routed to the reviewer; a `High` classification
//! ESCALATES to the durable human gate (the run parks `waiting`), and once the
//! call executes on approval the classification PERSISTS to
//! `mcp_tool_calls.review_classification`. Made deterministic via the debug-only
//! `ZIEE_AGENT_FORCE_RISK=high` seam (the reviewer is model-driven in prod;
//! forcing the class removes the model-nondeterminism while exercising the real
//! reviewer→gate→journal wiring). Bridge-gated for the tool-call emission.

use std::time::{Duration, Instant};

use serde_json::{json, Value};
use uuid::Uuid;

use crate::chat::helpers::create_conversation;
use crate::common::test_helpers::create_user_with_permissions;
use crate::common::{TestServer, TestServerOptions};
use crate::mcp::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};
use crate::workflow::{db_pool, import_dev_workflow, poll_run, register_mock_as_user_server, run_workflow};

const AGENT_TOOL_YAML: &str = r#"inputs:
  - name: q
    required: true
steps:
  - id: act
    kind: agent
    prompt: "Use the echo tool to echo the text '{{ inputs.q }}', then report what it returned."
    servers: ["review_mock"]
"#;

async fn poll_until_waiting(server: &TestServer, token: &str, run_id: Uuid) -> Uuid {
    let deadline = Instant::now() + Duration::from_secs(40);
    loop {
        let run: Value = reqwest::Client::new()
            .get(server.api_url(&format!("/workflow-runs/{run_id}")))
            .header("Authorization", format!("Bearer {token}"))
            .send().await.unwrap().json().await.unwrap();
        match run["status"].as_str().unwrap_or("") {
            "waiting" => {
                return Uuid::parse_str(
                    run["pending_elicitation_json"]["elicitation_id"].as_str().unwrap(),
                )
                .unwrap();
            }
            s @ ("failed" | "cancelled" | "completed") => {
                panic!("run {run_id} reached terminal '{s}' before parking: {run}")
            }
            _ => {}
        }
        assert!(Instant::now() < deadline, "run {run_id} never parked: {run}");
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

#[tokio::test]
async fn reviewer_high_escalates_to_gate_and_persists_classification() {
    let base = match std::env::var("ZIEE_TEST_LLM_BASE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!("SKIP reviewer_high_escalates — ZIEE_TEST_LLM_BASE_URL unset");
            return;
        }
    };
    let key = std::env::var("ZIEE_TEST_LLM_KEY").unwrap_or_else(|_| "sk-local-audit".into());
    let model_name = std::env::var("ZIEE_TEST_LLM_MODEL").unwrap_or_else(|_| "qwen3.6-35b-a3b".into());

    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("ZIEE_AGENT_FORCE_RISK".to_string(), "high".to_string())],
        ..Default::default()
    })
    .await;
    let user = create_user_with_permissions(&server, "wf_agent_review", &["*"]).await;

    let provider: Value = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": format!("RV {}", &Uuid::new_v4().to_string()[..8]),
            "provider_type": "custom", "enabled": true, "api_key": key, "base_url": base,
        }))
        .send().await.unwrap().json().await.unwrap();
    let model: Value = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "provider_id": provider["id"], "name": model_name, "display_name": "RV Qwen",
            "enabled": true, "engine_type": "none", "file_format": "gguf",
            "capabilities": { "chat": true, "completion": true, "tools": true, "embedding": false }
        }))
        .send().await.unwrap().json().await.unwrap();
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();
    crate::chat::helpers::ensure_user_has_model_access(&server, &user.user_id, &model).await;
    let conv = create_conversation(&server, &user.token, Some(model_id), Some("review")).await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();

    let mock = MockMcpServer::start().await;
    for _ in 0..50 {
        mock.on_method("tools/list", MockResponse::JsonOk(json!({
            "tools": [ { "name": "echo", "description": "echo text",
                "inputSchema": { "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"] } } ]
        })));
    }
    for _ in 0..20 {
        mock.on_method("tools/call", MockResponse::JsonOk(json!({
            "content": [ { "type": "text", "text": "ECHO: review-9" } ], "isError": false,
        })));
    }
    let _mcp_id = register_mock_as_user_server(&server, &user.token, "review_mock", &mock.base_url()).await;

    let wf = import_dev_workflow(&server, &user.token, "agent-review", AGENT_TOOL_YAML).await;
    let wf_id = wf["id"].as_str().unwrap().to_string();
    let run = run_workflow(&server, &user.token, &wf_id, json!({
        "inputs": { "q": "review-9" }, "conversation_id": conv_id.to_string(),
    })).await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();

    // ── High → the mutating call ESCALATES to the durable gate (run parks). ──
    let eid = poll_until_waiting(&server, &user.token, run_id).await;
    let pool = db_pool(&server).await;
    let pending: Value =
        sqlx::query_scalar("SELECT pending_elicitation_json FROM workflow_runs WHERE id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        pending["data"]["tool"].as_str(),
        Some("review_mock__echo"),
        "the reviewer must escalate the mutating echo call to the human gate: {pending}"
    );

    // ── Approve (for the session) → resume → the call executes → completes. ──
    let status = reqwest::Client::new()
        .post(server.api_url(&format!("/workflow-runs/{run_id}/elicit/{eid}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "response": { "approve": true, "approve_for_session": true } }))
        .send().await.unwrap().status();
    assert_eq!(status, 200, "approve must 200");
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "resumed run completes: {final_run}");

    // ── The forced-High classification PERSISTS onto the executed journal row. ──
    let class: Option<String> = sqlx::query_scalar(
        "SELECT review_classification FROM mcp_tool_calls \
         WHERE conversation_id = $1 AND tool_name = 'echo' AND review_classification IS NOT NULL \
         ORDER BY created_at ASC LIMIT 1",
    )
    .bind(conv_id)
    .fetch_optional(&pool)
    .await
    .unwrap()
    .flatten();
    pool.close().await;
    assert_eq!(
        class.as_deref(),
        Some("high"),
        "the reviewer's `high` classification must persist to mcp_tool_calls.review_classification"
    );
}
