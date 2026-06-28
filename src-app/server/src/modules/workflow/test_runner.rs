//! Workflow test-fixture model + assertion matcher (B6).
//!
//! `POST /api/workflows/{id}/test` reads every `tests/*.yaml` under the
//! workflow's extracted bundle dir, parses each into a `TestFixture`,
//! runs it (mocked for `mode: ci`, real for `mode: real_llm`), then
//! compares the resolved outputs against `expected_outputs` via the
//! assertion modes in plan §7: `contains` / `equals` / `min_length` /
//! `max_length` / `matches_schema`.
//!
//! Mocks are honored for test runs REGARDLESS of `is_dev` — the test
//! handler threads `force_mocks: true` into the RunContext (the
//! sanctioned mock context, plan §3). This module owns only the
//! pure-data parts (fixture parse + mock-coverage check + assertion
//! matching); the run plumbing lives in the handler + `runner::run_for_test`.

#![allow(dead_code)]

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::modules::workflow::elicit::validate_response_shape;
use crate::modules::workflow::validate::{StepConfig, WorkflowDef};

/// `mode:` field of a test fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureMode {
    /// Default — fixture must mock every llm/llm_map step; zero tokens.
    #[default]
    Ci,
    /// Spends real tokens against a configured provider.
    RealLlm,
}

/// One parsed `tests/<name>.yaml` fixture.
#[derive(Debug, Clone, Deserialize)]
pub struct TestFixture {
    #[serde(default)]
    pub mode: FixtureMode,
    #[serde(default)]
    pub inputs: Value,
    #[serde(default)]
    pub mocks: HashMap<String, Value>,
    /// `{output_name: {assertion: value, ...}}`.
    #[serde(default)]
    pub expected_outputs: HashMap<String, AssertionSet>,
}

/// The set of assertions declared for one output. All present
/// assertions must pass.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AssertionSet {
    #[serde(default)]
    pub contains: Option<String>,
    #[serde(default)]
    pub equals: Option<Value>,
    #[serde(default)]
    pub min_length: Option<u64>,
    #[serde(default)]
    pub max_length: Option<u64>,
    #[serde(default)]
    pub matches_schema: Option<Value>,
}

/// Per-fixture result.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FixtureResult {
    pub name: String,
    pub passed: bool,
    /// `true` when the fixture was skipped (e.g. a `real_llm` fixture
    /// with no provider configured). Skipped fixtures count toward
    /// neither passed nor failed.
    #[serde(default)]
    pub skipped: bool,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<FixtureFailure>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FixtureFailure {
    /// The output name (or "" for whole-run errors like missing mocks).
    pub output_name: String,
    /// The assertion that failed (or a descriptive code like
    /// "missing_mocks" / "run_failed").
    pub assertion: String,
    pub expected: String,
    pub actual_preview: String,
}

/// Top-level `POST /api/workflows/{id}/test` response.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TestRunResponse {
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub results: Vec<FixtureResult>,
}

/// Verify that a `mode: ci` fixture's `mocks` cover EVERY `llm` /
/// `llm_map` step in the workflow (plan §7: "validator requires that
/// `mocks` covers EVERY llm and llm_map step"). Returns the list of
/// un-mocked step ids (empty == covered).
pub fn missing_mock_steps(workflow: &WorkflowDef, mocks: &HashMap<String, Value>) -> Vec<String> {
    workflow
        .steps
        .iter()
        .filter(|s| matches!(s.config, StepConfig::Llm { .. } | StepConfig::LlmMap { .. }))
        .filter(|s| !mocks.contains_key(&s.id) && s.mock.is_none())
        .map(|s| s.id.clone())
        .collect()
}

