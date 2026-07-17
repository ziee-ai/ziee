//! Output-type inference + propagation across steps (plan §4.1 pattern
//! (a) — "Output-type inference").
//!
//! This is the consumer half of the compiler-strength validation
//! roadmap: instead of merely checking that a `{{ X.Y }}` reference
//! NAMES a real step/input (Layer 2 in `validate.rs`), we infer a
//! coarse JSON-shape type for every step output so the reference
//! checker (`ref_check.rs`, pattern (b)) can catch type-level mistakes
//! — e.g. `{{ research.output[0] }}` when `research` produces a String.
//!
//! Phase 1 deliberately stays coarse: an `llm` step with
//! `output_format: json` yields `Unknown` (we don't yet parse the
//! prompt for shape hints — that's the Phase-2 ObjectUnknown/ArrayUnknown
//! inference). The escape hatch is preserved by `ref_check.rs` emitting
//! WARNINGS (not errors) whenever a reference lands on an `Unknown`.
//!
//! Derivation table (plan §4.1 "Type system shape (Phase 1)"):
//!   - `kind: llm`, `output_format: text`           → String
//!   - `kind: llm`, `output_format: json`           → Unknown (Phase 1)
//!   - `kind: llm_map`                              → Array(inner) where
//!       inner is the per-item output type (the item prompt's
//!       output_format)
//!   - `kind: sandbox` (default)                    → Unknown
//!   - `kind: elicit`                               → inferred from the
//!       step's `schema:` JSON Schema (object-with-properties →
//!       Object(...), else ObjectUnknown)


use std::collections::{BTreeMap, HashMap};

use crate::modules::workflow::validate::{OutputFormat, StepConfig, StepDef, WorkflowDef};

/// Coarse JSON-shape type inferred for a step output / input. Mirrors
/// the enum in plan §4.1 "Type system shape (Phase 1)".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InferredType {
    String,
    Number,
    Bool,
    Null,
    /// Array with a known element type.
    Array(Box<InferredType>),
    /// Array whose element type is unresolved (e.g. `llm_map` over an
    /// `Unknown` item type, or a `for_each` result).
    ArrayUnknown,
    /// Object with a known shape (field → type).
    Object(BTreeMap<String, InferredType>),
    /// Generic JSON object (shape unknown).
    ObjectUnknown,
    /// Worst case — text output with no schema, or json output whose
    /// shape we don't infer in Phase 1.
    Unknown,
}

impl InferredType {
    /// Short human label for diagnostics.
    pub fn label(&self) -> String {
        match self {
            InferredType::String => "string".into(),
            InferredType::Number => "number".into(),
            InferredType::Bool => "bool".into(),
            InferredType::Null => "null".into(),
            InferredType::Array(inner) => format!("array<{}>", inner.label()),
            InferredType::ArrayUnknown => "array<unknown>".into(),
            InferredType::Object(_) => "object{…}".into(),
            InferredType::ObjectUnknown => "object".into(),
            InferredType::Unknown => "unknown".into(),
        }
    }

    /// True for `Array` / `ArrayUnknown` — intended for `ref_check` to decide
    /// whether `[N]` indexing is type-valid (not yet consumed there).
    #[allow(dead_code)]
    pub fn is_array(&self) -> bool {
        matches!(self, InferredType::Array(_) | InferredType::ArrayUnknown)
    }

    /// True for `Object` / `ObjectUnknown`. Type-predicate companion to
    /// `is_array`; no caller yet.
    #[allow(dead_code)]
    pub fn is_object(&self) -> bool {
        matches!(self, InferredType::Object(_) | InferredType::ObjectUnknown)
    }
}

/// Infer the output type of a single step per the derivation table.
pub fn infer_step_output_type(step: &StepDef) -> InferredType {
    match &step.config {
        StepConfig::Llm { output_format, .. } => match output_format {
            OutputFormat::Text => InferredType::String,
            // Phase 1: json output's shape is not inferred from the prompt.
            OutputFormat::Json => InferredType::Unknown,
        },
        StepConfig::LlmMap { output_format, .. } => {
            // Each item produces a per-item output; the step output is an
            // array of those. The per-item type follows the same llm rule.
            let inner = match output_format {
                OutputFormat::Text => InferredType::String,
                OutputFormat::Json => InferredType::Unknown,
            };
            InferredType::Array(Box::new(inner))
        }
        StepConfig::Sandbox { .. } => {
            // Sandbox output is best-effort JSON-sniffed at runtime; no
            // static guarantee → Unknown.
            InferredType::Unknown
        }
        StepConfig::Elicit { schema, .. } => infer_from_json_schema(schema),
        // Tool output is the MCP result (structuredContent or text) — no
        // static shape guarantee.
        StepConfig::Tool { .. } => InferredType::Unknown,
        // Agent output is the model's final answer (text, or json when
        // `output_format: json`) — no static shape guarantee for json.
        StepConfig::Agent { output_format, .. } => match output_format {
            OutputFormat::Text => InferredType::String,
            OutputFormat::Json => InferredType::Unknown,
        },
    }
}

