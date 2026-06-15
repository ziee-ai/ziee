//! Type-aware reference validation (plan §4.1 pattern (b) — "Reference
//! validation", actionlint-style).
//!
//! Layer 2 in `validate.rs` already checks that a `{{ X.Y }}` reference
//! NAMES a real input or earlier step. This module adds the TYPE layer:
//! given the inferred output type of every step (`type_infer.rs`), it
//! checks that the *access path* in each reference is type-valid:
//!
//!   - `{{ X.output[N] }}`   → ERROR if X's output type is not
//!     Array / ArrayUnknown.
//!   - `{{ X.output.field }}` → ERROR if X's output is a *known* Object
//!     missing `field`; WARNING if X's output is ObjectUnknown / Unknown
//!     (the Phase-1 escape hatch — under-specified workflows aren't
//!     rejected); ERROR if X's output is a scalar/array (field access on
//!     a non-object).
//!   - `{{ X.path }}`        → always String; `[N]` / `.field` on it is
//!     an ERROR.
//!   - `{{ inputs.name }}`   → resolved against the input's inferred
//!     type (from its `default`, else Unknown → warnings only).
//!
//! When a reference lands on `Unknown` (e.g. an `llm` step with
//! `output_format: json`, or an input with no default), accesses degrade
//! to a WARNING rather than an ERROR — per plan §4.1: "validator emits a
//! warning rather than error … preserves the Phase-1 escape hatch."
//!
//! NOTE: this is additive on top of the name-level check in
//! `validate.rs::check_template_refs`. The name check still fires for
//! unknown step/input ids; this module assumes those passed and focuses
//! purely on type compatibility. Unknown ids are skipped here (no double
//! reporting).

#![allow(dead_code)]

use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::modules::workflow::type_infer::{
    infer_all_input_types, infer_all_step_types, InferredType,
};
use crate::modules::workflow::validate::{StepConfig, ValidationError, WorkflowDef};

static VAR_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\{\{\s*([^}]+?)\s*\}\}").expect("ref_check var regex"));

/// One access segment after the head, e.g. `.output`, `[2]`, `.title`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Access {
    Field(String),
    Index(usize),
}

/// A fully-parsed reference expression for type checking. Unlike
/// `template::scan_var_refs` (which collapses to `(head, field)`), this
/// retains the full access chain + index so the type checker can walk it.
#[derive(Debug, Clone)]
struct RefExpr {
    head: String,
    accesses: Vec<Access>,
}

/// Parse one `{{ … }}` body into a `RefExpr`. Drops the optional
/// `| filter` suffix (filters don't affect typing). Returns None on a
/// shape this module doesn't type-check (it doesn't re-report syntax
/// errors — `validate.rs` already runs the template scanner first).
fn parse_ref(body: &str) -> Option<RefExpr> {
    let lhs = body.split('|').next().unwrap_or(body).trim();
    if lhs.is_empty() {
        return None;
    }
    // Split into a leading identifier + a tail of `.field` / `[N]` access.
    let mut accesses = Vec::new();
    // Read head identifier (up to first `.` or `[`).
    let head_end = lhs
        .char_indices()
        .find(|(_, c)| *c == '.' || *c == '[')
        .map(|(i, _)| i)
        .unwrap_or(lhs.len());
    let head = lhs[..head_end].to_string();
    let mut rest = &lhs[head_end..];
    while !rest.is_empty() {
        if let Some(stripped) = rest.strip_prefix('.') {
            // `.field` — read until next `.` or `[`.
            let mut end = stripped.len();
            for (i, c) in stripped.char_indices() {
                if c == '.' || c == '[' {
                    end = i;
                    break;
                }
            }
            let field = &stripped[..end];
            if field.is_empty() {
                return None;
            }
            accesses.push(Access::Field(field.to_string()));
            rest = &stripped[end..];
        } else if let Some(stripped) = rest.strip_prefix('[') {
            // `[N]` — read until `]`.
            let close = stripped.find(']')?;
            let idx: usize = stripped[..close].trim().parse().ok()?;
            accesses.push(Access::Index(idx));
            rest = &stripped[close + 1..];
        } else {
            return None;
        }
    }
    if head.is_empty() {
        return None;
    }
    Some(RefExpr { head, accesses })
}

