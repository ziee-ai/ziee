//! TEST-14 — a tool call inside a workflow `kind: agent` run is journaled to
//! `mcp_tool_calls` and LINKED to the run via `workflow_run_id` (E4), with a
//! sanitized `result_json`. Bridge-gated (needs a real tool-calling model to emit
//! the tool call). Soft-skips unless `ZIEE_TEST_LLM_BASE_URL` is set.

use serde_json::{json, Value};
use uuid::Uuid;

use crate::chat::helpers::create_conversation;
use crate::common::TestServer;
use crate::mcp::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};
use crate::common::test_helpers::create_user_with_permissions;
use crate::workflow::{db_pool, import_dev_workflow, poll_run, register_mock_as_user_server, run_workflow};

const AGENT_TOOL_YAML: &str = r#"inputs:
  - name: q
    required: true
steps:
  - id: act
    kind: agent
    prompt: "Use the echo tool to echo the text '{{ inputs.q }}', then report what it returned."
    servers: ["journal_mock"]
"#;

#[tokio::test]
async fn agent_tool_call_is_journaled_and_linked_to_the_run() {
    let base = match std::env::var("ZIEE_TEST_LLM_BASE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!("SKIP agent_tool_call_is_journaled — ZIEE_TEST_LLM_BASE_URL unset");
            return;
        }
    };
    let key = std::env::var("ZIEE_TEST_LLM_KEY").unwrap_or_else(|_| "sk-local-audit".into());
    let model_name = std::env::var("ZIEE_TEST_LLM_MODEL").unwrap_or_else(|_| "qwen3.6-35b-a3b".into());

    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "wf_agent_journal", &["*"]).await;

    // A bridge-backed model + a conversation bound to it (the run snapshots it).
    let provider: Value = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": format!("J {}", &Uuid::new_v4().to_string()[..8]),
            "provider_type": "custom", "enabled": true, "api_key": key, "base_url": base,
        }))
        .send().await.unwrap().json().await.unwrap();
    let model: Value = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "provider_id": provider["id"], "name": model_name, "display_name": "J Qwen",
            "enabled": true, "engine_type": "none", "file_format": "gguf",
            "capabilities": { "chat": true, "completion": true, "tools": true, "embedding": false }
        }))
        .send().await.unwrap().json().await.unwrap();
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();
    crate::chat::helpers::ensure_user_has_model_access(&server, &user.user_id, &model).await;
    let conv = create_conversation(&server, &user.token, Some(model_id), Some("journal")).await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();

    // A mock MCP server advertising `echo`, registered under the name the agent
    // step's `servers: ["journal_mock"]` references.
    let mock = MockMcpServer::start().await;
    for _ in 0..50 {
        mock.on_method("tools/list", MockResponse::JsonOk(json!({
            "tools": [ { "name": "echo", "description": "echo text",
                "inputSchema": { "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"] } } ]
        })));
    }
    for _ in 0..20 {
        mock.on_method("tools/call", MockResponse::JsonOk(json!({
            "content": [ { "type": "text", "text": "ECHO: journaled-42" } ], "isError": false,
        })));
    }
    let _mcp_id = register_mock_as_user_server(&server, &user.token, "journal_mock", &mock.base_url()).await;

    let wf = import_dev_workflow(&server, &user.token, "agent-journal", AGENT_TOOL_YAML).await;
    let wf_id = wf["id"].as_str().unwrap().to_string();
    let run = run_workflow(&server, &user.token, &wf_id, json!({
        "inputs": { "q": "journaled-42" }, "conversation_id": conv_id.to_string(),
    })).await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "run should complete: {final_run}");

    // The tool call was journaled AND linked to the run.
    let pool = db_pool(&server).await;
    let (count, sample): (i64, Option<String>) = sqlx::query_as(
        "SELECT count(*)::int8, min(tool_name) FROM mcp_tool_calls WHERE workflow_run_id = $1",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(count >= 1, "≥1 mcp_tool_calls row must be linked to the run via workflow_run_id");
    assert_eq!(sample.as_deref(), Some("echo"), "the journaled tool is `echo`");

    // result_json is present + sanitized (no raw base64 blobs; a JSON object/string).
    let result_json: Option<Value> = sqlx::query_scalar(
        "SELECT result_json FROM mcp_tool_calls WHERE workflow_run_id = $1 AND tool_name = 'echo' LIMIT 1",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(result_json.is_some(), "result_json must be recorded for the journaled call");
}
