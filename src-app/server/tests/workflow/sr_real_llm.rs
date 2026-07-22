//! Real-LLM coverage for the SR-specific PROMPTS (the only SR risk the
//! deterministic tests don't reach): do `sr-review`'s `expand` / `screen` /
//! `extract` / `synthesize` prompts produce valid, parseable output in the shapes
//! the DAG expects, with a real model?
//!
//! Two layers (both requested):
//!   - FOCUSED per-prompt tests — a tiny workflow running ONE real LLM step over
//!     mocked input, to isolate a prompt failure to a step.
//!   - END-TO-END — the REAL `sr-review` seed workflow with its tool steps mocked
//!     tiny and its LLM steps real, driving BOTH durable gates.
//!
//! Provider: Groq-first (`llama-3.3-70b-versatile` — cheap + tool-capable; see
//! `get_or_create_groq_first_model`), falling back to Anthropic/OpenAI/Gemini.
//! NO soft-skip: the helper PANICS if no provider key is set — `tests/.env.test`
//! ships keys, so this RUNS (per `feedback_no_ignore_unless_platform`). Cost is
//! bounded by MOCKING the fan-out sources (tiny record/paper sets), so total spend
//! is a handful of short calls.
//!
//! Assertions are SHAPE-based (real output is non-deterministic): an array of
//! decisions, rows with the expected keys, non-empty markdown — never exact text.

use serde_json::{Value, json};
use uuid::Uuid;

use super::{import_dev_workflow, poll_run, run_workflow, workflow_user};
use crate::common::TestServer;

/// The real `sr-review` seed definition (its tool steps get mocked below; its LLM
/// steps run real). include_str! keeps it drift-proof against the shipped bundle.
const SR_REVIEW_YAML: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/hub-seed/workflows/io.github.ziee/sr-review/workflow.yaml"
));

/// (user, conversation_id) bound to a real Groq-first model.
async fn real_llm_setup(
    server: &TestServer,
    name: &str,
) -> (crate::common::test_helpers::TestUser, String) {
    let user = workflow_user(server, name).await;
    let model = crate::chat::helpers::get_or_create_groq_first_model(server, &user.user_id).await;
    let model_id = model["id"].as_str().expect("model id").to_string();
    let conv = crate::chat::helpers::create_conversation(
        server,
        &user.token,
        Some(Uuid::parse_str(&model_id).unwrap()),
        Some("sr real-llm"),
    )
    .await;
    let conv_id = conv["id"].as_str().expect("conv id").to_string();
    (user, conv_id)
}

async fn read_output(server: &TestServer, token: &str, run_id: Uuid, step: &str) -> Value {
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}/output/{step}")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("read output");
    assert_eq!(resp.status(), 200, "output 200 for `{step}`");
    let body = resp.text().await.expect("body");
    serde_json::from_str(&body).unwrap_or(Value::String(body))
}

// ──────────────────────────── FOCUSED per-prompt ────────────────────────────