/// Infer the type an `inputs.<name>` reference resolves to. The input
/// declaration carries no explicit type in Phase 1; we sniff the
/// `default` value's JSON shape when present, else fall back to
/// `Unknown` (the safe choice — `ref_check` then warns, never errors,
/// on field/index access against it). A bare scalar default that is a
/// string is the most common case and yields `String`.
pub fn infer_input_type(default: Option<&serde_json::Value>) -> InferredType {
    match default {
        Some(v) => infer_from_json_value(v),
        None => InferredType::Unknown,
    }
}

/// Map a concrete JSON value to a coarse `InferredType`.
fn infer_from_json_value(v: &serde_json::Value) -> InferredType {
    match v {
        serde_json::Value::String(_) => InferredType::String,
        serde_json::Value::Number(_) => InferredType::Number,
        serde_json::Value::Bool(_) => InferredType::Bool,
        serde_json::Value::Null => InferredType::Null,
        serde_json::Value::Array(items) => {
            // Use the first element's type if the array is homogeneous-ish;
            // otherwise ArrayUnknown.
            match items.first() {
                Some(first) => {
                    let t = infer_from_json_value(first);
                    if items.iter().all(|i| infer_from_json_value(i) == t) {
                        InferredType::Array(Box::new(t))
                    } else {
                        InferredType::ArrayUnknown
                    }
                }
                None => InferredType::ArrayUnknown,
            }
        }
        serde_json::Value::Object(_) => InferredType::ObjectUnknown,
    }
}

/// Infer an `InferredType` from a JSON-Schema fragment (used for
/// `elicit` step schemas). Only the shallow `type` + `properties`
/// structure is interpreted; nested object shapes recurse one level.
fn infer_from_json_schema(schema: &serde_json::Value) -> InferredType {
    let obj = match schema.as_object() {
        Some(o) => o,
        None => return InferredType::ObjectUnknown,
    };
    let ty = obj.get("type").and_then(|t| t.as_str());
    match ty {
        Some("object") => {
            if let Some(props) = obj.get("properties").and_then(|p| p.as_object()) {
                let mut fields = BTreeMap::new();
                for (k, v) in props {
                    fields.insert(k.clone(), infer_from_json_schema(v));
                }
                if fields.is_empty() {
                    InferredType::ObjectUnknown
                } else {
                    InferredType::Object(fields)
                }
            } else {
                InferredType::ObjectUnknown
            }
        }
        Some("array") => {
            if let Some(items) = obj.get("items") {
                InferredType::Array(Box::new(infer_from_json_schema(items)))
            } else {
                InferredType::ArrayUnknown
            }
        }
        Some("string") => InferredType::String,
        Some("integer") | Some("number") => InferredType::Number,
        Some("boolean") => InferredType::Bool,
        Some("null") => InferredType::Null,
        // No `type` (or an unrecognized one) but properties present →
        // treat as an object.
        _ => {
            if obj.contains_key("properties") {
                infer_from_json_schema(&serde_json::json!({
                    "type": "object",
                    "properties": obj.get("properties").cloned().unwrap_or_default(),
                }))
            } else {
                InferredType::Unknown
            }
        }
    }
}

/// Build a `step_id → InferredType` map over all steps. Steps are
/// processed in declaration order; the topo order doesn't change the
/// result here because each step's output type is a pure function of
/// the step itself in Phase 1 (no cross-step propagation yet — that
/// arrives with Phase-2 json-shape inference). Returned as a
/// `HashMap` for O(1) lookup by `ref_check`.
pub fn infer_all_step_types(workflow: &WorkflowDef) -> HashMap<String, InferredType> {
    let mut map = HashMap::with_capacity(workflow.steps.len());
    for step in &workflow.steps {
        map.insert(step.id.clone(), infer_step_output_type(step));
    }
    map
}

