//! TEST-5 — POST /api/workflows/validate-def (the builder's live-validation feed).
//!
//! The JSON-body twin of `/validate`: it takes a posted `WorkflowDef` and returns
//! structured `{errors, warnings, cost_estimate}` with a **200** — validation
//! findings are data, never a hard 4xx. A valid def → empty `errors`; a def with a
//! dead `tools:` field on an `llm` step (WORKFLOW_DEAD_TOOLS_FIELD) → a non-empty
//! `errors` array, still 200.

use serde_json::{json, Value as Json};

use super::{plain_server, workflow_user};

#[tokio::test]
async fn validate_def_valid_and_invalid_both_200() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_validate_def").await;
    let client = reqwest::Client::new();

    // A valid 1-step llm def → 200, empty errors, cost estimate present.
    let valid = json!({
        "inputs": [{ "name": "topic", "required": true }],
        "steps": [{
            "id": "gen",
            "kind": "llm",
            "prompt": "say something about {{ inputs.topic }}"
        }],
        "outputs": [{ "name": "result", "from": "{{ gen.output }}" }]
    });
    let resp = client
        .post(server.api_url("/workflows/validate-def"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&valid)
        .send()
        .await
        .expect("validate-def valid");
    assert_eq!(resp.status(), 200, "validate-def returns 200 for a valid def");
    let body: Json = resp.json().await.expect("parse valid body");
    assert!(
        body["errors"].as_array().map(|a| a.is_empty()).unwrap_or(false),
        "a valid def has empty errors: {body}"
    );
    assert!(body["warnings"].is_array(), "warnings array present: {body}");
    // cost_estimate is a DryRunResult; a single llm step → 1 estimated call.
    assert!(
        body["cost_estimate"].is_object(),
        "cost_estimate object present: {body}"
    );
    assert_eq!(
        body["cost_estimate"]["total_est_calls"], 1,
        "one llm step → total_est_calls = 1: {body}"
    );

    // An INVALID def: dead `tools:` on an llm step. Findings are returned as data
    // with a 200 (NOT a hard 4xx).
    let invalid = json!({
        "inputs": [{ "name": "topic", "required": true }],
        "steps": [{
            "id": "gen",
            "kind": "llm",
            "prompt": "hi {{ inputs.topic }}",
            "tools": ["web_search"]
        }],
        "outputs": [{ "name": "result", "from": "{{ gen.output }}" }]
    });
    let resp = client
        .post(server.api_url("/workflows/validate-def"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&invalid)
        .send()
        .await
        .expect("validate-def invalid");
    assert_eq!(
        resp.status(),
        200,
        "validation findings are a 200 payload, not a 4xx"
    );
    let body: Json = resp.json().await.expect("parse invalid body");
    let errors = body["errors"].as_array().expect("errors array");
    assert!(
        !errors.is_empty(),
        "an invalid def yields a non-empty errors array: {body}"
    );
    assert!(
        errors
            .iter()
            .any(|e| e["code"].as_str() == Some("WORKFLOW_DEAD_TOOLS_FIELD")),
        "the dead-tools finding is surfaced by code: {body}"
    );
}
