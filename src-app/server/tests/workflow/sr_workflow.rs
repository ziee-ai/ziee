//! Deterministic end-to-end runs of the vendored SR systematic-review workflows:
//! dev-import the REAL seed `workflow.yaml`, mock every llm/llm_map/tool step, and
//! let the two `elicit` gates genuinely SUSPEND (durable, `timeout_ms: 0`) — then
//! submit each to resume. No live LLM / network — a CI-safe regression net for the
//! SR DAG, the auto-expansion rounds, the durable double-gate, and the output
//! wiring the frontend bridge (`literature/workflowBridge.ts`) consumes.

use std::time::Duration;

use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    import_dev_workflow, plain_server, poll_run, run_workflow, stub_conversation, workflow_user,
};

/// The vendored `sr-review` source — the committed `workflow.yaml` (the seed stores
/// these as source; `build.rs` packs them at build). The test imports the exact
/// definition that ships.
const SR_REVIEW_YAML: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/hub-seed/workflows/io.github.ziee/sr-review/workflow.yaml"
));

const SR_SNOWBALL_SCREEN_YAML: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/hub-seed/workflows/io.github.ziee/sr-snowball-screen/workflow.yaml"
));

/// Poll `GET /workflow-runs/{id}` until `status == "waiting"` (parked on a durable
/// gate) with a pending `elicitation_id` that is NOT `exclude`, returning that id.
/// `exclude` guards the resume race: after submitting gate N, the run briefly
/// still shows gate N's `waiting` state before `resume_run` advances to gate N+1,
/// so polling for gate N+1 must skip the stale prior id. Panics on timeout or an
/// early terminal status.
async fn poll_until_waiting(
    server: &super::TestServer,
    token: &str,
    run_id: Uuid,
    exclude: Option<Uuid>,
) -> Uuid {
    for _ in 0..100 {
        let resp = reqwest::Client::new()
            .get(server.api_url(&format!("/workflow-runs/{run_id}")))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .expect("get run");
        let run: Value = resp.json().await.expect("run json");
        match run["status"].as_str().unwrap_or("") {
            "waiting" => {
                let eid = run["pending_elicitation_json"]["elicitation_id"]
                    .as_str()
                    .expect("waiting run must carry a pending elicitation_id");
                let eid = Uuid::parse_str(eid).expect("elicitation_id uuid");
                if exclude != Some(eid) {
                    return eid;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            s @ ("failed" | "cancelled" | "completed") => {
                panic!("run {run_id} reached terminal '{s}' before parking: {run}")
            }
            _ => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
    panic!("run {run_id} never reached a fresh `waiting` gate");
}

/// Submit an elicitation response to a (possibly cold/`waiting`) run.
async fn submit_elicit(
    server: &super::TestServer,
    token: &str,
    run_id: Uuid,
    elicitation_id: Uuid,
    response: Value,
) -> reqwest::Response {
    reqwest::Client::new()
        .post(server.api_url(&format!("/workflow-runs/{run_id}/elicit/{elicitation_id}")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "response": response }))
        .send()
        .await
        .expect("submit elicit")
}

/// GET one completed step's full output value.
async fn read_step_output(
    server: &super::TestServer,
    token: &str,
    run_id: Uuid,
    step: &str,
) -> Value {
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}/output/{step}")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("read step output");
    assert_eq!(resp.status(), 200, "output endpoint 200 for completed step `{step}`");
    let body = resp.text().await.expect("output body");
    serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("step `{step}` output is JSON: {e}; body={body}"))
}

/// GET one completed step's output as RAW TEXT (for `output_format: text` steps
/// like `synthesize`, whose output is markdown, not JSON).
async fn read_step_output_text(
    server: &super::TestServer,
    token: &str,
    run_id: Uuid,
    step: &str,
) -> String {
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}/output/{step}")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("read step output");
    assert_eq!(resp.status(), 200, "output endpoint 200 for completed step `{step}`");
    resp.text().await.expect("output body")
}

fn record(doi: &str, title: &str, source: &str) -> Value {
    json!({
        "doi": doi, "pmid": null, "title": title, "abstract_text": "An abstract.",
        "authors": ["A B"], "year": 2021, "venue": "Nature", "url": null,
        "source": source, "source_ids": [format!("{source}:1")],
        "cited_by_count": 3, "is_preprint": false, "relevance": 0.9
    })
}

fn agg(records: Vec<Value>, identified: Value) -> Value {
    let n = records.len();
    json!({
        "query": "q", "records": records, "identified": identified,
        "after_dedup": n, "degraded_sources": [], "completeness": null
    })
}

