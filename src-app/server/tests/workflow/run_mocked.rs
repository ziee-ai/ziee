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

// audit id all-428538b56261 — concurrent multi-user workflow runs. The existing
// workflow_mcp tests are single-user, and resources_test only covers cross-user
// isolation on READ of an already-finished run. This runs TWO users' workflows
// CONCURRENTLY (mocked, deterministic) and asserts each run completes and its
// output carries only ITS OWN marker — no cross-run/user bleed under parallel
// execution in the runner.
#[tokio::test]
async fn concurrent_multi_user_runs_are_isolated() {
    let server = plain_server().await;
    let user_a = workflow_user(&server, "wf_cc_a").await;
    let user_b = workflow_user(&server, "wf_cc_b").await;

    // Each user dev-imports the fixture workflow + gets a stub conversation.
    let wf_a = import_dev_workflow(&server, &user_a.token, "cc-a", FIXTURE_WORKFLOW_YAML).await;
    let wf_a_id = wf_a["id"].as_str().unwrap().to_string();
    let (_stub_a, conv_a) = stub_conversation(&server, &user_a.user_id, &user_a.token).await;

    let wf_b = import_dev_workflow(&server, &user_b.token, "cc-b", FIXTURE_WORKFLOW_YAML).await;
    let wf_b_id = wf_b["id"].as_str().unwrap().to_string();
    let (_stub_b, conv_b) = stub_conversation(&server, &user_b.user_id, &user_b.token).await;

    let mocks = |marker: &str| {
        json!({
            "inputs": { "topic": "concurrency" },
            "conversation_id": null,
            "mocks": {
                "research": [{"title": "T", "url": "https://example.com/t"}],
                "summarize": ["s"],
                "write": format!("MEMO_BODY_MARKER: {marker}")
            }
        })
    };
    let mut body_a = mocks("RUN_A_MARKER");
    body_a["conversation_id"] = json!(conv_a.to_string());
    let mut body_b = mocks("RUN_B_MARKER");
    body_b["conversation_id"] = json!(conv_b.to_string());

    // Dispatch BOTH runs concurrently (they execute in parallel in the runner).
    let (run_a, run_b) = tokio::join!(
        run_workflow(&server, &user_a.token, &wf_a_id, body_a),
        run_workflow(&server, &user_b.token, &wf_b_id, body_b),
    );
    let run_a_id = Uuid::parse_str(run_a["run_id"].as_str().unwrap()).unwrap();
    let run_b_id = Uuid::parse_str(run_b["run_id"].as_str().unwrap()).unwrap();

    let final_a = poll_run(&server, &user_a.token, run_a_id).await;
    let final_b = poll_run(&server, &user_b.token, run_b_id).await;
    assert_eq!(final_a["status"], "completed", "run A: {final_a}");
    assert_eq!(final_b["status"], "completed", "run B: {final_b}");

    // Each run's write output carries ONLY its own marker — no cross-bleed.
    let read_write = |token: String, run_id: Uuid| {
        let url = server.api_url(&format!("/workflow-runs/{run_id}/output/write"));
        async move {
            reqwest::Client::new()
                .get(&url)
                .header("Authorization", format!("Bearer {token}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        }
    };
    let out_a = read_write(user_a.token.clone(), run_a_id).await;
    let out_b = read_write(user_b.token.clone(), run_b_id).await;
    assert!(out_a.contains("RUN_A_MARKER") && !out_a.contains("RUN_B_MARKER"), "run A output isolated: {out_a}");
    assert!(out_b.contains("RUN_B_MARKER") && !out_b.contains("RUN_A_MARKER"), "run B output isolated: {out_b}");
}