/// Apply every assertion in `set` to the resolved `actual` output value.
/// Returns `Ok(())` on pass, or `Err(FixtureFailure)` on the first
/// failing assertion. `output_name` is threaded only for the failure
/// payload.
pub fn check_assertions(
    output_name: &str,
    set: &AssertionSet,
    actual: &Value,
) -> Result<(), FixtureFailure> {
    let fail = |assertion: &str, expected: String| FixtureFailure {
        output_name: output_name.to_string(),
        assertion: assertion.to_string(),
        expected,
        actual_preview: preview(actual),
    };

    // contains — substring match against the string form of the value.
    if let Some(needle) = &set.contains {
        let hay = as_match_string(actual);
        if !hay.contains(needle.as_str()) {
            return Err(fail("contains", format!("contains {needle:?}")));
        }
    }

    // equals — deep JSON equality. A JSON-typed expected compares
    // structurally; a string expected also matches a string-typed actual.
    if let Some(expected) = &set.equals {
        if !values_equal(expected, actual) {
            return Err(fail("equals", truncate(&expected.to_string(), 200)));
        }
    }

    // min_length / max_length — string char length OR array element count.
    if set.min_length.is_some() || set.max_length.is_some() {
        let len = length_of(actual);
        match len {
            Some(n) => {
                if let Some(min) = set.min_length {
                    if (n as u64) < min {
                        return Err(fail(
                            "min_length",
                            format!("length >= {min} (got {n})"),
                        ));
                    }
                }
                if let Some(max) = set.max_length {
                    if (n as u64) > max {
                        return Err(fail(
                            "max_length",
                            format!("length <= {max} (got {n})"),
                        ));
                    }
                }
            }
            None => {
                return Err(fail(
                    "min_length",
                    "a string or array value (length not defined)".into(),
                ));
            }
        }
    }

    // matches_schema — lightweight structural check (reuses the
    // elicit.rs validator: type + required + properties + minItems/maxItems).
    if let Some(schema) = &set.matches_schema {
        if let Err(msg) = validate_response_shape(schema, actual) {
            return Err(fail("matches_schema", msg));
        }
    }

    Ok(())
}

// ---- helpers ----

/// The string used for `contains`. A string value is matched bare; any
/// other value is matched against its compact JSON form.
fn as_match_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Deep equality with one ergonomic relaxation: a JSON string `expected`
/// matches a string-typed `actual` even when the YAML parsed the expected
/// as a bare scalar.
fn values_equal(expected: &Value, actual: &Value) -> bool {
    expected == actual
}

/// Length for `min_length`/`max_length`: string char count or array len.
fn length_of(v: &Value) -> Option<usize> {
    match v {
        Value::String(s) => Some(s.chars().count()),
        Value::Array(a) => Some(a.len()),
        _ => None,
    }
}