/// Full sr-review run: seed search → 3 auto-expansion rounds (round 1 ADDS, rounds
/// 2-3 EARLY-STOP with empty lists) → dedup → AI screen → DURABLE screening gate
/// (suspend → submit included_ids → resume) → full-text → extract → DURABLE review
/// gate (suspend → submit → resume) → synthesize. Asserts the run completes and the
/// declared outputs surface.
#[tokio::test]
async fn sr_review_runs_through_both_durable_gates() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_sr_review_user").await;
    let wf = import_dev_workflow(&server, &user.token, "sr-review", SR_REVIEW_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();
    // A stub model + conversation so spawn_run can snapshot a model (never invoked
    // — every dispatched non-gate step is mocked).
    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;

    // Mock every non-gate step by id. The two `elicit` gates (screen_review,
    // review) are NOT mocked → they genuinely suspend (durable, timeout_ms: 0).
    let mocks = json!({
        "search0": agg(vec![record("10.1/x", "Base editing", "europepmc"),
                            record("10.2/y", "Off-target", "crossref")],
                       json!({"europepmc": 1, "crossref": 1})),
        // Round 1 ADDS: non-empty expansion → esearch1/esnow1 contribute records.
        "expand1": {"stop": false, "new_queries": ["base editor fidelity"], "snowball_seed_ids": ["10.1/x"]},
        "esearch1": agg(vec![record("10.3/z", "Editor fidelity", "pubmed")], json!({"pubmed": 1})),
        "esnow1": agg(vec![record("10.4/w", "A cited work", "semanticscholar")], json!({"semanticscholar": 1})),
        // Rounds 2-3 EARLY-STOP: stop=true + empty lists → no-op rounds.
        "expand2": {"stop": true, "new_queries": [], "snowball_seed_ids": []},
        "esearch2": agg(vec![], json!({})),
        "esnow2": agg(vec![], json!({})),
        "expand3": {"stop": true, "new_queries": [], "snowball_seed_ids": []},
        "esearch3": agg(vec![], json!({})),
        "esnow3": agg(vec![], json!({})),
        // Merged, deduped union of all rounds (4 distinct records).
        "dedup_all": agg(vec![record("10.1/x", "Base editing", "europepmc"),
                              record("10.2/y", "Off-target", "crossref"),
                              record("10.3/z", "Editor fidelity", "pubmed"),
                              record("10.4/w", "A cited work", "semanticscholar")],
                         json!({"europepmc": 1, "crossref": 1, "pubmed": 1, "semanticscholar": 1})),
        "screen": [
            {"id":"10.1/x","decision":"include","reason":"on-topic","confidence":0.9},
            {"id":"10.2/y","decision":"exclude","reason":"off-target focus","confidence":0.6},
            {"id":"10.3/z","decision":"include","reason":"on-topic","confidence":0.8},
            {"id":"10.4/w","decision":"exclude","reason":"tangential","confidence":0.5}
        ],
        // Full text for the human-included set (10.1/x, 10.3/z).
        "fetch": {"papers": [
            {"id":"10.1/x","status":"full_text","text":"Full text of paper X."},
            {"id":"10.3/z","status":"full_text","text":"Full text of paper Z."}
        ]},
        "extract": [
            {"id":"10.1/x","population":"P","intervention":"I","comparator":"C","outcome":"O","effect":"E","risk_of_bias":"low","confidence":0.8,"quote":"Full text of paper X."},
            {"id":"10.3/z","population":"P2","intervention":"I2","comparator":"C2","outcome":"O2","effect":"E2","risk_of_bias":"some","confidence":0.7,"quote":"Full text of paper Z."}
        ],
        "synthesize": "## Synthesis\n\nThe evidence [10.1/x] [10.3/z] suggests X.\n\n**Limitations**: AI-assisted, requires human verification.\n\n1. 10.1/x\n2. 10.3/z\n"
    });

    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({ "inputs": { "query": "crispr base editing" }, "conversation_id": conv_id.to_string(), "mocks": mocks }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();

    // ── Gate 1 (screening): the run suspends durably; submit the included set. ──
    let eid1 = poll_until_waiting(&server, &user.token, run_id, None).await;
    let r1 = submit_elicit(
        &server,
        &user.token,
        run_id,
        eid1,
        json!({ "included_ids": ["10.1/x", "10.3/z"], "approved": true }),
    )
    .await;
    assert_eq!(r1.status(), 200, "screening-gate submit must 200");

    // ── Gate 2 (extraction review): suspends again (skip gate-1's stale id);
    //    submit the approved table. ──
    let eid2 = poll_until_waiting(&server, &user.token, run_id, Some(eid1)).await;
    let r2 = submit_elicit(
        &server,
        &user.token,
        run_id,
        eid2,
        json!({
            "approved": true,
            "extractions": [
                {"id":"10.1/x","population":"P","intervention":"I","comparator":"C","outcome":"O","effect":"E","risk_of_bias":"low","confidence":0.8,"quote":"Full text of paper X."},
                {"id":"10.3/z","population":"P2","intervention":"I2","comparator":"C2","outcome":"O2","effect":"E2","risk_of_bias":"some","confidence":0.7,"quote":"Full text of paper Z."}
            ]
        }),
    )
    .await;
    assert_eq!(r2.status(), 200, "review-gate submit must 200");

    // ── Completion + output wiring. ──
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "sr-review run should complete; got: {final_run}");

    // The frontend bridge keys off step_outputs_json (candidate step `dedup_all` +
    // `screen`) before calling readOutput.
    let so = &final_run["step_outputs_json"];
    assert!(
        so.get("dedup_all").is_some() && so.get("screen").is_some(),
        "step_outputs_json carries the bridge's keying source: {so}"
    );
    // final_output_json carries PREVIEW wrappers per declared output.
    assert!(
        final_run["final_output_json"]["candidates"]["value_preview"].is_string(),
        "candidates output is a preview wrapper: {}",
        final_run["final_output_json"]
    );

    // Full values on disk.
    let candidates = read_step_output(&server, &user.token, run_id, "dedup_all").await;
    assert_eq!(
        candidates["records"].as_array().map(|r| r.len()),
        Some(4),
        "all 4 deduped records surfaced: {candidates}"
    );
    let screening = read_step_output(&server, &user.token, run_id, "screen").await;
    assert_eq!(screening.as_array().map(|a| a.len()), Some(4), "one AI decision per record");

    // The elicit gates' outputs = the submitted responses.
    let included = read_step_output(&server, &user.token, run_id, "screen_review").await;
    assert_eq!(
        included["included_ids"],
        json!(["10.1/x", "10.3/z"]),
        "the human-finalized included set is the gate output: {included}"
    );
    let report = read_step_output_text(&server, &user.token, run_id, "synthesize").await;
    assert!(report.contains("Limitations"), "synthesis markdown surfaced: {report}");
    let review = read_step_output(&server, &user.token, run_id, "review").await;
    assert_eq!(
        review["extractions"].as_array().map(|a| a.len()),
        Some(2),
        "reviewer-approved extraction table surfaced: {review}"
    );
}

