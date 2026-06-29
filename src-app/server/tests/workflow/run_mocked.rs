use serde_json::json;
use uuid::Uuid;
use super::FIXTURE_WORKFLOW_YAML;
use super::admin_and_refresh;
use super::import_dev_workflow;
use super::install_fixture_workflow;
use super::plain_server;
use super::poll_run;
use super::run_workflow;
use super::server_with_workflow_catalog;
use super::stub_conversation;
use super::workflow_user;

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

/// Fan-in DAG: a step with MORE THAN ONE `depends_on` (two upstream roots →
/// one downstream join). The existing fixtures are all linear chains; this pins
/// that the runner waits for BOTH upstreams before the join (the join's template
/// references both outputs, so completion proves both resolved first).
const FAN_IN_WORKFLOW_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: true
steps:
  - id: source_a
    kind: llm
    prompt: "Facts about {{ inputs.topic }} from source A. Return a JSON array."
    output_format: json
  - id: source_b
    kind: llm
    prompt: "Facts about {{ inputs.topic }} from source B. Return a JSON array."
    output_format: json
  - id: merge
    kind: llm
    prompt: |
      Merge A={{ source_a.output | json }} and B={{ source_b.output | json }}.
    depends_on: [source_a, source_b]
outputs:
  - name: merged
    from: "{{ merge.output }}"
    expose: full
"#;

#[tokio::test]
async fn fan_in_dag_waits_for_all_upstreams_before_the_join_step() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_fanin_user").await;

    let wf = import_dev_workflow(&server, &user.token, "fan-in-merge", FAN_IN_WORKFLOW_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();

    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;

    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({
            "inputs": { "topic": "tardigrade biology" },
            "conversation_id": conv_id.to_string(),
            "mocks": {
                "source_a": [{"fact": "A1"}, {"fact": "A2"}],
                "source_b": [{"fact": "B1"}],
                "merge": "FAN_IN_MERGE_MARKER both sources merged"
            }
        }),
    )
    .await;

    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "the fan-in run must complete (both upstreams resolved before merge): {final_run}"
    );

    // All three steps recorded output — both upstreams AND the join.
    let outputs = &final_run["step_outputs_json"];
    for step in ["source_a", "source_b", "merge"] {
        assert!(outputs.get(step).is_some(), "step '{step}' has output: {outputs}");
    }

    // The join step's output (which references BOTH upstreams) is readable.
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}/output/merge")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("read merge output");
    assert_eq!(resp.status(), 200);
    let content = resp.text().await.expect("merge content");
    assert!(content.contains("FAN_IN_MERGE_MARKER"), "join output present: {content}");
}

