//! `POST /workflows/validate` (valid + invalid) and
//! `POST /workflows/{id}/dry-run` structured responses.
//!
//! - validate (valid) → `{valid: true, steps: 3, ...}`.
//! - validate (cycle) → `{valid: false, errors: non-empty}`.
//! - dry-run → per-step `{step_id, kind, est_calls, ...}` + totals.

use serde_json::json;

use super::{FIXTURE_WORKFLOW_YAML, import_dev_workflow, plain_server, workflow_user};

/// A workflow with a `depends_on` cycle (a → b → a). The validator's
/// cycle-check must reject it with a non-empty errors list.
const CYCLIC_WORKFLOW_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: true
steps:
  - id: a
    kind: llm
    prompt: "step a {{ b.output }}"
    depends_on: [b]
  - id: b
    kind: llm
    prompt: "step b {{ a.output }}"
    depends_on: [a]
outputs:
  - name: result
    from: "{{ a.output }}"
"#;

#[tokio::test]
async fn validate_accepts_valid_workflow() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_validate_ok").await;

    let body: serde_json::Value = reqwest::Client::new()
        .post(server.api_url("/workflows/validate"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "workflow_yaml": FIXTURE_WORKFLOW_YAML }))
        .send()
        .await
        .expect("validate")
        .json()
        .await
        .expect("parse validate");

    assert_eq!(body["valid"], true, "valid workflow validates: {body}");
    assert_eq!(body["steps"], 3, "3 steps reported: {body}");
    assert!(
        body["errors"].as_array().map(|a| a.is_empty()).unwrap_or(false),
        "no errors on a valid workflow: {body}"
    );
    // 3 llm steps → 3 est_max_calls.
    assert_eq!(body["est_max_calls"], 3, "est_max_calls = 3 llm steps: {body}");
}

#[tokio::test]
async fn validate_rejects_cyclic_workflow() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_validate_cycle").await;

    let body: serde_json::Value = reqwest::Client::new()
        .post(server.api_url("/workflows/validate"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "workflow_yaml": CYCLIC_WORKFLOW_YAML }))
        .send()
        .await
        .expect("validate cyclic")
        .json()
        .await
        .expect("parse validate");

    assert_eq!(body["valid"], false, "cyclic workflow is invalid: {body}");
    let errors = body["errors"].as_array().expect("errors array");
    assert!(
        !errors.is_empty(),
        "cycle detection yields a non-empty errors list: {body}"
    );
    // At least one error should reference the cycle.
    let mentions_cycle = errors.iter().any(|e| {
        let code = e["code"].as_str().unwrap_or("");
        let msg = e["message"].as_str().unwrap_or("").to_lowercase();
        code.to_lowercase().contains("cycle") || msg.contains("cycle") || msg.contains("cyclic")
    });
    assert!(
        mentions_cycle,
        "an error should reference the cycle: {body}"
    );
}

#[tokio::test]
async fn dry_run_returns_per_step_cost_structure() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_dry_run").await;

    // dry-run operates on an installed workflow; dev-import gives us one
    // without needing the mock hub.
    let wf = import_dev_workflow(&server, &user.token, "dry-run-wf", FIXTURE_WORKFLOW_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();

    let body: serde_json::Value = reqwest::Client::new()
        .post(server.api_url(&format!("/workflows/{wf_id}/dry-run")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "inputs": { "topic": "graph theory" } }))
        .send()
        .await
        .expect("dry-run")
        .json()
        .await
        .expect("parse dry-run");

    // Per-step structure: each step has step_id, kind, est_calls.
    let steps = body["steps"].as_array().expect("dry-run steps array");
    assert_eq!(steps.len(), 3, "dry-run reports all 3 steps: {body}");
    for step in steps {
        assert!(step["step_id"].is_string(), "step has step_id: {step}");
        assert!(step["kind"].is_string(), "step has kind: {step}");
        assert!(step["est_calls"].is_u64(), "step has est_calls: {step}");
        assert!(step["est_tokens_in"].is_u64(), "step has est_tokens_in: {step}");
        assert!(step["est_tokens_out"].is_u64(), "step has est_tokens_out: {step}");
    }

    // Totals present. 3 llm steps each contribute 1 call.
    assert!(body["total_est_calls"].is_u64(), "total_est_calls present: {body}");
    assert_eq!(
        body["total_est_calls"], 3,
        "3 llm steps → 3 total est_calls: {body}"
    );
    assert!(body["total_est_tokens"].is_u64(), "total_est_tokens present: {body}");
}

/// POST /workflows/{id}/test (dev fixture-runner surface). A bundle imported
/// without any `tests/*.yaml` fixtures runs zero fixtures → an all-zero
/// TestRunResponse; a non-existent workflow id is access-gated to 404. Neither
/// path had a test.
#[tokio::test]
async fn test_workflow_endpoint_no_fixtures_and_not_found() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_test_ep").await;
    let wf = import_dev_workflow(&server, &user.token, "test-ep", FIXTURE_WORKFLOW_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id");
    let client = reqwest::Client::new();

    let res = client
        .post(server.api_url(&format!("/workflows/{wf_id}/test")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "test endpoint should 200");
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["total"], 0, "no fixtures -> total 0: {body}");
    assert_eq!(body["passed"], 0);
    assert_eq!(body["failed"], 0);
    assert!(body["results"].as_array().unwrap().is_empty());

    let missing = uuid::Uuid::new_v4();
    let res = client
        .post(server.api_url(&format!("/workflows/{missing}/test")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404, "unknown workflow must 404");
}