/// Deterministic snowball run (kept as a separate re-runnable workflow): the
/// `snowball` (tool: fetch_references) step is mocked with a canned AggregateResult,
/// `screen` (llm_map) with canned decisions; the bridge consumes the identical
/// `candidates`/`ai_screening` outputs (it keys off either `search` or `snowball`).
#[tokio::test]
async fn sr_snowball_screen_runs_and_surfaces_screening_outputs() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_sr_snow_user").await;
    let wf =
        import_dev_workflow(&server, &user.token, "sr-snowball-screen", SR_SNOWBALL_SCREEN_YAML)
            .await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();
    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;

    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({
            "inputs": { "seed_ids": ["10.1/included"], "direction": "backward" },
            "conversation_id": conv_id.to_string(),
            "mocks": {
                "snowball": {
                    "query": "cited-by references of 1 paper(s)",
                    "records": [
                        {"doi":"10.9/cited-a","pmid":null,"title":"A cited work","abstract_text":"x","authors":["A B"],"year":2019,"venue":"Nature","url":null,"source":"semanticscholar","source_ids":["semanticscholar:1"],"cited_by_count":12,"is_preprint":false,"relevance":0.8},
                        {"doi":"10.9/cited-b","pmid":"888","title":"Another cited work","abstract_text":"y","authors":["C D"],"year":2020,"venue":null,"url":null,"source":"semanticscholar","source_ids":["semanticscholar:2"],"cited_by_count":3,"is_preprint":false,"relevance":0.6}
                    ],
                    "identified": {"semanticscholar": 2},
                    "after_dedup": 2,
                    "degraded_sources": [],
                    "completeness": null
                },
                "screen": [
                    {"id":"10.9/cited-a","decision":"include","reason":"on-topic","confidence":0.85},
                    {"id":"10.9/cited-b","decision":"exclude","reason":"out of scope","confidence":0.5}
                ]
            }
        }),
    )
    .await;

    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "SR snowball-screen run should complete; got: {final_run}"
    );
    let so = &final_run["step_outputs_json"];
    assert!(
        so.get("snowball").is_some() && so.get("screen").is_some(),
        "step_outputs_json carries the bridge's keying source: {so}"
    );

    let candidates = read_step_output(&server, &user.token, run_id, "snowball").await;
    let recs = candidates["records"]
        .as_array()
        .unwrap_or_else(|| panic!("snowball output has records[]: {candidates}"));
    assert_eq!(recs.len(), 2, "both snowballed records surfaced");
    assert_eq!(candidates["after_dedup"], 2);

    let screening = read_step_output(&server, &user.token, run_id, "screen").await;
    let ai = screening
        .as_array()
        .unwrap_or_else(|| panic!("screen output is a decisions array: {screening}"));
    assert_eq!(ai.len(), 2, "one AI decision per record");
    assert_eq!(ai[0]["decision"], "include");
    assert_eq!(ai[1]["decision"], "exclude");
}
