//! Drive a workflow run end-to-end with mocks for every llm step → no
//! live provider needed.
//!
//! Approach (documented choice): we dev-import the 3-step
//! `research → summarize → write` workflow (`is_dev=true`, so per-step
//! `mocks` are honored), then `POST /workflows/{id}/run` with a `mocks`
//! map covering all three llm steps. The runner's mock short-circuit
//! (`run_mock_step`) writes the canned value as the step output WITHOUT
//! dispatching to a provider. A stub model is still required because
//! `spawn_run` snapshots the conversation's model at run start (the
//! provider object is constructed but never invoked when all steps are
//! mocked).
//!
//! Asserts: run reaches `completed`, per-step output metadata is
//! recorded in `step_outputs_json`, output files are readable via the
//! per-step output endpoint, and `final_output_json` is populated from
//! the workflow's `outputs[]`.

use serde_json::json;
use uuid::Uuid;

use super::{
    FIXTURE_WORKFLOW_YAML, admin_and_refresh, import_dev_workflow, install_fixture_workflow,
    plain_server, poll_run, run_workflow, server_with_workflow_catalog, stub_conversation,
    workflow_user,
};

#[tokio::test]
async fn mocked_run_completes_and_writes_outputs() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_run_user").await;

    // Dev-import the 3-step workflow (is_dev=true → mocks honored).
    let wf = import_dev_workflow(&server, &user.token, "research-summarize-write", FIXTURE_WORKFLOW_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();
    assert_eq!(wf["is_dev"], true, "dev import is is_dev: {wf}");

    // A stub model + conversation so the runner can snapshot a model.
    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;

    // Run with mocks covering every llm step. `research` returns a JSON
    // array (consumed by `{{ research.output | json }}`); the others
    // return strings.
    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({
            "inputs": { "topic": "quantum entanglement" },
            "conversation_id": conv_id.to_string(),
            "mocks": {
                "research": [
                    {"title": "Mock A", "url": "https://example.com/a"},
                    {"title": "Mock B", "url": "https://example.com/b"}
                ],
                "summarize": ["entanglement is a correlation between particles"],
                "write": "MEMO_BODY_MARKER: quantum entanglement memo from mocked run"
            }
        }),
    )
    .await;

    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;

    assert_eq!(
        final_run["status"], "completed",
        "mocked run should complete; got: {final_run}"
    );

    // Per-step output metadata recorded for all three steps.
    let outputs = &final_run["step_outputs_json"];
    assert!(outputs.is_object(), "step_outputs_json is an object: {final_run}");
    for step in ["research", "summarize", "write"] {
        assert!(
            outputs.get(step).is_some(),
            "step '{step}' has output metadata: {outputs}"
        );
    }

    // final_output_json populated from outputs[] (the `memo` output).
    let final_output = &final_run["final_output_json"];
    assert!(
        final_output.is_object(),
        "final_output_json populated: {final_run}"
    );
    assert!(
        final_output.get("memo").is_some(),
        "final_output_json carries the declared `memo` output: {final_output}"
    );

    // The write step's output file is readable via the per-step output
    // endpoint, and carries the mocked body marker.
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}/output/write")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("read write output");
    assert_eq!(resp.status(), 200, "output endpoint should 200 for a completed step");
    let content = resp.text().await.expect("output content");
    assert!(
        content.contains("MEMO_BODY_MARKER"),
        "write step output file carries the mocked body: {content}"
    );
}

#[tokio::test]
async fn run_with_mocks_on_published_workflow_is_rejected() {
    // The /run handler 403s when mocks are passed against a PUBLISHED
    // (non-dev) workflow — mocks are dev-only (plan §1). This installs a
    // real published workflow from the mock hub (is_dev=false) and
    // asserts the 403, the true negative path the prior version only
    // documented via a dev-workflow positive control.
    let (server, _mock) = server_with_workflow_catalog().await;
    let admin = admin_and_refresh(&server).await;

    // Install the fixture workflow from the hub → is_dev=false (published).
    let installed = install_fixture_workflow(&server, &admin.token).await;
    let wf = &installed["workflow"];
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();
    assert_eq!(
        wf["is_dev"], false,
        "hub install must be a PUBLISHED (non-dev) workflow: {wf}"
    );

    let (_stub, conv_id) = stub_conversation(&server, &admin.user_id, &admin.token).await;

    // Mocks against a published workflow → 403 WORKFLOW_MOCKS_NOT_ALLOWED.
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/workflows/{wf_id}/run")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "inputs": { "topic": "t" },
            "conversation_id": conv_id.to_string(),
            "mocks": { "research": [], "summarize": [], "write": "x" }
        }))
        .send()
        .await
        .expect("run published wf with mocks");
    assert_eq!(
        resp.status(),
        403,
        "published workflow must reject mocks: {}",
        resp.text().await.unwrap_or_default()
    );
}
