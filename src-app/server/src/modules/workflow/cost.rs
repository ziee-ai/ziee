//! Static DAG cost estimation (B6) — powers `POST /api/workflows/validate`
//! and `POST /api/workflows/{id}/dry-run`. Walks the steps WITHOUT
//! executing anything: zero LLM tokens spent, zero `workflow_runs` rows
//! created. Inspired by Snakemake's `--dry-run`.
//!
//! Two estimation surfaces:
//!  - `estimate_static` — input-free; used by `/validate` to bound a
//!    workflow's worst case from the YAML alone. `llm_map` fan-out count
//!    is unknown statically (depends on a prior step's runtime output),
//!    so it's reported as the per-step `max_parallel` cap with a
//!    "runtime-dependent" note.
//!  - `dry_run` — given concrete inputs, tries to resolve each
//!    `llm_map`'s `for_each` template against the inputs. When the
//!    template references ONLY inputs (a static array), the real fan-out
//!    count is reported; when it references a prior step's output (the
//!    common case), the count is "runtime-dependent" and we fall back to
//!    the `max_parallel` cap.

#![allow(dead_code)]

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::modules::workflow::validate::{StepConfig, WorkflowDef};

/// Per-call token ceiling (used to derive a token UPPER BOUND from a
/// call count). This is a budgeting heuristic, not a measurement — the
/// real per-call cap the dispatcher enforces is the provider's; this
/// just gives the UI a worst-case number so a 50-call fan-out shows a
/// scary-but-honest ceiling.
pub const PER_CALL_TOKEN_CEILING: u64 = 50_000;

/// Rough split between input + output tokens for the dry-run per-step
/// breakdown (informational only).
const TOKENS_IN_PER_CALL: u64 = 30_000;
const TOKENS_OUT_PER_CALL: u64 = 20_000;

/// Per-step dry-run breakdown row.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DryRunStep {
    pub step_id: String,
    pub kind: String,
    /// Number of LLM calls this step makes. `llm` = 1, `llm_map` = the
    /// fan-out count (or `max_parallel` cap when runtime-dependent),
    /// `sandbox` / `elicit` = 0.
    pub est_calls: u64,
    pub est_tokens_in: u64,
    pub est_tokens_out: u64,
    /// `true` when the call count couldn't be resolved statically — i.e.
    /// an `llm_map` whose `for_each` references a prior step's output. The
    /// reported `est_calls` is then the `max_parallel` cap, not a true
    /// count.
    pub runtime_dependent: bool,
}

/// Result of `dry_run`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DryRunResult {
    pub steps: Vec<DryRunStep>,
    pub total_est_calls: u64,
    pub total_est_tokens: u64,
    /// Rough constant-rate cost estimate. Omitted (None) — we don't ship
    /// per-model pricing tables in Phase 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub est_cost_usd: Option<f64>,
}

/// Input-free static estimate (for `/validate`). Returns
/// `(steps, est_max_calls, est_max_tokens)`. `llm_map` contributes its
/// `max_parallel` cap as the per-step max (a single batch of concurrent
/// calls — the true count can be higher across batches, but Phase 1
/// reports the concurrency cap as the static bound, marked runtime-
/// dependent in the dry-run surface).
pub fn estimate_static(workflow: &WorkflowDef) -> (u64, u64, u64) {
    let steps = workflow.steps.len() as u64;
    let mut max_calls: u64 = 0;
    for step in &workflow.steps {
        max_calls += match &step.config {
            StepConfig::Llm { .. } => 1,
            StepConfig::LlmMap { max_parallel, .. } => *max_parallel as u64,
            StepConfig::Sandbox { .. } | StepConfig::Elicit { .. } => 0,
        };
    }
    let max_tokens = max_calls.saturating_mul(PER_CALL_TOKEN_CEILING);
    (steps, max_calls, max_tokens)
}

/// Walk the DAG with concrete `inputs`, resolving `for_each` against the
/// inputs where possible. Spends zero tokens. `inputs` is the already-
/// bound input map (after defaults applied + required-input validation).
pub fn dry_run(workflow: &WorkflowDef, inputs: &serde_json::Map<String, Value>) -> DryRunResult {
    let mut steps = Vec::with_capacity(workflow.steps.len());
    let mut total_calls: u64 = 0;

    for step in &workflow.steps {
        let (est_calls, runtime_dependent) = match &step.config {
            StepConfig::Llm { .. } => (1u64, false),
            StepConfig::LlmMap {
                for_each,
                max_parallel,
                ..
            } => {
                // Try to resolve the for_each template against the inputs
                // ONLY. If it references a prior step's output, we can't
                // know the count at dry-run time → fall back to the
                // max_parallel cap, flagged runtime-dependent.
                match resolve_for_each_against_inputs(for_each, inputs) {
                    Some(n) => (n, false),
                    None => (*max_parallel as u64, true),
                }
            }
            StepConfig::Sandbox { .. } | StepConfig::Elicit { .. } => (0u64, false),
        };
        total_calls += est_calls;
        steps.push(DryRunStep {
            step_id: step.id.clone(),
            kind: step.config.kind_str().to_string(),
            est_calls,
            est_tokens_in: est_calls.saturating_mul(TOKENS_IN_PER_CALL),
            est_tokens_out: est_calls.saturating_mul(TOKENS_OUT_PER_CALL),
            runtime_dependent,
        });
    }

    let total_est_tokens = total_calls.saturating_mul(PER_CALL_TOKEN_CEILING);
    DryRunResult {
        steps,
        total_est_calls: total_calls,
        total_est_tokens,
        est_cost_usd: None,
    }
}

