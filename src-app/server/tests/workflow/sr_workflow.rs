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

// A minimal workflow whose ONLY step is a REAL lit_search tool call (not mocked).
const REAL_LIT_SEARCH_WORKFLOW: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: query
    required: true
steps:
  - id: search
    kind: tool
    message: "Searching for {{ inputs.query }}"
    server: lit_search
    tool: literature_search
    arguments:
      query: "{{ inputs.query }}"
outputs:
  - name: results
    from: "{{ search.output }}"
    expose: full
"#;

/// PROVES a workflow `tool` step ACTUALLY invokes the lit_search MCP server and
/// searches — this is NOT mocked. The `search` step runs the real path
/// (ToolDispatcher → the `lit_search` built-in → the connectors), with the
/// europepmc + crossref connectors pointed at loopback MOCK UPSTREAMS (the same
/// `LIT_SEARCH_*_ENDPOINT` seams + mock servers the direct `literature_search` MCP
/// test uses). The step output must carry the UNION-deduped result (4 raw records
/// → 3 after the shared DOI collapses) with per-source `identified` counts — which
/// can ONLY appear if the workflow genuinely called out to search and ran the real
/// aggregate→dedup pipeline. (The durable-gate test above mocks the tool steps to
/// isolate the gate mechanics; THIS test covers the real tool→MCP→search path.)
#[tokio::test]
async fn tool_step_really_calls_lit_search_mcp_and_searches() {
    use crate::common::TestServerOptions;
    use crate::common::test_helpers::create_user_with_permissions;
    use crate::lit_search::{configure, start_mock_crossref, start_mock_europepmc};

    // Mock upstreams FIRST — their ports go into the endpoint seams.
    let epmc = start_mock_europepmc().await;
    let crossref = start_mock_crossref().await;
    let server = crate::common::TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            ("LIT_SEARCH_ALLOW_LOOPBACK".to_string(), "1".to_string()),
            ("LIT_SEARCH_EUROPEPMC_ENDPOINT".to_string(), format!("{epmc}/search")),
            ("LIT_SEARCH_CROSSREF_ENDPOINT".to_string(), format!("{crossref}/works")),
        ],
        ..Default::default()
    })
    .await;

    // One user: enable lit_search (admin), call the tool (use), and run workflows.
    let user = create_user_with_permissions(
        &server,
        "wf_real_litsearch",
        &[
            "workflows::read",
            "workflows::install",
            "workflows::manage",
            "workflows::execute",
            "lit_search::use",
            "lit_search::admin::read",
            "lit_search::admin::manage",
        ],
    )
    .await;
    configure(&server, &user.token, &["europepmc", "crossref"]).await;

    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;
    let wf =
        import_dev_workflow(&server, &user.token, "real-litsearch", REAL_LIT_SEARCH_WORKFLOW).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();

    // NO `mocks` — the `search` step really calls lit_search → the mock upstreams.
    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({ "inputs": { "query": "crispr" }, "conversation_id": conv_id.to_string() }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "the real lit_search tool-step run should complete: {final_run}"
    );

    // The step output IS the real union-deduped search result from the mock
    // upstreams (mirrors the direct literature_search MCP test's assertions).
    let out = read_step_output(&server, &user.token, run_id, "search").await;
    assert_eq!(out["identified"]["europepmc"], 2, "europepmc upstream was queried: {out}");
    assert_eq!(out["identified"]["crossref"], 2, "crossref upstream was queried: {out}");
    assert_eq!(out["after_dedup"], 3, "the shared DOI collapsed 4→3 via real dedup: {out}");
    assert_eq!(
        out["records"].as_array().map(|r| r.len()),
        Some(3),
        "3 deduped records reached the workflow output: {out}"
    );
}

const REAL_SNOWBALL_WORKFLOW: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: seed_ids
    required: true
steps:
  - id: snowball
    kind: tool
    message: "Snowballing references"
    server: lit_search
    tool: fetch_references
    arguments:
      ids: "{{ inputs.seed_ids }}"
      direction: backward
outputs:
  - name: results
    from: "{{ snowball.output }}"
    expose: full
"#;