/// Run type-aware checks over every template body in the workflow.
/// Produces a mix of error- and warning-severity `ValidationError`s
/// (severity carried by the new `ValidationError.severity` field).
pub fn check_typed_refs(workflow: &WorkflowDef) -> Vec<ValidationError> {
    let step_types = infer_all_step_types(workflow);
    let input_types = infer_all_input_types(workflow);
    let mut out = Vec::new();

    let check_body = |loc: &str, body_text: &str, out: &mut Vec<ValidationError>| {
        for cap in VAR_RE.captures_iter(body_text) {
            let inner = cap.get(1).unwrap().as_str();
            let Some(expr) = parse_ref(inner) else {
                continue;
            };
            check_one_ref(&expr, loc, &step_types, &input_types, out);
        }
    };

    for s in &workflow.steps {
        match &s.config {
            StepConfig::Llm { prompt, .. } => {
                if let Some(p) = prompt {
                    check_body(&format!("{}.prompt", s.id), p, &mut out);
                }
            }
            StepConfig::LlmMap {
                prompt, for_each, ..
            } => {
                check_body(&format!("{}.for_each", s.id), for_each, &mut out);
                // Special: for_each must resolve to an array.
                check_for_each_type(s, for_each, &step_types, &input_types, &mut out);
                if let Some(p) = prompt {
                    check_body(&format!("{}.prompt", s.id), p, &mut out);
                }
            }
            StepConfig::Sandbox { run, stdin, .. } => {
                check_body(&format!("{}.run", s.id), run, &mut out);
                if let Some(st) = stdin {
                    check_body(&format!("{}.stdin", s.id), st, &mut out);
                }
            }
            StepConfig::Elicit { .. } => {}
        }
        if let Some(msg) = &s.message {
            check_body(&format!("{}.message", s.id), msg, &mut out);
        }
    }
    for o in &workflow.outputs {
        check_body(&format!("outputs[{}].from", o.name), &o.from, &mut out);
    }
    out
}

/// `for_each` must point at an array. We can resolve the head's type and
/// walk the access chain; if the final type is a known non-array → error;
/// if Unknown → warning (escape hatch).
fn check_for_each_type(
    step: &crate::modules::workflow::validate::StepDef,
    for_each: &str,
    step_types: &HashMap<String, InferredType>,
    input_types: &HashMap<String, InferredType>,
    out: &mut Vec<ValidationError>,
) {
    // Extract the single `{{ … }}` (for_each is "exactly one template").
    let Some(cap) = VAR_RE.captures(for_each) else {
        return;
    };
    let Some(expr) = parse_ref(cap.get(1).unwrap().as_str()) else {
        return;
    };
    let Some(ty) = resolve_ref_type(&expr, step_types, input_types) else {
        return; // unknown id — name check in validate.rs reports it
    };
    match ty {
        InferredType::Array(_) | InferredType::ArrayUnknown => {}
        InferredType::Unknown => {
            out.push(ValidationError::warn(
                "semantic",
                "WORKFLOW_FOR_EACH_TYPE_UNRESOLVED",
                format!(
                    "llm_map for_each '{}' has unresolved type (under-specified upstream output); ensure it yields an array at runtime",
                    for_each.trim()
                ),
                format!("{}.for_each", step.id),
            ));
        }
        other => {
            out.push(ValidationError::at(
                "semantic",
                "WORKFLOW_FOR_EACH_NOT_ARRAY",
                format!(
                    "llm_map for_each '{}' resolves to {} but must be an array",
                    for_each.trim(),
                    other.label()
                ),
                format!("{}.for_each", step.id),
            ));
        }
    }
}