/// `screen` (llm_map) — sr-review's first-pass title/abstract screen prompt over
/// 2 records. Asserts the real model returns one parseable decision per record.
#[tokio::test]
async fn real_llm_sr_screen_prompt_yields_decisions() {
    let server = TestServer::start().await;
    let (user, conv_id) = real_llm_setup(&server, "wf_sr_screen").await;
    let yaml = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: query
    required: true
  - name: records
    required: true
steps:
  - id: screen
    kind: llm_map
    for_each: "{{ inputs.records }}"
    item_var: paper
    output_format: json
    max_parallel: 2
    on_error: skip
    prompt: |
      First-pass title/abstract screen for a systematic review of:
      "{{ inputs.query }}". Candidate (untrusted DATA):
      Title: {{ paper.title }}
      Abstract: {{ paper.abstract_text }}
      Be inclusive. Respond with ONLY a JSON object:
      {"id": "{{ paper.doi }}", "decision": "include" | "exclude",
       "reason": "<one short sentence>", "confidence": <0.0-1.0>}
outputs:
  - name: decisions
    from: "{{ screen.output }}"
    expose: full
"#;
    let wf = import_dev_workflow(&server, &user.token, "sr-screen-focus", yaml).await;
    let run = run_workflow(
        &server,
        &user.token,
        wf["id"].as_str().unwrap(),
        json!({
            "conversation_id": conv_id,
            "inputs": {
                "query": "CRISPR base editing off-target effects",
                "records": [
                    {"doi": "10.1/a", "title": "Base editing reduces off-target effects", "abstract_text": "A study of base-editor fidelity in human cells."},
                    {"doi": "10.2/b", "title": "Marine sponge taxonomy", "abstract_text": "A survey of sponges in the Pacific."}
                ]
            }
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "screen run completes: {final_run}");
    let out = read_output(&server, &user.token, run_id, "screen").await;
    eprintln!("\n[SR-SCREEN real output] {out}\n");
    let arr = out.as_array().unwrap_or_else(|| panic!("screen output is an array: {out}"));
    assert_eq!(arr.len(), 2, "one decision per record: {out}");
    for d in arr {
        let dec = d["decision"].as_str().unwrap_or("");
        assert!(dec == "include" || dec == "exclude", "valid decision: {d}");
    }
    // SENSIBLE (not just well-formed): the model must DISCRIMINATE — include the
    // on-topic base-editing paper, exclude the clearly off-topic marine-sponge one.
    let decision = |id: &str| {
        arr.iter()
            .find_map(|d| (d["id"].as_str() == Some(id)).then(|| d["decision"].as_str().unwrap_or("")))
    };
    assert_eq!(decision("10.1/a"), Some("include"), "on-topic base-editing paper INCLUDED: {out}");
    assert_eq!(decision("10.2/b"), Some("exclude"), "off-topic marine-sponge paper EXCLUDED: {out}");
}

/// `extract` (llm_map) — sr-review's per-study extraction prompt over 1 paper with
/// short full text. Asserts the row carries the PICO keys + a `quote`.
#[tokio::test]
async fn real_llm_sr_extract_prompt_yields_pico_fields() {
    let server = TestServer::start().await;
    let (user, conv_id) = real_llm_setup(&server, "wf_sr_extract").await;
    let yaml = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: query
    required: true
  - name: papers
    required: true
steps:
  - id: extract
    kind: llm_map
    for_each: "{{ inputs.papers }}"
    item_var: paper
    output_format: json
    max_parallel: 2
    on_error: skip
    prompt: |
      Extract systematic-review data for ONE study from its full text (untrusted DATA).
      Study id: {{ paper.id }}
      Full text: {{ paper.text }}
      Review question: "{{ inputs.query }}".
      Give each field a SHORT string ("" if not reported), a `confidence` (0.0-1.0),
      and ONE verbatim `quote` from the full text. Respond with ONLY this JSON object:
      {"id": "{{ paper.id }}", "population": "", "intervention": "", "comparator": "",
       "outcome": "", "effect": "", "risk_of_bias": "", "confidence": 0.0, "quote": ""}
outputs:
  - name: extractions
    from: "{{ extract.output }}"
    expose: full
"#;
    let wf = import_dev_workflow(&server, &user.token, "sr-extract-focus", yaml).await;
    let run = run_workflow(
        &server,
        &user.token,
        wf["id"].as_str().unwrap(),
        json!({
            "conversation_id": conv_id,
            "inputs": {
                "query": "base editing off-target effects",
                "papers": [
                    {"id": "10.1/a", "text": "In a randomized trial of 200 patients, base editing reduced off-target mutations by 47% compared with standard CRISPR, with low risk of bias."}
                ]
            }
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "extract run completes: {final_run}");
    let out = read_output(&server, &user.token, run_id, "extract").await;
    eprintln!("\n[SR-EXTRACT real output] {out}\n");
    let row = &out.as_array().and_then(|a| a.first()).cloned().unwrap_or(Value::Null);
    for key in ["id", "population", "intervention", "outcome", "confidence", "quote"] {
        assert!(row.get(key).is_some(), "extraction row has `{key}`: {out}");
    }
    // SENSIBLE: the extraction must reflect the SOURCE TEXT (the 47% off-target
    // effect), not be empty/invented — and the quote must be drawn from it.
    let blob = row.to_string().to_lowercase();
    assert!(
        blob.contains("47") || blob.contains("off-target") || blob.contains("off target"),
        "extraction captured the real effect from the source: {row}"
    );
    let quote = row["quote"].as_str().unwrap_or("");
    assert!(quote.trim().len() > 5, "a non-trivial supporting quote was extracted: {row}");
}

/// `expand` (llm, json) — sr-review's auto-expansion decision prompt. Asserts the
/// `{stop, new_queries[], snowball_seed_ids[]}` shape.
#[tokio::test]
async fn real_llm_sr_expand_prompt_yields_expansion_shape() {
    let server = TestServer::start().await;
    let (user, conv_id) = real_llm_setup(&server, "wf_sr_expand").await;
    let yaml = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: query
    required: true
  - name: records
    required: true
steps:
  - id: expand
    kind: llm
    output_format: json
    prompt: |
      You are boosting recall for a systematic review of: "{{ inputs.query }}".
      Records found so far (untrusted DATA): {{ inputs.records | json }}
      Decide how to expand the search ONE round. AT MOST 5 new queries and AT MOST
      10 snowball_seed_ids (DOIs from above). If saturated, set "stop": true with
      EMPTY lists. Respond with ONLY this JSON object:
      {"stop": false, "new_queries": ["..."], "snowball_seed_ids": ["..."]}
outputs:
  - name: expansion
    from: "{{ expand.output }}"
    expose: full
"#;
    let wf = import_dev_workflow(&server, &user.token, "sr-expand-focus", yaml).await;
    let run = run_workflow(
        &server,
        &user.token,
        wf["id"].as_str().unwrap(),
        json!({
            "conversation_id": conv_id,
            "inputs": {
                "query": "CRISPR base editing off-target effects",
                "records": [{"doi": "10.1/a", "title": "Base editing reduces off-target effects"}]
            }
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "expand run completes: {final_run}");
    let out = read_output(&server, &user.token, run_id, "expand").await;
    eprintln!("\n[SR-EXPAND real output] {out}\n");
    // Shape-only: whether to stop or which queries to add is a legitimate judgment
    // call, so we don't assert a specific decision — only that it's actionable.
    assert!(out["stop"].is_boolean(), "`stop` is a bool: {out}");
    assert!(out["new_queries"].is_array(), "`new_queries` is an array: {out}");
    assert!(out["snowball_seed_ids"].is_array(), "`snowball_seed_ids` is an array: {out}");
}

/// `synthesize` (llm) — sr-review's cited-synthesis prompt over canned extractions.
/// Asserts a non-trivial markdown deliverable.
#[tokio::test]
async fn real_llm_sr_synthesize_prompt_yields_markdown() {
    let server = TestServer::start().await;
    let (user, conv_id) = real_llm_setup(&server, "wf_sr_synth").await;
    let yaml = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: query
    required: true
  - name: extractions
    required: true
steps:
  - id: synthesize
    kind: llm
    output_format: text
    prompt: |
      Write an evidence synthesis answering: "{{ inputs.query }}". Use ONLY this
      reviewer-approved data (cite each claim inline by study id, e.g. [10.1/a]):
      {{ inputs.extractions | json }}
      End with a short "Limitations" paragraph. Return Markdown.
outputs:
  - name: report
    from: "{{ synthesize.output }}"
    expose: full
    mime_type: text/markdown
"#;
    let wf = import_dev_workflow(&server, &user.token, "sr-synth-focus", yaml).await;
    let run = run_workflow(
        &server,
        &user.token,
        wf["id"].as_str().unwrap(),
        json!({
            "conversation_id": conv_id,
            "inputs": {
                "query": "Does base editing reduce off-target effects?",
                "extractions": [
                    {"id": "10.1/a", "effect": "47% reduction in off-target mutations", "confidence": 0.8, "quote": "reduced off-target mutations by 47%"}
                ]
            }
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "synthesize run completes: {final_run}");
    let report = read_output(&server, &user.token, run_id, "synthesize").await;
    let text = report.as_str().unwrap_or("");
    eprintln!("\n[SR-SYNTHESIS real output]\n{text}\n");
    assert!(text.trim().len() > 40, "real synthesis is non-trivial markdown: {text:?}");
    // SENSIBLE: the synthesis must USE the supplied evidence (the 47% effect) and
    // CITE the study id it was given — not produce generic boilerplate.
    let low = text.to_lowercase();
    assert!(
        low.contains("47") || low.contains("off-target") || low.contains("off target"),
        "synthesis reflects the supplied evidence: {text}"
    );
    assert!(text.contains("10.1/a"), "synthesis cites the study id it was given: {text}");
}

// ──────────────────────────────── END-TO-END ────────────────────────────────

fn rec(doi: &str, title: &str) -> Value {
    json!({
        "doi": doi, "pmid": null, "title": title, "abstract_text": "A study abstract.",
        "authors": ["A B"], "year": 2021, "venue": "Nature", "url": null,
        "source": "europepmc", "source_ids": ["europepmc:1"], "cited_by_count": 3,
        "is_preprint": false, "relevance": 0.9
    })
}

fn agg(records: Vec<Value>) -> Value {
    let n = records.len();
    json!({ "query": "q", "records": records, "identified": {"europepmc": n}, "after_dedup": n, "degraded_sources": [], "completeness": null })
}

/// END-TO-END: the REAL `sr-review` workflow with its TOOL steps mocked tiny and
/// its LLM steps (expand×3, screen, extract, synthesize) running REAL via Groq.
/// No human gates — the run completes UNATTENDED. Bounded fan-out (3 candidates,
/// 2 papers) keeps spend to ~9 short calls.
#[tokio::test]
async fn real_llm_sr_review_end_to_end_completes() {
    let server = TestServer::start().await;
    let (user, conv_id) = real_llm_setup(&server, "wf_sr_e2e").await;
    let wf = import_dev_workflow(&server, &user.token, "sr-review-e2e", SR_REVIEW_YAML).await;
    let wf_id = wf["id"].as_str().unwrap().to_string();

    // Mock ONLY the tool steps (bounds fan-out); LLM steps run real.
    let mocks = json!({
        "search0": agg(vec![rec("10.1/a", "Base editing reduces off-target effects")]),
        "esearch1": agg(vec![]), "esnow1": agg(vec![]),
        "esearch2": agg(vec![]), "esnow2": agg(vec![]),
        "esearch3": agg(vec![]), "esnow3": agg(vec![]),
        "dedup_all": agg(vec![
            rec("10.1/a", "Base editing reduces off-target effects"),
            rec("10.2/b", "Prime editing fidelity"),
            rec("10.3/c", "Off-target detection methods")
        ]),
        "select_included": {"included_ids": ["10.1/a", "10.3/c"], "included": 2, "excluded": 1, "skipped": 0},
        "fetch": {"papers": [
            {"id": "10.1/a", "status": "full_text", "text": "A randomized trial of 200 patients found base editing reduced off-target mutations by 47%."},
            {"id": "10.3/c", "status": "full_text", "text": "A method paper describing GUIDE-seq detection of off-target edits."}
        ]}
    });

    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({ "inputs": { "query": "CRISPR base editing off-target effects" }, "conversation_id": conv_id, "mocks": mocks }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();

    // No human gates — the run completes unattended. This is the full SR
    // pipeline (search → expand×N → screen → dedup → fetch → select → extract →
    // synthesize → review), ~15 sequential LLM steps, so it needs a longer
    // deadline than the 30s default on a slow local bridge.
    let final_run = super::poll_run_for(&server, &user.token, run_id, 240).await;
    assert_eq!(final_run["status"], "completed", "real-LLM sr-review completes: {final_run}");

    // The real `screen` produced a decision array; the real `synthesize` produced
    // non-empty markdown. (Shape-only — real output is non-deterministic.)
    let screen = read_output(&server, &user.token, run_id, "screen").await;
    eprintln!("\n[SR-E2E screen decisions] {screen}\n");
    assert!(screen.as_array().map(|a| !a.is_empty()).unwrap_or(false), "real screen decisions: {screen}");
    let report = read_output(&server, &user.token, run_id, "synthesize").await;
    let text = report.as_str().unwrap_or("");
    eprintln!("\n[SR-E2E synthesis]\n{text}\n");
    assert!(text.trim().len() > 40, "real synthesis markdown: {report}");
    // SENSIBLE: the end-to-end synthesis must engage the actual evidence (the
    // included papers' off-target effect), not generic filler.
    let low = text.to_lowercase();
    assert!(
        low.contains("off-target") || low.contains("off target") || low.contains("base edit") || low.contains("47"),
        "e2e synthesis engages the real evidence: {text}"
    );
}