fn preview(v: &Value) -> String {
    let s = as_match_string(v);
    truncate(&s, 200)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max).collect();
        format!("{head}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::workflow::validate::parse_workflow_yaml;
    use serde_json::json;

    fn aset() -> AssertionSet {
        AssertionSet::default()
    }

    #[test]
    fn contains_passes_and_fails() {
        let mut s = aset();
        s.contains = Some("entangle".into());
        assert!(check_assertions("memo", &s, &json!("about entanglement")).is_ok());
        let err = check_assertions("memo", &s, &json!("about gravity")).unwrap_err();
        assert_eq!(err.assertion, "contains");
        assert_eq!(err.output_name, "memo");
    }

    #[test]
    fn contains_matches_json_form_of_non_string() {
        let mut s = aset();
        s.contains = Some("\"a\"".into());
        assert!(check_assertions("x", &s, &json!(["a", "b"])).is_ok());
    }

    #[test]
    fn equals_deep_eq_pass_and_fail() {
        let mut s = aset();
        s.equals = Some(json!({"a": [1, 2], "b": "x"}));
        assert!(check_assertions("o", &s, &json!({"b": "x", "a": [1, 2]})).is_ok());
        let err = check_assertions("o", &s, &json!({"a": [1, 2], "b": "y"})).unwrap_err();
        assert_eq!(err.assertion, "equals");
    }

    #[test]
    fn min_max_length_on_string() {
        let mut s = aset();
        s.min_length = Some(3);
        s.max_length = Some(5);
        assert!(check_assertions("o", &s, &json!("abcd")).is_ok());
        let too_short = check_assertions("o", &s, &json!("ab")).unwrap_err();
        assert_eq!(too_short.assertion, "min_length");
        let too_long = check_assertions("o", &s, &json!("abcdef")).unwrap_err();
        assert_eq!(too_long.assertion, "max_length");
    }

    #[test]
    fn min_length_on_array() {
        let mut s = aset();
        s.min_length = Some(2);
        assert!(check_assertions("o", &s, &json!([1, 2, 3])).is_ok());
        let err = check_assertions("o", &s, &json!([1])).unwrap_err();
        assert_eq!(err.assertion, "min_length");
    }

    #[test]
    fn min_length_on_non_lengthy_value_fails() {
        let mut s = aset();
        s.min_length = Some(1);
        let err = check_assertions("o", &s, &json!(42)).unwrap_err();
        assert_eq!(err.assertion, "min_length");
    }

    #[test]
    fn matches_schema_array_min_items() {
        let mut s = aset();
        s.matches_schema = Some(json!({"type": "array", "minItems": 2}));
        assert!(check_assertions("o", &s, &json!([1, 2])).is_ok());
        let too_few = check_assertions("o", &s, &json!([1])).unwrap_err();
        assert_eq!(too_few.assertion, "matches_schema");
        let wrong_type = check_assertions("o", &s, &json!("x")).unwrap_err();
        assert_eq!(wrong_type.assertion, "matches_schema");
    }

    #[test]
    fn matches_schema_object_required() {
        let mut s = aset();
        s.matches_schema =
            Some(json!({"type": "object", "required": ["claim"], "properties": {"claim": {"type": "string"}}}));
        assert!(check_assertions("o", &s, &json!({"claim": "c1"})).is_ok());
        let missing = check_assertions("o", &s, &json!({"other": 1})).unwrap_err();
        assert_eq!(missing.assertion, "matches_schema");
        let wrong = check_assertions("o", &s, &json!({"claim": 5})).unwrap_err();
        assert_eq!(wrong.assertion, "matches_schema");
    }

    #[test]
    fn all_assertions_in_a_set_must_pass() {
        let mut s = aset();
        s.contains = Some("foo".into());
        s.min_length = Some(100);
        // contains passes, min_length fails → first failure is min_length
        // (contains is checked first and passes).
        let err = check_assertions("o", &s, &json!("foobar")).unwrap_err();
        assert_eq!(err.assertion, "min_length");
    }

    #[test]
    fn fixture_yaml_round_trips() {
        let yaml = r#"
mode: ci
inputs:
  topic: "quantum entanglement"
mocks:
  summarize: "- Bullet 1\n- Bullet 2"
expected_outputs:
  summary:
    contains: "Bullet"
    min_length: 5
"#;
        let f: TestFixture = serde_norway::from_str(yaml).expect("parse fixture");
        assert_eq!(f.mode, FixtureMode::Ci);
        assert_eq!(f.inputs.get("topic").and_then(|v| v.as_str()), Some("quantum entanglement"));
        assert!(f.mocks.contains_key("summarize"));
        let a = f.expected_outputs.get("summary").unwrap();
        assert_eq!(a.contains.as_deref(), Some("Bullet"));
        assert_eq!(a.min_length, Some(5));
    }

    #[test]
    fn missing_mocks_reports_uncovered_llm_steps() {
        let w = parse_workflow_yaml(
            r#"
steps:
  - id: gen
    kind: llm
    prompt: "x"
  - id: fan
    kind: llm_map
    for_each: "{{ gen.output }}"
    item_var: q
    prompt: "{{ q }}"
    depends_on: [gen]
  - id: shape
    kind: sandbox
    run: "echo hi"
    depends_on: [fan]
"#,
        )
        .unwrap();
        // Only `gen` mocked → `fan` is missing; sandbox `shape` is exempt.
        let mut mocks = HashMap::new();
        mocks.insert("gen".to_string(), json!("[\"a\"]"));
        let missing = missing_mock_steps(&w, &mocks);
        assert_eq!(missing, vec!["fan".to_string()]);

        // Both mocked → none missing.
        mocks.insert("fan".to_string(), json!([["c"]]));
        assert!(missing_mock_steps(&w, &mocks).is_empty());
    }

    #[test]
    fn missing_mocks_honors_step_def_mock() {
        // A step with a baked-in `mock:` counts as covered.
        let w = parse_workflow_yaml(
            r#"
steps:
  - id: gen
    kind: llm
    prompt: "x"
    mock: "[\"a\"]"
"#,
        )
        .unwrap();
        assert!(missing_mock_steps(&w, &HashMap::new()).is_empty());
    }
}