/// PROVES a workflow `tool` step REALLY calls lit_search `fetch_references`
/// (citation snowballing via Semantic Scholar) — not mocked. The S2 paper-graph is
/// a loopback MOCK upstream (`LIT_SEARCH_S2_PAPER_ENDPOINT`); the step output must
/// carry the CITED work (10.9/cited) and NOT the citing paper (backward direction)
/// — which only the real S2 fetch + dedup path produces.
#[tokio::test]
async fn tool_step_really_calls_lit_search_fetch_references() {
    use crate::common::TestServerOptions;
    use crate::common::test_helpers::create_user_with_permissions;
    use crate::lit_search::{configure, start_mock_s2_paper};

    let s2 = start_mock_s2_paper().await;
    let server = crate::common::TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            ("LIT_SEARCH_ALLOW_LOOPBACK".to_string(), "1".to_string()),
            ("LIT_SEARCH_S2_PAPER_ENDPOINT".to_string(), s2),
        ],
        ..Default::default()
    })
    .await;
    let user = create_user_with_permissions(
        &server,
        "wf_real_snowball",
        &[
            "workflows::read",
            "workflows::install",
            "workflows::manage",
            "workflows::execute",
            "lit_search::use",
            "lit_search::admin::read",
            "lit_search::admin::manage",
        ],
    )
    .await;
    configure(&server, &user.token, &["semanticscholar"]).await;
    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;
    let wf =
        import_dev_workflow(&server, &user.token, "real-snowball", REAL_SNOWBALL_WORKFLOW).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();

    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({ "inputs": { "seed_ids": ["10.1234/seed"] }, "conversation_id": conv_id.to_string() }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "the real fetch_references run should complete: {final_run}"
    );

    let out = read_step_output(&server, &user.token, run_id, "snowball").await;
    let recs = out["records"].as_array().expect("records array");
    assert!(
        recs.iter().any(|r| r["doi"] == "10.9/cited"),
        "backward snowball returned the cited reference via the real S2 fetch: {out}"
    );
    assert!(
        !recs.iter().any(|r| r["doi"] == "10.9/citing"),
        "backward direction must exclude the citing paper: {out}"
    );
}

const REAL_FULLTEXT_WORKFLOW: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: ids
    required: true
steps:
  - id: fetch
    kind: tool
    message: "Fetching full text"
    server: lit_search
    tool: fetch_paper_fulltext
    arguments:
      ids: "{{ inputs.ids }}"
outputs:
  - name: results
    from: "{{ fetch.output }}"
    expose: full
"#;

/// PROVES a workflow `tool` step REALLY calls lit_search `fetch_paper_fulltext`
/// and resolves open-access full text — not mocked. Europe PMC's fullTextXML is a
/// loopback MOCK upstream (`LIT_SEARCH_EUROPEPMC_FULLTEXT_ENDPOINT`); the run is
/// conversation-bound, so the tool resolves the OA text and the step output reports
/// `status: full_text` with a non-empty char count — only the real resolve+extract
/// path yields that.
#[tokio::test]
async fn tool_step_really_calls_lit_search_fetch_fulltext() {
    use std::sync::atomic::Ordering;

    use crate::common::TestServerOptions;
    use crate::common::test_helpers::create_user_with_permissions;
    use crate::lit_search::{configure, start_mock_epmc_fulltext};

    let (epmc, hits) = start_mock_epmc_fulltext().await;
    let server = crate::common::TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            ("LIT_SEARCH_ALLOW_LOOPBACK".to_string(), "1".to_string()),
            ("LIT_SEARCH_EUROPEPMC_FULLTEXT_ENDPOINT".to_string(), epmc),
        ],
        ..Default::default()
    })
    .await;
    let user = create_user_with_permissions(
        &server,
        "wf_real_fulltext",
        &[
            "workflows::read",
            "workflows::install",
            "workflows::manage",
            "workflows::execute",
            "lit_search::use",
            "lit_search::admin::read",
            "lit_search::admin::manage",
        ],
    )
    .await;
    configure(&server, &user.token, &["europepmc"]).await;
    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;
    let wf =
        import_dev_workflow(&server, &user.token, "real-fulltext", REAL_FULLTEXT_WORKFLOW).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();

    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({ "inputs": { "ids": ["PMC123456"] }, "conversation_id": conv_id.to_string() }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "the real fetch_paper_fulltext run should complete: {final_run}"
    );

    let out = read_step_output(&server, &user.token, run_id, "fetch").await;
    let paper = &out["papers"][0];
    assert_eq!(paper["status"], "full_text", "real OA full text resolved: {out}");
    assert_eq!(paper["source"], "europepmc", "resolved from the (mock) europepmc upstream: {out}");
    assert!(
        paper["chars"].as_u64().unwrap_or(0) > 0,
        "non-empty extracted text reached the workflow output: {out}"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1, "the workflow actually hit the upstream once");
}