/// Build an `inputs.<name> → InferredType` map from the workflow's
/// declared inputs.
pub fn infer_all_input_types(workflow: &WorkflowDef) -> HashMap<String, InferredType> {
    let mut map = HashMap::with_capacity(workflow.inputs.len());
    for input in &workflow.inputs {
        map.insert(input.name.clone(), infer_input_type(input.default.as_ref()));
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::workflow::validate::parse_workflow_yaml;

    fn wf(yaml: &str) -> WorkflowDef {
        parse_workflow_yaml(yaml).expect("parse")
    }

    #[test]
    fn llm_text_infers_string() {
        let w = wf(r#"
steps:
  - id: gen
    kind: llm
    prompt: "x"
    output_format: text
"#);
        assert_eq!(infer_step_output_type(&w.steps[0]), InferredType::String);
    }

    #[test]
    fn llm_json_infers_unknown_phase1() {
        let w = wf(r#"
steps:
  - id: gen
    kind: llm
    prompt: "x"
    output_format: json
"#);
        assert_eq!(infer_step_output_type(&w.steps[0]), InferredType::Unknown);
    }

    #[test]
    fn llm_map_infers_array_of_item_type() {
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
    output_format: text
    depends_on: [gen]
"#);
        let t = infer_step_output_type(&w.steps[1]);
        assert_eq!(t, InferredType::Array(Box::new(InferredType::String)));
        assert!(t.is_array());
    }

    #[test]
    fn llm_map_json_infers_array_of_unknown() {
        let w = wf(r#"
steps:
  - id: fan
    kind: llm_map
    for_each: "{{ inputs.qs }}"
    item_var: q
    prompt: "{{ q }}"
    output_format: json
inputs:
  - name: qs
"#);
        let t = infer_step_output_type(&w.steps[0]);
        assert_eq!(t, InferredType::Array(Box::new(InferredType::Unknown)));
    }

    #[test]
    fn sandbox_infers_unknown() {
        let w = wf(r#"
sandbox:
  flavor: minimal
steps:
  - id: build
    kind: sandbox
    run: "echo hi"
"#);
        assert_eq!(infer_step_output_type(&w.steps[0]), InferredType::Unknown);
    }

    #[test]
    fn elicit_infers_object_shape_from_schema() {
        let w = wf(r#"
steps:
  - id: confirm
    kind: elicit
    message: "go?"
    schema:
      type: object
      properties:
        proceed: { type: boolean }
        max_sources: { type: integer }
      required: [proceed]
"#);
        let t = infer_step_output_type(&w.steps[0]);
        match t {
            InferredType::Object(fields) => {
                assert_eq!(fields.get("proceed"), Some(&InferredType::Bool));
                assert_eq!(fields.get("max_sources"), Some(&InferredType::Number));
            }
            other => panic!("expected Object, got {other:?}"),
        }
    }

    #[test]
    fn elicit_without_properties_is_object_unknown() {
        let w = wf(r#"
steps:
  - id: confirm
    kind: elicit
    message: "go?"
    schema:
      type: object
"#);
        assert_eq!(
            infer_step_output_type(&w.steps[0]),
            InferredType::ObjectUnknown
        );
    }

    #[test]
    fn input_default_sniffs_type() {
        assert_eq!(
            infer_input_type(Some(&serde_json::json!("hello"))),
            InferredType::String
        );
        assert_eq!(
            infer_input_type(Some(&serde_json::json!(42))),
            InferredType::Number
        );
        assert_eq!(
            infer_input_type(Some(&serde_json::json!(["a", "b"]))),
            InferredType::Array(Box::new(InferredType::String))
        );
        assert_eq!(infer_input_type(None), InferredType::Unknown);
    }

    #[test]
    fn infer_all_builds_full_map() {
        let w = wf(r#"
inputs:
  - name: topic
    default: "ai"
steps:
  - id: a
    kind: llm
    prompt: "{{ inputs.topic }}"
  - id: b
    kind: llm
    prompt: "x"
    output_format: json
    depends_on: [a]
"#);
        let steps = infer_all_step_types(&w);
        assert_eq!(steps.get("a"), Some(&InferredType::String));
        assert_eq!(steps.get("b"), Some(&InferredType::Unknown));
        let inputs = infer_all_input_types(&w);
        assert_eq!(inputs.get("topic"), Some(&InferredType::String));
    }
}