/// Resolve the *final* type of a reference after walking its access
/// chain — or None when the head is an unknown id (skip; name check
/// owns that error). Reports type errors/warnings into `out` along the
/// way (so a partial walk that hits a type wall reports it). Returns
/// the resolved final type when the walk completes cleanly.
fn check_one_ref(
    expr: &RefExpr,
    loc: &str,
    step_types: &HashMap<String, InferredType>,
    input_types: &HashMap<String, InferredType>,
    out: &mut Vec<ValidationError>,
) {
    // `inputs.<name>` head.
    if expr.head == "inputs" {
        // First access is the input name (a Field). Resolve its type, then
        // walk any remaining accesses.
        let Some(Access::Field(name)) = expr.accesses.first() else {
            return;
        };
        let Some(base_ty) = input_types.get(name).cloned() else {
            return; // unknown input — name check owns it
        };
        walk_accesses(&base_ty, &expr.accesses[1..], loc, expr, out);
        return;
    }

    // `<step_id>` head. First access must be `.output` or `.path`.
    let Some(base_ty) = step_types.get(&expr.head).cloned() else {
        return; // unknown step — name check owns it
    };
    let Some(first) = expr.accesses.first() else {
        return;
    };
    match first {
        Access::Field(f) if f == "path" => {
            // `.path` is always String. Any further access is an error.
            if expr.accesses.len() > 1 {
                out.push(ValidationError::at(
                    "semantic",
                    "WORKFLOW_PATH_ACCESS",
                    format!(
                        "'{}.path' is a string; cannot access '.{}'/index on it",
                        expr.head,
                        access_label(&expr.accesses[1])
                    ),
                    loc.to_string(),
                ));
            }
        }
        Access::Field(f) if f == "output" => {
            walk_accesses(&base_ty, &expr.accesses[1..], loc, expr, out);
        }
        // Any other first field is reported by validate.rs's name check.
        _ => {}
    }
}

/// Walk a chain of accesses (`.field` / `[N]`) against a starting type,
/// reporting the first type incompatibility. `Unknown` / `ObjectUnknown`
/// degrade to warnings.
fn walk_accesses(
    start: &InferredType,
    accesses: &[Access],
    loc: &str,
    expr: &RefExpr,
    out: &mut Vec<ValidationError>,
) {
    let mut cur = start.clone();
    for acc in accesses {
        match acc {
            Access::Index(_) => match &cur {
                InferredType::Array(inner) => cur = (**inner).clone(),
                InferredType::ArrayUnknown => cur = InferredType::Unknown,
                InferredType::Unknown => {
                    out.push(ValidationError::warn(
                        "semantic",
                        "WORKFLOW_REF_INDEX_UNRESOLVED",
                        format!(
                            "'{}' indexed with [{}] but its type is unresolved; ensure it is an array at runtime",
                            render_expr(expr),
                            access_label(acc)
                        ),
                        loc.to_string(),
                    ));
                    cur = InferredType::Unknown;
                }
                other => {
                    out.push(ValidationError::at(
                        "semantic",
                        "WORKFLOW_REF_INDEX_NON_ARRAY",
                        format!(
                            "'{}' indexed with [{}] but resolves to {} (not an array)",
                            render_expr(expr),
                            access_label(acc),
                            other.label()
                        ),
                        loc.to_string(),
                    ));
                    return;
                }
            },
            Access::Field(field) => match &cur {
                InferredType::Object(fields) => match fields.get(field) {
                    Some(t) => cur = t.clone(),
                    None => {
                        out.push(ValidationError::at(
                            "semantic",
                            "WORKFLOW_REF_UNKNOWN_FIELD",
                            format!(
                                "'{}' accesses field '.{}' which is not present on object {{{}}}",
                                render_expr(expr),
                                field,
                                fields.keys().cloned().collect::<Vec<_>>().join(", ")
                            ),
                            loc.to_string(),
                        ));
                        return;
                    }
                },
                InferredType::ObjectUnknown | InferredType::Unknown => {
                    out.push(ValidationError::warn(
                        "semantic",
                        "WORKFLOW_REF_FIELD_UNRESOLVED",
                        format!(
                            "'{}' accesses field '.{}' but the object shape is unknown; cannot type-check (ensure the field exists at runtime)",
                            render_expr(expr),
                            field
                        ),
                        loc.to_string(),
                    ));
                    cur = InferredType::Unknown;
                }
                other => {
                    out.push(ValidationError::at(
                        "semantic",
                        "WORKFLOW_REF_FIELD_NON_OBJECT",
                        format!(
                            "'{}' accesses field '.{}' but resolves to {} (not an object)",
                            render_expr(expr),
                            field,
                            other.label()
                        ),
                        loc.to_string(),
                    ));
                    return;
                }
            },
        }
    }
}

