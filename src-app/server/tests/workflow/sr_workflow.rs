//! Deterministic end-to-end run of the vendored `sr-search-screen` systematic-
//! review workflow: dev-import the REAL seed `workflow.yaml`, mock the `search`
//! (kind: tool) + `screen` (kind: llm_map) steps, and assert the run completes
//! and surfaces the `candidates` + `ai_screening` outputs the frontend bridge
//! (`literature/workflowBridge.ts`) consumes. No live LLM / network — a CI-safe
//! regression net for the SR workflow DAG + output wiring. (The constituent
//! tool / llm_map dispatcher mechanics get real-LLM coverage in `tool_step.rs`
//! and `real_stack.rs`; a gated real-LLM SR variant just swaps these mocks for a
//! tool-capable model + the lit_search endpoint seam.)

use serde_json::json;
use uuid::Uuid;

use super::{
    import_dev_workflow, plain_server, poll_run, run_workflow, stub_conversation, workflow_user,
};

/// The vendored seed workflow.yaml — the test runs the SHIPPED definition, so a
/// drift between the bundle and this assertion is caught.
const SR_SEARCH_SCREEN_YAML: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/hub-seed/workflows/io.github.ziee/sr-search-screen/workflow.yaml"
));

#[tokio::test]
async fn sr_search_screen_runs_and_surfaces_screening_outputs() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_sr_user").await;

    // Dev-import (is_dev=true → per-step mocks honored). The mock short-circuit
    // keys on step id regardless of kind, so the `tool` `search` step is mockable.
    let wf =
        import_dev_workflow(&server, &user.token, "sr-search-screen", SR_SEARCH_SCREEN_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();

    // A stub model + conversation so `spawn_run` can snapshot a model (never
    // invoked — every dispatched step is mocked).
    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;

    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({
            "inputs": { "query": "crispr base editing" },
            "conversation_id": conv_id.to_string(),
            "mocks": {
                // `search` (kind: tool) — canned AggregateResult (the deduped set).
                "search": {
                    "query": "crispr base editing",
                    "records": [
                        {"doi":"10.1/x","pmid":null,"title":"Base editing in plants","abstract_text":"A study.","authors":["A B"],"year":2021,"venue":"Nature","url":null,"source":"europepmc","source_ids":["europepmc:1"],"cited_by_count":3,"is_preprint":false,"relevance":0.9},
                        {"doi":"10.2/y","pmid":"999","title":"Off-target effects","abstract_text":"Another.","authors":["C D"],"year":2022,"venue":null,"url":null,"source":"crossref","source_ids":["crossref:1"],"cited_by_count":null,"is_preprint":false,"relevance":0.7}
                    ],
                    "identified": {"europepmc": 1, "crossref": 1},
                    "after_dedup": 2,
                    "degraded_sources": [],
                    "completeness": null
                },
                // `screen` (kind: llm_map) — canned per-record decisions.
                "screen": [
                    {"id":"10.1/x","decision":"include","reason":"on-topic","confidence":0.9},
                    {"id":"10.2/y","decision":"exclude","reason":"off-target focus","confidence":0.6}
                ]
            }
        }),
    )
    .await;

    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "SR search-screen run should complete; got: {final_run}"
    );

    // `step_outputs_json` is what the frontend bridge keys off (to pick the
    // candidate step `search`/`snowball` + the `screen` step) before calling
    // readOutput; assert both producing steps are recorded there.
    let so = &final_run["step_outputs_json"];
    assert!(
        so.get("search").is_some() && so.get("screen").is_some(),
        "step_outputs_json carries the bridge's keying source: {so}"
    );

    // `final_output_json` carries 500-char PREVIEW wrappers per declared output
    // ({value_preview, size_bytes, expose}) — NOT the raw values. Assert that
    // contract holds (the chat-summary path + the frontend bridge depend on it).
    let out = &final_run["final_output_json"];
    assert!(
        out["candidates"]["value_preview"].is_string(),
        "candidates output is persisted as a preview wrapper: {out}"
    );
    assert!(
        out["ai_screening"]["value_preview"].is_string(),
        "ai_screening output is persisted as a preview wrapper: {out}"
    );

    // The FULL step values (what the screening bridge reads) live on disk,
    // served per-step by GET /workflow-runs/{id}/output/{step_id}. `candidates`
    // ← the `search` step (AggregateResult); `ai_screening` ← the `screen` step.
    let read_output = |step: &'static str| {
        let url = server.api_url(&format!("/workflow-runs/{run_id}/output/{step}"));
        let token = user.token.clone();
        async move {
            let resp = reqwest::Client::new()
                .get(url)
                .header("Authorization", format!("Bearer {token}"))
                .send()
                .await
                .expect("read step output");
            assert_eq!(resp.status(), 200, "output endpoint 200 for completed step `{step}`");
            let body = resp.text().await.expect("output body");
            serde_json::from_str::<serde_json::Value>(&body)
                .unwrap_or_else(|e| panic!("step `{step}` output is JSON: {e}; body={body}"))
        }
    };

    let candidates = read_output("search").await;
    let recs = candidates["records"]
        .as_array()
        .unwrap_or_else(|| panic!("search output has records[]: {candidates}"));
    assert_eq!(recs.len(), 2, "both deduped records surfaced");
    assert_eq!(candidates["after_dedup"], 2);

    let screening = read_output("screen").await;
    let ai = screening
        .as_array()
        .unwrap_or_else(|| panic!("screen output is a decisions array: {screening}"));
    assert_eq!(ai.len(), 2, "one AI decision per record");
    assert_eq!(ai[0]["decision"], "include");
    assert_eq!(ai[1]["decision"], "exclude");
}