/// If `for_each` is exactly `{{ inputs.<name> }}` and that input resolves
/// to a JSON array, return its length. Anything else (a prior-step
/// reference, a non-array input, a more complex template) returns None
/// → the caller treats the count as runtime-dependent.
fn resolve_for_each_against_inputs(
    for_each: &str,
    inputs: &serde_json::Map<String, Value>,
) -> Option<u64> {
    let inner = for_each.trim();
    let inner = inner.strip_prefix("{{")?.strip_suffix("}}")?.trim();
    // Drop any `| filter` suffix.
    let inner = inner.split('|').next().unwrap_or(inner).trim();
    let name = inner.strip_prefix("inputs.")?;
    // Reject indexing / nested access — only a bare `inputs.<name>`.
    if name.contains('.') || name.contains('[') {
        return None;
    }
    match inputs.get(name) {
        Some(Value::Array(a)) => Some(a.len() as u64),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::workflow::validate::parse_workflow_yaml;

    fn wf(yaml: &str) -> WorkflowDef {
        parse_workflow_yaml(yaml).expect("parse")
    }

    #[test]
    fn static_counts_one_call_per_llm_step() {
        let w = wf(r#"
steps:
  - id: a
    kind: llm
    prompt: "x"
  - id: b
    kind: llm
    prompt: "y"
    depends_on: [a]
"#);
        let (steps, calls, tokens) = estimate_static(&w);
        assert_eq!(steps, 2);
        assert_eq!(calls, 2);
        assert_eq!(tokens, 2 * PER_CALL_TOKEN_CEILING);
    }

    #[test]
    fn static_sandbox_counts_zero_calls() {
        let w = wf(r#"
sandbox:
  flavor: minimal
steps:
  - id: a
    kind: sandbox
    run: "echo hi"
  - id: b
    kind: sandbox
    run: "echo bye"
    depends_on: [a]
"#);
        let (_steps, calls, _tokens) = estimate_static(&w);
        assert_eq!(calls, 0);
    }

    #[test]
    fn static_llm_map_counts_max_parallel() {
        let w = wf(r#"
steps:
  - id: gen
    kind: llm
    prompt: "x"
  - id: fan
    kind: llm_map
    for_each: "{{ gen.output }}"
    item_var: q
    prompt: "{{ q }}"
    max_parallel: 7
    depends_on: [gen]
"#);
        let (_steps, calls, _tokens) = estimate_static(&w);
        // 1 (gen) + 7 (fan-out cap) = 8.
        assert_eq!(calls, 8);
    }

    #[test]
    fn dry_run_resolves_for_each_from_inputs() {
        let w = wf(r#"
inputs:
  - name: queries
    required: true
steps:
  - id: fan
    kind: llm_map
    for_each: "{{ inputs.queries }}"
    item_var: q
    prompt: "{{ q }}"
    max_parallel: 5
"#);
        let mut inputs = serde_json::Map::new();
        inputs.insert(
            "queries".into(),
            serde_json::json!(["a", "b", "c", "d"]),
        );
        let res = dry_run(&w, &inputs);
        assert_eq!(res.steps.len(), 1);
        assert_eq!(res.steps[0].est_calls, 4);
        assert!(!res.steps[0].runtime_dependent);
        assert_eq!(res.total_est_calls, 4);
    }

    #[test]
    fn dry_run_marks_prior_step_for_each_runtime_dependent() {
        let w = wf(r#"
steps:
  - id: gen
    kind: llm
    prompt: "x"
  - id: fan
    kind: llm_map
    for_each: "{{ gen.output }}"
    item_var: q
    prompt: "{{ q }}"
    max_parallel: 6
    depends_on: [gen]
"#);
        let res = dry_run(&w, &serde_json::Map::new());
        let fan = res.steps.iter().find(|s| s.step_id == "fan").unwrap();
        assert!(fan.runtime_dependent);
        assert_eq!(fan.est_calls, 6); // falls back to max_parallel cap
        let gen_step = res.steps.iter().find(|s| s.step_id == "gen").unwrap();
        assert_eq!(gen_step.est_calls, 1);
        assert!(!gen_step.runtime_dependent);
    }
}