/// Resolve a reference's final type without emitting diagnostics (used
/// by `check_for_each_type`). Returns None on an unknown id.
fn resolve_ref_type(
    expr: &RefExpr,
    step_types: &HashMap<String, InferredType>,
    input_types: &HashMap<String, InferredType>,
) -> Option<InferredType> {
    let (mut cur, rest) = if expr.head == "inputs" {
        let Some(Access::Field(name)) = expr.accesses.first() else {
            return None;
        };
        (input_types.get(name).cloned()?, &expr.accesses[1..])
    } else {
        let base = step_types.get(&expr.head).cloned()?;
        // For a step head, the first access is `.output` / `.path`.
        match expr.accesses.first() {
            Some(Access::Field(f)) if f == "path" => return Some(InferredType::String),
            Some(Access::Field(f)) if f == "output" => (base, &expr.accesses[1..]),
            _ => return None,
        }
    };
    for acc in rest {
        cur = match (acc, &cur) {
            (Access::Index(_), InferredType::Array(inner)) => (**inner).clone(),
            (Access::Index(_), InferredType::ArrayUnknown) => InferredType::Unknown,
            (Access::Field(f), InferredType::Object(fields)) => {
                fields.get(f).cloned().unwrap_or(InferredType::Unknown)
            }
            _ => InferredType::Unknown,
        };
    }
    Some(cur)
}

fn access_label(a: &Access) -> String {
    match a {
        Access::Field(f) => f.clone(),
        Access::Index(i) => i.to_string(),
    }
}