const SR_SNOWBALL_SCREEN_YAML: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/hub-seed/workflows/io.github.ziee/sr-snowball-screen/workflow.yaml"
));

/// Same deterministic shape as the search-screen test, for the SNOWBALL variant:
/// the `snowball` (tool: fetch_references) step is mocked with a canned
/// AggregateResult, `screen` (llm_map) with canned decisions; the bridge consumes
/// the identical `candidates`/`ai_screening` outputs (it keys off either the
/// `search` or `snowball` step).
#[tokio::test]
async fn sr_snowball_screen_runs_and_surfaces_screening_outputs() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_sr_snow_user").await;
    let wf = import_dev_workflow(
        &server,
        &user.token,
        "sr-snowball-screen",
        SR_SNOWBALL_SCREEN_YAML,
    )
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
                // `snowball` (kind: tool) — canned AggregateResult (the cited works).
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
                // `screen` (kind: llm_map) — canned per-record decisions.
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

    let read_output = |step: &'static str| {
        let url = server.api_url(&format!("/workflow-runs/{run_id}/output/{step}"));
        let token = user.token.clone();
        async move {
            let resp = reqwest::Client::new()
                .get(url)
                .header("Authorization", format!("Bearer {token}"))
                .send()
                .await
                .expect("read step output");
            assert_eq!(resp.status(), 200, "output endpoint 200 for completed step `{step}`");
            let body = resp.text().await.expect("output body");
            serde_json::from_str::<serde_json::Value>(&body)
                .unwrap_or_else(|e| panic!("step `{step}` output is JSON: {e}; body={body}"))
        }
    };

    // candidates ← the `snowball` step (NOT `search`); the bridge handles both.
    let candidates = read_output("snowball").await;
    let recs = candidates["records"]
        .as_array()
        .unwrap_or_else(|| panic!("snowball output has records[]: {candidates}"));
    assert_eq!(recs.len(), 2, "both snowballed records surfaced");
    assert_eq!(candidates["after_dedup"], 2);

    let screening = read_output("screen").await;
    let ai = screening
        .as_array()
        .unwrap_or_else(|| panic!("screen output is a decisions array: {screening}"));
    assert_eq!(ai.len(), 2, "one AI decision per record");
    assert_eq!(ai[0]["decision"], "include");
    assert_eq!(ai[1]["decision"], "exclude");
}
