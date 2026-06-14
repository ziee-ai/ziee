//! workflow.yaml parser + minimal validation surface for B2.
//!
//! Phase B2 deliverable: deserialize the YAML into a typed `WorkflowDef`
//! that captures the §1 schema, and cycle-check `depends_on`. That's
//! the minimum the install handlers need to reject malformed bundles
//! at install time.
//!
//! TODO B4 — Layer 2/3 validators per plan §4.1:
//! - Layer 1 JSON Schema (jsonschema-rs against the vendored
//!   `workflow-definition.schema.json`) — currently subsumed by serde
//!   deserialization; should add the schema-driven path for clearer
//!   error messages.
//! - Layer 2 semantic (template reference resolution, prompt vs
//!   prompt_file mutual exclusion re-check, prompt_file path
//!   resolution within bundle).
//! - Layer 3 security (path traversal, flavor in `KNOWN_FLAVORS`,
//!   reject `mock:` outside dev workflows).
//! - The "compiled IR" pass that fills `workflows.compiled_ir_json`
//!   (plan §4.1 pattern (d)).

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::common::AppError;

// ============================================================
// Typed shape (mirrors plan §1)
// ============================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct WorkflowDef {
    #[serde(default, rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxDecl>,
    #[serde(default = "default_expose_logs")]
    pub expose_logs: ExposeLogs,
    #[serde(default)]
    pub inputs: Vec<InputDef>,
    #[serde(default)]
    pub steps: Vec<StepDef>,
    #[serde(default)]
    pub outputs: Vec<OutputDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SandboxDecl {
    pub flavor: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExposeLogs {
    Always,
    #[default]
    OnError,
    Never,
}

fn default_expose_logs() -> ExposeLogs {
    ExposeLogs::OnError
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InputDef {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StepDef {
    pub id: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default)]
    pub log: LogCapture,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expose_logs: Option<ExposeLogs>,
    /// Dev-only canned response. Honored only when
    /// `workflows.is_dev = true`. Publisher's `validate.py` rejects
    /// any step carrying this field. TODO B4 — runtime gate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mock: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ArtifactDecl>,
    /// The tagged union of step kinds (kind: llm | llm_map | sandbox |
    /// elicit). The `default kind = llm` rule is in plan §1.
    #[serde(flatten)]
    pub config: StepConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum LogCapture {
    #[default]
    Off,
    Stderr,
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StepConfig {
    Llm {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prompt: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prompt_file: Option<String>,
        #[serde(default)]
        output_format: OutputFormat,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tools: Vec<String>,
    },
    LlmMap {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prompt: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prompt_file: Option<String>,
        for_each: String,
        item_var: String,
        #[serde(default)]
        output_format: OutputFormat,
        #[serde(default = "default_max_parallel")]
        max_parallel: u32,
        #[serde(default)]
        on_error: OnError,
        #[serde(default)]
        max_retries: u32,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tools: Vec<String>,
    },
    Sandbox {
        run: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stdin: Option<String>,
        #[serde(default = "default_sandbox_timeout_ms")]
        timeout_ms: u32,
    },
    Elicit {
        message: String,
        schema: serde_json::Value,
        #[serde(default = "default_elicit_timeout_ms")]
        timeout_ms: u32,
    },
}

fn default_max_parallel() -> u32 {
    5
}
fn default_sandbox_timeout_ms() -> u32 {
    30_000
}
fn default_elicit_timeout_ms() -> u32 {
    300_000
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum OnError {
    #[default]
    Fail,
    Skip,
    Retry,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactDecl {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glob: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OutputDef {
    pub name: String,
    pub from: String,
    #[serde(default = "default_expose_mode")]
    pub expose: ExposeMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExposeMode {
    #[default]
    Full,
    Preview,
    Artifact,
    Path,
    Hidden,
}

fn default_expose_mode() -> ExposeMode {
    ExposeMode::Full
}

// ============================================================
// Parser + cycle check (B2 minimum)
// ============================================================

/// Parse + structurally-validate a `workflow.yaml` body. Returns the
/// typed `WorkflowDef` on success.
///
/// B2 contract — the install handler MUST call this so a malformed
/// bundle is rejected before the row hits the DB. The full Layer 2+3
/// validator suite (B4) layers on top of this base structural check.
pub fn parse_workflow_yaml(yaml: &str) -> Result<WorkflowDef, AppError> {
    let workflow: WorkflowDef = serde_yaml::from_str(yaml).map_err(|e| {
        AppError::bad_request(
            "WORKFLOW_INVALID_YAML",
            format!("workflow.yaml deserialization failed: {e}"),
        )
    })?;
    cycle_check(&workflow)?;
    Ok(workflow)
}

/// Toposort + reject cycles in `steps[*].depends_on`. Used by the
/// install handler.
pub fn cycle_check(workflow: &WorkflowDef) -> Result<(), AppError> {
    // Build name -> step idx map; validate dep targets exist.
    let mut step_idx: HashMap<&str, usize> = HashMap::with_capacity(workflow.steps.len());
    for (idx, step) in workflow.steps.iter().enumerate() {
        if step_idx.insert(step.id.as_str(), idx).is_some() {
            return Err(AppError::bad_request(
                "WORKFLOW_DUPLICATE_STEP_ID",
                format!("workflow.yaml: duplicate step id '{}'", step.id),
            ));
        }
    }
    for step in &workflow.steps {
        for dep in &step.depends_on {
            if !step_idx.contains_key(dep.as_str()) {
                return Err(AppError::bad_request(
                    "WORKFLOW_UNKNOWN_DEPENDENCY",
                    format!(
                        "workflow.yaml: step '{}' depends_on unknown step '{}'",
                        step.id, dep
                    ),
                ));
            }
        }
    }

    // Iterative DFS with 3-color marking: 0 = unvisited, 1 = in-stack,
    // 2 = done. A back-edge to an in-stack node is a cycle.
    let n = workflow.steps.len();
    let mut color = vec![0u8; n];
    for start in 0..n {
        if color[start] != 0 {
            continue;
        }
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        color[start] = 1;
        while let Some((node, dep_idx)) = stack.last().copied() {
            let step = &workflow.steps[node];
            if dep_idx >= step.depends_on.len() {
                color[node] = 2;
                stack.pop();
                continue;
            }
            // advance dep cursor on this frame
            stack.last_mut().unwrap().1 = dep_idx + 1;

            let next_name = step.depends_on[dep_idx].as_str();
            let next = *step_idx.get(next_name).unwrap(); // validated above
            match color[next] {
                0 => {
                    color[next] = 1;
                    stack.push((next, 0));
                }
                1 => {
                    return Err(AppError::bad_request(
                        "WORKFLOW_CYCLE",
                        format!(
                            "workflow.yaml: depends_on cycle involving step '{}' -> '{}'",
                            workflow.steps[node].id, workflow.steps[next].id
                        ),
                    ));
                }
                _ => {} // already done — skip
            }
        }
    }

    // Quick duplicate-output-name check (cheap; outputs are small).
    let mut seen_out = HashSet::with_capacity(workflow.outputs.len());
    for out in &workflow.outputs {
        if !seen_out.insert(out.name.as_str()) {
            return Err(AppError::bad_request(
                "WORKFLOW_DUPLICATE_OUTPUT_NAME",
                format!("workflow.yaml: duplicate output name '{}'", out.name),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_llm_workflow() {
        let yaml = r#"
inputs:
  - name: topic
    required: true
steps:
  - id: gen
    kind: llm
    prompt: "say something about {{ inputs.topic }}"
outputs:
  - name: result
    from: "{{ gen.output }}"
"#;
        let wf = parse_workflow_yaml(yaml).expect("parse");
        assert_eq!(wf.steps.len(), 1);
        assert_eq!(wf.outputs.len(), 1);
    }

    #[test]
    fn rejects_unknown_dependency() {
        let yaml = r#"
steps:
  - id: a
    kind: llm
    prompt: "x"
    depends_on: [does_not_exist]
"#;
        let err = parse_workflow_yaml(yaml).unwrap_err();
        assert!(err.to_string().contains("unknown step"));
    }

    #[test]
    fn rejects_simple_cycle() {
        let yaml = r#"
steps:
  - id: a
    kind: llm
    prompt: "x"
    depends_on: [b]
  - id: b
    kind: llm
    prompt: "y"
    depends_on: [a]
"#;
        let err = parse_workflow_yaml(yaml).unwrap_err();
        assert!(err.to_string().contains("cycle"));
    }

    #[test]
    fn rejects_duplicate_step_id() {
        let yaml = r#"
steps:
  - id: a
    kind: llm
    prompt: "x"
  - id: a
    kind: llm
    prompt: "y"
"#;
        let err = parse_workflow_yaml(yaml).unwrap_err();
        assert!(err.to_string().contains("duplicate step id"));
    }

    #[test]
    fn accepts_sandbox_step() {
        let yaml = r#"
sandbox:
  flavor: minimal
steps:
  - id: build
    kind: sandbox
    run: "echo hello"
"#;
        let wf = parse_workflow_yaml(yaml).expect("parse");
        assert!(matches!(wf.steps[0].config, StepConfig::Sandbox { .. }));
    }
}
