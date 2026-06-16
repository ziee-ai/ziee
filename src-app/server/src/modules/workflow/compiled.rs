//! Compiled IR (plan §4.1 pattern (d) — "Compiled IR", dbt-compile
//! pattern).
//!
//! Output of the validator's "compile" pass: a compact, typed,
//! statically-resolved view of a `workflow.yaml`. It is METADATA, not
//! the bodies — prompt/run text stays on disk in the bundle. The IR
//! captures:
//!   - topo-ordered step indices (the runner's dispatch order),
//!   - per-step inferred output type (from `type_infer.rs`),
//!   - resolved static template segments where possible (literal-only
//!     inputs whose value is known at compile time),
//!   - the inputs schema (name / required / default / inferred type),
//!   - per-step effect annotations (kind + estimated call count).
//!
//! Persisted into `workflows.compiled_ir_json` at install time (the
//! column existed but was always NULL before this pass). For Phase 1 the
//! runner still re-parses `workflow.yaml`; populating the IR is enough
//! to make it available + non-NULL (the runner-reads-IR optimization is
//! noted in the plan as optional). Recompute on every install + on any
//! workflow definition change.

#![allow(dead_code)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::modules::workflow::cost;
use crate::modules::workflow::type_infer::{infer_input_type, infer_step_output_type, InferredType};
use crate::modules::workflow::validate::{StepConfig, WorkflowDef};

/// Schema version of the IR payload — lets the runner / future readers
/// detect a stale IR and recompile.
pub const WORKFLOW_IR_VERSION: u32 = 1;

/// Serializable view of an `InferredType` for the IR (the in-memory
/// `InferredType` enum is not Serialize; this mirror is). Coarse on
/// purpose — Phase 1 typing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IrType {
    String,
    Number,
    Bool,
    Null,
    Array { element: Box<IrType> },
    ArrayUnknown,
    Object { fields: BTreeMap<String, IrType> },
    ObjectUnknown,
    Unknown,
}

impl From<&InferredType> for IrType {
    fn from(t: &InferredType) -> Self {
        match t {
            InferredType::String => IrType::String,
            InferredType::Number => IrType::Number,
            InferredType::Bool => IrType::Bool,
            InferredType::Null => IrType::Null,
            InferredType::Array(inner) => IrType::Array {
                element: Box::new(IrType::from(inner.as_ref())),
            },
            InferredType::ArrayUnknown => IrType::ArrayUnknown,
            InferredType::Object(fields) => IrType::Object {
                fields: fields.iter().map(|(k, v)| (k.clone(), IrType::from(v))).collect(),
            },
            InferredType::ObjectUnknown => IrType::ObjectUnknown,
            InferredType::Unknown => IrType::Unknown,
        }
    }
}

/// One declared input, compiled.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrInput {
    pub name: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    pub inferred_type: IrType,
}

/// Per-step compiled metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrStep {
    pub id: String,
    pub kind: String,
    pub depends_on: Vec<String>,
    /// Inferred static output type.
    pub output_type: IrType,
    /// Effect annotation: estimated LLM call count for this step.
    /// `llm` = 1, `llm_map` = max_parallel cap (runtime-dependent fan-out
    /// can exceed it across batches), `sandbox` / `elicit` = 0.
    pub est_calls: u64,
    /// `true` when this step is a `sandbox` step (touches the sandbox).
    pub uses_sandbox: bool,
}

/// One declared output, compiled.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrOutput {
    pub name: String,
    pub from: String,
}

/// The compiled IR persisted to `workflows.compiled_ir_json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowIr {
    pub ir_version: u32,
    /// Topo-ordered indices into `steps` (the runner's dispatch order).
    pub topo_order: Vec<usize>,
    pub inputs: Vec<IrInput>,
    pub steps: Vec<IrStep>,
    pub outputs: Vec<IrOutput>,
    /// Effect summary scaffold (full effect-summary endpoint is Phase 2):
    /// aggregate static call count + step count. Cheap to compute here.
    pub est_total_calls: u64,
    pub step_count: u64,
    pub sandbox_flavor: Option<String>,
}