fn render_expr(expr: &RefExpr) -> String {
    let mut s = expr.head.clone();
    for a in &expr.accesses {
        match a {
            Access::Field(f) => {
                s.push('.');
                s.push_str(f);
            }
            Access::Index(i) => {
                s.push('[');
                s.push_str(&i.to_string());
                s.push(']');
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::workflow::validate::{parse_workflow_yaml, Severity};

    fn wf(yaml: &str) -> WorkflowDef {
        parse_workflow_yaml(yaml).expect("parse")
    }

    fn errors(out: &[ValidationError]) -> Vec<&ValidationError> {
        out.iter().filter(|e| e.severity == Severity::Error).collect()
    }
    fn warnings(out: &[ValidationError]) -> Vec<&ValidationError> {
        out.iter()
            .filter(|e| e.severity == Severity::Warning)
            .collect()
    }

    #[test]
    fn index_on_string_output_is_error() {
        // gen is llm/text → String; referencing gen.output[0] is a type error.
        let w = wf(r#"
steps:
  - id: gen
    kind: llm
    prompt: "x"
    output_format: text
  - id: use
    kind: llm
    prompt: "{{ gen.output[0] }}"
    depends_on: [gen]
"#);
        let out = check_typed_refs(&w);
        assert!(
            errors(&out)
                .iter()
                .any(|e| e.code == "WORKFLOW_REF_INDEX_NON_ARRAY"),
            "expected index-on-string error, got {out:?}"
        );
    }

    #[test]
    fn field_on_unknown_output_is_warning() {
        // gen is llm/json → Unknown; gen.output.title degrades to warning.
        let w = wf(r#"
steps:
  - id: gen
    kind: llm
    prompt: "x"
    output_format: json
  - id: use
    kind: llm
    prompt: "{{ gen.output.title }}"
    depends_on: [gen]
"#);
        let out = check_typed_refs(&w);
        assert!(
            errors(&out).is_empty(),
            "unknown-shape field access must NOT error: {out:?}"
        );
        assert!(
            warnings(&out)
                .iter()
                .any(|e| e.code == "WORKFLOW_REF_FIELD_UNRESOLVED"),
            "expected field-unresolved warning, got {out:?}"
        );
    }

    #[test]
    fn valid_array_index_on_llm_map_ok() {
        // fan is llm_map → Array; fan.output[0] is type-valid (no diag).
        let w = wf(r#"
steps:
  - id: gen
    kind: llm
    prompt: "x"
  - id: fan
    kind: llm_map
    for_each: "{{ inputs.qs }}"
    item_var: q
    prompt: "{{ q }}"
    output_format: text
    depends_on: [gen]
  - id: use
    kind: llm
    prompt: "first: {{ fan.output[0] }}"
    depends_on: [fan]
inputs:
  - name: qs
    default: ["a", "b"]
"#);
        let out = check_typed_refs(&w);
        assert!(errors(&out).is_empty(), "unexpected errors: {out:?}");
    }

    #[test]
    fn known_object_field_present_ok_absent_error() {
        // elicit schema declares {proceed: bool}; .proceed ok, .nope error.
        let w = wf(r#"
steps:
  - id: confirm
    kind: elicit
    message: "go?"
    schema:
      type: object
      properties:
        proceed: { type: boolean }
      required: [proceed]
  - id: ok_ref
    kind: llm
    prompt: "{{ confirm.output.proceed }}"
    depends_on: [confirm]
  - id: bad_ref
    kind: llm
    prompt: "{{ confirm.output.nope }}"
    depends_on: [confirm]
"#);
        let out = check_typed_refs(&w);
        assert!(
            errors(&out)
                .iter()
                .any(|e| e.code == "WORKFLOW_REF_UNKNOWN_FIELD"
                    && e.location.as_deref() == Some("bad_ref.prompt")),
            "expected unknown-field error on bad_ref, got {out:?}"
        );
        // The valid .proceed access must not error.
        assert!(
            !errors(&out)
                .iter()
                .any(|e| e.location.as_deref() == Some("ok_ref.prompt")),
            "valid object field access must not error: {out:?}"
        );
    }

    #[test]
    fn for_each_on_string_is_error() {
        // gen is llm/text → String; for_each over it must error.
        let w = wf(r#"
steps:
  - id: gen
    kind: llm
    prompt: "x"
    output_format: text
  - id: fan
    kind: llm_map
    for_each: "{{ gen.output }}"
    item_var: q
    prompt: "{{ q }}"
    depends_on: [gen]
"#);
        let out = check_typed_refs(&w);
        assert!(
            errors(&out)
                .iter()
                .any(|e| e.code == "WORKFLOW_FOR_EACH_NOT_ARRAY"),
            "expected for-each-not-array error, got {out:?}"
        );
    }

    #[test]
    fn path_ref_is_string_no_diag() {
        let w = wf(r#"
sandbox:
  flavor: minimal
steps:
  - id: gen
    kind: llm
    prompt: "x"
  - id: build
    kind: sandbox
    run: "cat {{ gen.path }}"
    depends_on: [gen]
"#);
        let out = check_typed_refs(&w);
        assert!(errors(&out).is_empty(), "unexpected errors: {out:?}");
        assert!(warnings(&out).is_empty(), "unexpected warnings: {out:?}");
    }

    #[test]
    fn parse_ref_handles_chain_and_index() {
        let e = parse_ref("foo.output[2].title").unwrap();
        assert_eq!(e.head, "foo");
        assert_eq!(
            e.accesses,
            vec![
                Access::Field("output".into()),
                Access::Index(2),
                Access::Field("title".into()),
            ]
        );
    }
}