/// Compile a validated `WorkflowDef` into a `WorkflowIr`. Assumes the
/// workflow already passed `validate_for_install` (so topo-sort
/// succeeds); if a cycle somehow slips through, the topo order falls
/// back to declaration order rather than panicking.
pub fn compile(workflow: &WorkflowDef) -> WorkflowIr {
    let topo_order = crate::modules::workflow::validate::topo_sort_steps(workflow)
        .unwrap_or_else(|_| (0..workflow.steps.len()).collect());

    let inputs = workflow
        .inputs
        .iter()
        .map(|i| IrInput {
            name: i.name.clone(),
            required: i.required,
            default: i.default.clone(),
            inferred_type: IrType::from(&infer_input_type(i.default.as_ref())),
        })
        .collect();

    let mut est_total_calls: u64 = 0;
    let steps: Vec<IrStep> = workflow
        .steps
        .iter()
        .map(|s| {
            let est_calls = match &s.config {
                StepConfig::Llm { .. } => 1u64,
                StepConfig::LlmMap { max_parallel, .. } => *max_parallel as u64,
                StepConfig::Sandbox { .. } | StepConfig::Elicit { .. } => 0u64,
            };
            est_total_calls += est_calls;
            IrStep {
                id: s.id.clone(),
                kind: s.config.kind_str().to_string(),
                depends_on: s.depends_on.clone(),
                output_type: IrType::from(&infer_step_output_type(s)),
                est_calls,
                uses_sandbox: matches!(s.config, StepConfig::Sandbox { .. }),
            }
        })
        .collect();

    let outputs = workflow
        .outputs
        .iter()
        .map(|o| IrOutput {
            name: o.name.clone(),
            from: o.from.clone(),
        })
        .collect();

    // Cross-check the aggregate with the cost estimator so the two
    // surfaces stay consistent (estimate_static returns the same count).
    let (_steps, static_calls, _tokens) = cost::estimate_static(workflow);
    debug_assert_eq!(static_calls, est_total_calls);

    WorkflowIr {
        ir_version: WORKFLOW_IR_VERSION,
        topo_order,
        inputs,
        steps,
        outputs,
        est_total_calls,
        step_count: workflow.steps.len() as u64,
        sandbox_flavor: workflow.sandbox.as_ref().map(|s| s.flavor.clone()),
    }
}

/// Compile to a `serde_json::Value` for the `compiled_ir_json` column.
/// Returns None only on the (practically impossible) serialization
/// failure — callers persist `Some(v)` so the column is no longer NULL.
pub fn compile_to_json(workflow: &WorkflowDef) -> Option<serde_json::Value> {
    serde_json::to_value(compile(workflow)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::workflow::validate::parse_workflow_yaml;

    fn wf(yaml: &str) -> WorkflowDef {
        parse_workflow_yaml(yaml).expect("parse")
    }

    #[test]
    fn compile_produces_non_empty_ir() {
        let w = wf(r#"
inputs:
  - name: topic
    required: true
    default: "ai"
steps:
  - id: gen
    kind: llm
    prompt: "{{ inputs.topic }}"
    output_format: text
  - id: fan
    kind: llm_map
    for_each: "{{ inputs.topic }}"
    item_var: q
    prompt: "{{ q }}"
    max_parallel: 4
    depends_on: [gen]
outputs:
  - name: result
    from: "{{ fan.output }}"
"#);
        let ir = compile(&w);
        assert_eq!(ir.ir_version, WORKFLOW_IR_VERSION);
        assert_eq!(ir.step_count, 2);
        assert_eq!(ir.steps.len(), 2);
        assert_eq!(ir.inputs.len(), 1);
        assert_eq!(ir.outputs.len(), 1);
        assert_eq!(ir.topo_order.len(), 2);
        // gen=1 call, fan=4 (max_parallel) → 5 total.
        assert_eq!(ir.est_total_calls, 5);
        // gen is llm/text → String output type.
        let gen_step = ir.steps.iter().find(|s| s.id == "gen").unwrap();
        assert_eq!(gen_step.output_type, IrType::String);
        assert_eq!(gen_step.est_calls, 1);
        // fan is llm_map → array of string.
        let fan = ir.steps.iter().find(|s| s.id == "fan").unwrap();
        assert!(matches!(fan.output_type, IrType::Array { .. }));
    }

    #[test]
    fn compile_to_json_is_some_and_round_trips() {
        let w = wf(r#"
steps:
  - id: a
    kind: llm
    prompt: "x"
"#);
        let v = compile_to_json(&w).expect("ir json");
        assert!(v.is_object());
        let back: WorkflowIr = serde_json::from_value(v).expect("round-trip");
        assert_eq!(back.step_count, 1);
        assert_eq!(back.ir_version, WORKFLOW_IR_VERSION);
    }

    #[test]
    fn topo_order_respects_dependencies() {
        let w = wf(r#"
steps:
  - id: c
    kind: llm
    prompt: "z"
    depends_on: [b]
  - id: b
    kind: llm
    prompt: "y"
    depends_on: [a]
  - id: a
    kind: llm
    prompt: "x"
"#);
        let ir = compile(&w);
        // Map topo indices to ids, assert a < b < c.
        let order_ids: Vec<&str> = ir
            .topo_order
            .iter()
            .map(|&i| w.steps[i].id.as_str())
            .collect();
        let pos = |id: &str| order_ids.iter().position(|x| *x == id).unwrap();
        assert!(pos("a") < pos("b") && pos("b") < pos("c"));
    }

    #[test]
    fn sandbox_flavor_captured() {
        let w = wf(r#"
sandbox:
  flavor: minimal
steps:
  - id: build
    kind: sandbox
    run: "echo hi"
"#);
        let ir = compile(&w);
        assert_eq!(ir.sandbox_flavor.as_deref(), Some("minimal"));
        assert!(ir.steps[0].uses_sandbox);
        assert_eq!(ir.steps[0].est_calls, 0);
    }
}
