//! workflow.yaml parser + Layer 1+2+3 validator (plan §4.1).
//!
//! Layer 1 (shape): structural validation via serde deserialization
//! into the typed `WorkflowDef`. The vendored JSON-Schema file is the
//! authoritative shape source on the publisher side; Rust serde
//! gives the consumer the same enforcement for free (typed enums for
//! `kind`, default values for omitted fields, mutually-exclusive
//! `prompt`/`prompt_file` via the `#[serde(flatten)]` tagged union).
//!
//! **Layer-1 jsonschema decision (Phase 8 G):** the plan §4.1 Layer 1
//! nominally calls for the `jsonschema` crate run against the vendored
//! `workflow-definition.schema.json`. We deliberately KEEP the serde
//! path instead of adding the crate, because:
//!   1. serde already gives EQUIVALENT shape enforcement (typed enums,
//!      defaults, the `prompt`/`prompt_file` flatten-mutex re-checked in
//!      `check_steps_shape`).
//!   2. the actual GOAL of plan §4.1 Layer 1 — publisher + consumer
//!      validators AGREE on shape — is now guaranteed by the shared
//!      `test-fixtures/` corpus (Layer 4 cross-fixture parity), not by
//!      both sides happening to call the same library.
//!   3. the vendored schema is draft-2020-12 with conditional
//!      `if/then`/`allOf` per-kind blocks; loading + compiling that with
//!      jsonschema-rs (a new workspace dep) is non-trivial and brings
//!      no behavioral win over serde + fixture-parity.
//! If a future need for literal schema fidelity arises (e.g. a third
//! Layer-1 consumer), revisit; for Phase 1 the equivalence holds.
//!
//! Layer 2 (semantic):
//! - step IDs unique + match `^[a-z][a-z0-9_]*$`,
//! - depends_on resolves + topo-sort succeeds (no cycles),
//! - every `{{ X.Y }}` template reference resolves (`X` is `inputs`
//!   with matching name, OR an earlier step in topo order),
//! - `prompt_file` paths exist in the bundle source,
//! - `prompt:` and `prompt_file:` mutually exclusive.
//!
//! Layer 3 (security):
//! - `prompt_file:` path safety (no `..`, no absolute, no symlink
//!   escape),
//! - `sandbox.flavor` value in `code_sandbox::KNOWN_FLAVORS`,
//! - reject `mock:` in non-dev workflows (called via `validate_for_install`).

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::path::Path;

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
    /// `workflows.is_dev = true`. Rejected at install for non-dev
    /// workflows (`validate_for_install` enforces).
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
        // NOTE: the elicitation PROMPT shown to the user is the shared
        // `StepDef.message` field (top-level on the step), NOT a nested
        // field here. A nested `message` would collide with
        // `StepDef.message` under `#[serde(flatten)]` (serde routes the
        // YAML `message` key to the outer field first, so the nested one
        // deserializes as missing). The workflow-definition.schema.json
        // already models `message` as a top-level step field for elicit.
        schema: serde_json::Value,
        #[serde(default = "default_elicit_timeout_ms")]
        timeout_ms: u32,
    },
}

impl StepConfig {
    pub fn kind_str(&self) -> &'static str {
        match self {
            StepConfig::Llm { .. } => "llm",
            StepConfig::LlmMap { .. } => "llm_map",
            StepConfig::Sandbox { .. } => "sandbox",
            StepConfig::Elicit { .. } => "elicit",
        }
    }
}

fn default_max_parallel() -> u32 {
    5
}
pub const MAX_PARALLEL_HARD_CAP: u32 = 20;
fn default_sandbox_timeout_ms() -> u32 {
    30_000
}
fn default_elicit_timeout_ms() -> u32 {
    300_000
}
pub const ELICIT_TIMEOUT_HARD_CAP_MS: u32 = 1_800_000;

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
// Validation
// ============================================================

/// Severity of a validation finding. Errors BLOCK install; warnings are
/// surfaced (e.g. via the `/validate` endpoint's `warnings` array) but do
/// NOT fail install — they preserve the Phase-1 escape hatch for
/// under-specified workflows (plan §4.1 pattern (b)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationError {
    pub layer: &'static str, // "schema" | "semantic" | "security"
    pub code: &'static str,
    pub message: String,
    /// Optional step id / output name / inputs.foo path for FE
    /// rendering.
    pub location: Option<String>,
    /// `Error` (blocks install) or `Warning` (surfaced, non-blocking).
    /// Defaults to `Error` for all existing call sites; only the
    /// type-aware ref checker (`ref_check.rs`) emits warnings.
    #[serde(default = "default_severity")]
    pub severity: Severity,
}

fn default_severity() -> Severity {
    Severity::Error
}

impl ValidationError {
    pub(crate) fn err<S: Into<String>>(layer: &'static str, code: &'static str, msg: S) -> Self {
        Self {
            layer,
            code,
            message: msg.into(),
            location: None,
            severity: Severity::Error,
        }
    }
    pub(crate) fn at<S: Into<String>, L: Into<String>>(
        layer: &'static str,
        code: &'static str,
        msg: S,
        loc: L,
    ) -> Self {
        Self {
            layer,
            code,
            message: msg.into(),
            location: Some(loc.into()),
            severity: Severity::Error,
        }
    }
    /// Warning-severity finding with a location. Surfaced but never
    /// blocks install (`validate_for_install` filters to errors only).
    pub(crate) fn warn<S: Into<String>, L: Into<String>>(
        layer: &'static str,
        code: &'static str,
        msg: S,
        loc: L,
    ) -> Self {
        Self {
            layer,
            code,
            message: msg.into(),
            location: Some(loc.into()),
            severity: Severity::Warning,
        }
    }
}

/// Parse YAML body. Layer 1 shape errors become AppError ("the install
/// handler short-circuits on the first parse failure"). For the
/// `/validate` REST surface (B6), use `validate_yaml_collecting` which
/// returns all errors.
pub fn parse_workflow_yaml(yaml: &str) -> Result<WorkflowDef, AppError> {
    serde_yaml::from_str::<WorkflowDef>(yaml).map_err(|e| {
        AppError::bad_request(
            "WORKFLOW_INVALID_YAML",
            format!("workflow.yaml deserialization failed: {e}"),
        )
    })
}

/// Full validator used by the install handler. Returns Ok on success,
/// or the first error as an AppError.
///
/// `bundle_root` is the extracted bundle dir (used for `prompt_file:`
/// path resolution).
/// `is_dev` controls whether `mock:` is allowed.
pub fn validate_for_install(
    workflow: &WorkflowDef,
    bundle_root: &Path,
    is_dev: bool,
) -> Result<(), AppError> {
    let findings = validate_collecting(workflow, bundle_root, is_dev);
    // Warnings (type-aware ref-check escape hatch) are surfaced via the
    // `/validate` endpoint but MUST NOT block install. Only errors fail.
    if let Some(first) = findings
        .into_iter()
        .find(|e| e.severity == Severity::Error)
    {
        return Err(AppError::bad_request(
            first.code,
            format!(
                "[{}/{}] {}{}",
                first.layer,
                first.code,
                first.location.map(|l| format!("{l}: ")).unwrap_or_default(),
                first.message
            ),
        ));
    }
    Ok(())
}

/// Same as `validate_for_install` but returns ALL errors. Used by
/// `/validate` REST endpoint (B6).
pub fn validate_collecting(
    workflow: &WorkflowDef,
    bundle_root: &Path,
    is_dev: bool,
) -> Vec<ValidationError> {
    let mut out = Vec::new();
    // Layer 2 + 3 — semantic + security.
    out.extend(check_steps_shape(workflow));
    out.extend(check_dependencies(workflow));
    out.extend(check_outputs(workflow));
    out.extend(check_template_refs(workflow));
    out.extend(check_prompt_files(workflow, bundle_root));
    out.extend(check_security(workflow));
    // Pattern (b): type-aware reference validation. Runs AFTER the
    // name-level `check_template_refs` so unknown ids are reported once
    // (the typed checker skips unknown ids). Emits a mix of errors
    // (definite type mismatches) + warnings (under-specified shapes).
    out.extend(crate::modules::workflow::ref_check::check_typed_refs(
        workflow,
    ));
    if !is_dev {
        out.extend(check_no_mock(workflow));
    }
    out
}

/// Topo-sort + cycle check kept as a standalone fn for tests + the
/// runner (it consumes the order at dispatch time).
pub fn topo_sort_steps(workflow: &WorkflowDef) -> Result<Vec<usize>, AppError> {
    let n = workflow.steps.len();
    let mut step_idx: HashMap<&str, usize> = HashMap::with_capacity(n);
    for (i, s) in workflow.steps.iter().enumerate() {
        step_idx.insert(s.id.as_str(), i);
    }
    // Kahn's algorithm. Stable order: by appearance.
    let mut indeg = vec![0u32; n];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, s) in workflow.steps.iter().enumerate() {
        for dep in &s.depends_on {
            let &j = step_idx.get(dep.as_str()).ok_or_else(|| {
                AppError::bad_request(
                    "WORKFLOW_UNKNOWN_DEPENDENCY",
                    format!("step '{}' depends_on unknown step '{}'", s.id, dep),
                )
            })?;
            adj[j].push(i);
            indeg[i] += 1;
        }
    }
    let mut queue: std::collections::VecDeque<usize> =
        indeg.iter().enumerate().filter(|(_, d)| **d == 0).map(|(i, _)| i).collect();
    let mut order = Vec::with_capacity(n);
    while let Some(i) = queue.pop_front() {
        order.push(i);
        for &j in &adj[i] {
            indeg[j] -= 1;
            if indeg[j] == 0 {
                queue.push_back(j);
            }
        }
    }
    if order.len() != n {
        return Err(AppError::bad_request(
            "WORKFLOW_CYCLE",
            "workflow.yaml: depends_on cycle detected",
        ));
    }
    Ok(order)
}

// Kept for backwards-compat with the B2 install handler.
pub fn cycle_check(workflow: &WorkflowDef) -> Result<(), AppError> {
    topo_sort_steps(workflow).map(|_| ())
}

// --- per-check helpers ---

fn check_steps_shape(workflow: &WorkflowDef) -> Vec<ValidationError> {
    let mut out = Vec::new();
    if workflow.steps.is_empty() {
        out.push(ValidationError::err(
            "schema",
            "WORKFLOW_NO_STEPS",
            "workflow.yaml: steps[] must contain at least one step",
        ));
    }
    if workflow.steps.len() > 50 {
        out.push(ValidationError::err(
            "semantic",
            "WORKFLOW_TOO_MANY_STEPS",
            format!(
                "workflow.yaml: {} steps exceeds Phase-1 cap of 50",
                workflow.steps.len()
            ),
        ));
    }
    let id_re = regex::Regex::new(r"^[a-z][a-z0-9_]*$").unwrap();
    let mut seen: HashSet<&str> = HashSet::new();
    for s in &workflow.steps {
        if !id_re.is_match(&s.id) {
            out.push(ValidationError::at(
                "schema",
                "WORKFLOW_BAD_STEP_ID",
                format!(
                    "step id '{}' must match ^[a-z][a-z0-9_]*$",
                    s.id
                ),
                &s.id,
            ));
        }
        if !seen.insert(s.id.as_str()) {
            out.push(ValidationError::at(
                "semantic",
                "WORKFLOW_DUPLICATE_STEP_ID",
                format!("duplicate step id '{}'", s.id),
                &s.id,
            ));
        }
        // Prompt vs prompt_file mutual exclusion (defense in depth on
        // top of #[serde(flatten)] which doesn't enforce oneOf).
        if let StepConfig::Llm {
            prompt, prompt_file, ..
        }
        | StepConfig::LlmMap {
            prompt, prompt_file, ..
        } = &s.config
        {
            let has_prompt = prompt.as_ref().filter(|s| !s.is_empty()).is_some();
            let has_file = prompt_file.is_some();
            if has_prompt && has_file {
                out.push(ValidationError::at(
                    "semantic",
                    "WORKFLOW_PROMPT_BOTH",
                    "step has both prompt: and prompt_file: (mutually exclusive)",
                    &s.id,
                ));
            }
            if !has_prompt && !has_file {
                out.push(ValidationError::at(
                    "semantic",
                    "WORKFLOW_PROMPT_MISSING",
                    "step has neither prompt: nor prompt_file:",
                    &s.id,
                ));
            }
        }
        if let StepConfig::Sandbox { run, .. } = &s.config {
            // Reject empty OR whitespace-only `run:` (a `run: "   "` would
            // otherwise pass `.is_empty()` yet produce a no-op `cd && `
            // command at dispatch). Plan §4 workflow_mcp + audit gap 7.
            if run.trim().is_empty() {
                out.push(ValidationError::at(
                    "semantic",
                    "WORKFLOW_SANDBOX_NO_RUN",
                    "sandbox step has empty run:",
                    &s.id,
                ));
            }
        }
        if let StepConfig::Elicit { timeout_ms, .. } = &s.config {
            if *timeout_ms > ELICIT_TIMEOUT_HARD_CAP_MS {
                out.push(ValidationError::at(
                    "semantic",
                    "WORKFLOW_ELICIT_TIMEOUT_CAP",
                    format!(
                        "elicit timeout_ms={} exceeds hard cap {}",
                        timeout_ms, ELICIT_TIMEOUT_HARD_CAP_MS
                    ),
                    &s.id,
                ));
            }
            // The elicitation prompt is the shared StepDef.message field.
            if s.message.as_deref().map(str::trim).unwrap_or("").is_empty() {
                out.push(ValidationError::at(
                    "semantic",
                    "WORKFLOW_ELICIT_NO_MESSAGE",
                    "elicit step requires a `message` (the prompt shown to the user)",
                    &s.id,
                ));
            }
        }
        // llm_map for_each separate check (avoid borrowing issue)
        if let StepConfig::LlmMap {
            max_parallel,
            for_each,
            item_var,
            ..
        } = &s.config
        {
            if *max_parallel > MAX_PARALLEL_HARD_CAP {
                out.push(ValidationError::at(
                    "semantic",
                    "WORKFLOW_PARALLEL_CAP",
                    format!(
                        "llm_map max_parallel={} exceeds hard cap {}",
                        max_parallel, MAX_PARALLEL_HARD_CAP
                    ),
                    &s.id,
                ));
            }
            if *max_parallel == 0 {
                out.push(ValidationError::at(
                    "semantic",
                    "WORKFLOW_PARALLEL_ZERO",
                    "llm_map max_parallel must be > 0",
                    &s.id,
                ));
            }
            if for_each.is_empty() {
                out.push(ValidationError::at(
                    "semantic",
                    "WORKFLOW_FOR_EACH_EMPTY",
                    "llm_map for_each must be a template referencing an array",
                    &s.id,
                ));
            } else if !for_each.contains("{{") {
                // L2: a non-template for_each (e.g. a bare literal) passes the
                // non-empty check but fails at runtime when the dispatcher
                // tries to parse it as an array. Reject at install with a
                // clear message.
                out.push(ValidationError::at(
                    "semantic",
                    "WORKFLOW_FOR_EACH_NOT_TEMPLATE",
                    "llm_map for_each must be a template referencing an array \
                     (e.g. \"{{ step_id.output }}\")",
                    &s.id,
                ));
            }
            if item_var.is_empty() {
                out.push(ValidationError::at(
                    "semantic",
                    "WORKFLOW_ITEM_VAR_EMPTY",
                    "llm_map item_var must be set",
                    &s.id,
                ));
            }
        }
    }
    out
}

fn check_dependencies(workflow: &WorkflowDef) -> Vec<ValidationError> {
    let mut out = Vec::new();
    let ids: HashSet<&str> = workflow.steps.iter().map(|s| s.id.as_str()).collect();
    for s in &workflow.steps {
        for dep in &s.depends_on {
            if !ids.contains(dep.as_str()) {
                out.push(ValidationError::at(
                    "semantic",
                    "WORKFLOW_UNKNOWN_DEPENDENCY",
                    format!("step '{}' depends_on unknown step '{}'", s.id, dep),
                    &s.id,
                ));
            }
            if dep == &s.id {
                out.push(ValidationError::at(
                    "semantic",
                    "WORKFLOW_SELF_DEPENDENCY",
                    "step depends_on itself",
                    &s.id,
                ));
            }
        }
    }
    if let Err(e) = topo_sort_steps(workflow) {
        out.push(ValidationError::err(
            "semantic",
            "WORKFLOW_CYCLE",
            e.to_string(),
        ));
    }
    out
}

fn check_outputs(workflow: &WorkflowDef) -> Vec<ValidationError> {
    let mut out = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    for o in &workflow.outputs {
        if !seen.insert(o.name.as_str()) {
            out.push(ValidationError::at(
                "semantic",
                "WORKFLOW_DUPLICATE_OUTPUT_NAME",
                format!("duplicate output name '{}'", o.name),
                &o.name,
            ));
        }
    }
    out
}

fn check_template_refs(workflow: &WorkflowDef) -> Vec<ValidationError> {
    let mut out = Vec::new();
    let input_names: HashSet<&str> =
        workflow.inputs.iter().map(|i| i.name.as_str()).collect();
    let step_ids: HashSet<&str> = workflow.steps.iter().map(|s| s.id.as_str()).collect();

    let mut check = |loc: &str, body: &str| {
        let refs = match crate::modules::workflow::template::scan_var_refs(body) {
            Ok(r) => r,
            Err(e) => {
                out.push(ValidationError::at(
                    "semantic",
                    "WORKFLOW_TEMPLATE_SYNTAX",
                    e.to_string(),
                    loc.to_string(),
                ));
                return;
            }
        };
        for (head, field) in refs {
            match head.as_str() {
                "inputs" => {
                    if !input_names.contains(field.as_str()) {
                        out.push(ValidationError::at(
                            "semantic",
                            "WORKFLOW_UNKNOWN_INPUT_REF",
                            format!("template references unknown input 'inputs.{field}'"),
                            loc.to_string(),
                        ));
                    }
                }
                step_id => {
                    if !step_ids.contains(step_id) {
                        out.push(ValidationError::at(
                            "semantic",
                            "WORKFLOW_UNKNOWN_STEP_REF",
                            format!(
                                "template references unknown step '{step_id}' (in '{step_id}.{field}')"
                            ),
                            loc.to_string(),
                        ));
                    } else if field != "output" && field != "path" {
                        // `field` is the LEADING access segment after the step
                        // head (`scan_var_refs` returns the first field only),
                        // so chained refs like `{{ s.output.proceed }}` or
                        // `{{ s.output[0] }}` carry leading field `output` and
                        // pass here — the deeper chain is type-checked by
                        // `ref_check.rs` and resolved by `template.rs` (C1).
                        // Only a non-output/path leading field (or a bare
                        // index directly on the step head) is an error.
                        out.push(ValidationError::at(
                            "semantic",
                            "WORKFLOW_BAD_STEP_FIELD",
                            format!(
                                "template references unknown field '{field}' on step '{step_id}' (expected 'output' or 'path')"
                            ),
                            loc.to_string(),
                        ));
                    }
                }
            }
        }
    };

    for s in &workflow.steps {
        let bodies: Vec<(String, &str)> = match &s.config {
            StepConfig::Llm { prompt, .. } => prompt
                .as_deref()
                .map(|p| vec![(format!("{}.prompt", s.id), p)])
                .unwrap_or_default(),
            StepConfig::LlmMap {
                prompt, for_each, ..
            } => {
                let mut v: Vec<(String, &str)> =
                    vec![(format!("{}.for_each", s.id), for_each.as_str())];
                if let Some(p) = prompt.as_deref() {
                    v.push((format!("{}.prompt", s.id), p));
                }
                v
            }
            StepConfig::Sandbox { run, stdin, .. } => {
                let mut v: Vec<(String, &str)> = vec![(format!("{}.run", s.id), run.as_str())];
                if let Some(st) = stdin.as_deref() {
                    v.push((format!("{}.stdin", s.id), st));
                }
                v
            }
            // Elicit's prompt is the shared StepDef.message, scanned below.
            StepConfig::Elicit { .. } => Vec::new(),
        };
        for (loc, body) in bodies {
            check(&loc, body);
        }
        if let Some(msg) = s.message.as_deref() {
            check(&format!("{}.message", s.id), msg);
        }
    }
    for o in &workflow.outputs {
        check(&format!("outputs[{}].from", o.name), &o.from);
    }
    out
}

fn check_prompt_files(workflow: &WorkflowDef, bundle_root: &Path) -> Vec<ValidationError> {
    let mut out = Vec::new();
    for s in &workflow.steps {
        let pf = match &s.config {
            StepConfig::Llm { prompt_file, .. } => prompt_file.as_deref(),
            StepConfig::LlmMap { prompt_file, .. } => prompt_file.as_deref(),
            _ => None,
        };
        if let Some(p) = pf {
            if p.contains("..") || p.starts_with('/') {
                out.push(ValidationError::at(
                    "security",
                    "WORKFLOW_PROMPT_FILE_UNSAFE",
                    format!("prompt_file '{p}' must be a bundle-relative path without '..'"),
                    &s.id,
                ));
                continue;
            }
            let resolved = bundle_root.join(p);
            // Defense: re-canonicalize and verify it's still inside the bundle root.
            match resolved.canonicalize() {
                Ok(canon) => {
                    let root_canon = bundle_root.canonicalize().unwrap_or(bundle_root.to_path_buf());
                    if !canon.starts_with(&root_canon) {
                        out.push(ValidationError::at(
                            "security",
                            "WORKFLOW_PROMPT_FILE_ESCAPE",
                            format!("prompt_file '{p}' resolves outside bundle"),
                            &s.id,
                        ));
                    }
                }
                Err(_) => {
                    out.push(ValidationError::at(
                        "semantic",
                        "WORKFLOW_PROMPT_FILE_MISSING",
                        format!("prompt_file '{p}' not found in bundle"),
                        &s.id,
                    ));
                }
            }
        }
    }
    out
}

fn check_security(workflow: &WorkflowDef) -> Vec<ValidationError> {
    let mut out = Vec::new();
    // sandbox.flavor must be in KNOWN_FLAVORS.
    if let Some(sb) = &workflow.sandbox {
        let known: Vec<&str> = crate::modules::code_sandbox::types::KNOWN_FLAVORS
            .iter()
            .map(|f| f.flavor)
            .collect();
        if !known.contains(&sb.flavor.as_str()) {
            out.push(ValidationError::err(
                "security",
                "WORKFLOW_UNKNOWN_FLAVOR",
                format!(
                    "sandbox.flavor '{}' is not in KNOWN_FLAVORS ({})",
                    sb.flavor,
                    known.join(", ")
                ),
            ));
        }
    }
    // If any step is `kind: sandbox`, sandbox.flavor MUST be declared.
    let has_sandbox = workflow
        .steps
        .iter()
        .any(|s| matches!(s.config, StepConfig::Sandbox { .. }));
    if has_sandbox && workflow.sandbox.is_none() {
        out.push(ValidationError::err(
            "semantic",
            "WORKFLOW_SANDBOX_FLAVOR_REQUIRED",
            "workflow has kind: sandbox steps but no top-level sandbox.flavor",
        ));
    }
    // Artifact declarations: path safety.
    for s in &workflow.steps {
        for a in &s.artifacts {
            if let Some(p) = a.path.as_deref()
                && (p.contains("..") || p.starts_with('/'))
            {
                out.push(ValidationError::at(
                    "security",
                    "WORKFLOW_ARTIFACT_PATH_UNSAFE",
                    format!("artifact path '{p}' must be relative, no '..'"),
                    &s.id,
                ));
            }
        }
    }
    out
}

fn check_no_mock(workflow: &WorkflowDef) -> Vec<ValidationError> {
    let mut out = Vec::new();
    for s in &workflow.steps {
        if s.mock.is_some() {
            out.push(ValidationError::at(
                "security",
                "WORKFLOW_MOCK_IN_PUBLISHED",
                "step has mock: set in a non-dev workflow (only dev workflows may carry mocks)",
                &s.id,
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(errs.is_empty(), "unexpected errors: {errs:?}");
    }

    #[test]
    fn elicit_step_deserializes_with_shared_message() {
        // Regression: `message` is StepDef's shared field; with
        // #[serde(flatten)] the elicit variant must NOT redeclare it or
        // the YAML key gets eaten by StepDef and the elicit step fails
        // to deserialize. The seed workflow answer-with-citations relies
        // on this.
        let yaml = r#"
inputs:
  - name: question
    required: true
steps:
  - id: confirm
    kind: elicit
    message: "Proceed with '{{ inputs.question }}'?"
    schema:
      type: object
      properties:
        proceed: { type: boolean }
      required: [proceed]
  - id: answer
    kind: llm
    prompt: "Answer: {{ inputs.question }} (confirmed: {{ confirm.output }})"
    depends_on: [confirm]
outputs:
  - name: result
    from: "{{ answer.output }}"
"#;
        let wf = parse_workflow_yaml(yaml).expect("elicit workflow must parse");
        assert_eq!(wf.steps.len(), 2);
        assert!(matches!(wf.steps[0].config, StepConfig::Elicit { .. }));
        assert_eq!(
            wf.steps[0].message.as_deref(),
            Some("Proceed with '{{ inputs.question }}'?")
        );
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(errs.is_empty(), "unexpected errors: {errs:?}");
    }

    #[test]
    fn sandbox_step_with_whitespace_only_run_rejected() {
        // Audit gap 7: a `run:` of only whitespace must be rejected (it
        // would otherwise produce a no-op `cd <dir> &&    ` at dispatch).
        let yaml = r#"
sandbox:
  flavor: minimal
steps:
  - id: build
    kind: sandbox
    run: "   \t  "
outputs:
  - name: result
    from: "{{ build.output }}"
"#;
        let wf = parse_workflow_yaml(yaml).expect("parse");
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(
            errs.iter().any(|e| e.code == "WORKFLOW_SANDBOX_NO_RUN"),
            "expected WORKFLOW_SANDBOX_NO_RUN for whitespace-only run, got: {errs:?}"
        );
    }

    #[test]
    fn sandbox_step_with_real_run_accepted() {
        let yaml = r#"
sandbox:
  flavor: minimal
steps:
  - id: build
    kind: sandbox
    run: "echo hi"
outputs:
  - name: result
    from: "{{ build.output }}"
"#;
        let wf = parse_workflow_yaml(yaml).expect("parse");
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(
            !errs.iter().any(|e| e.code == "WORKFLOW_SANDBOX_NO_RUN"),
            "non-empty run should not trip WORKFLOW_SANDBOX_NO_RUN: {errs:?}"
        );
    }

    #[test]
    fn elicit_step_without_message_rejected() {
        let yaml = r#"
steps:
  - id: confirm
    kind: elicit
    schema:
      type: object
"#;
        let wf = parse_workflow_yaml(yaml).expect("parse");
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(
            errs.iter().any(|e| e.code == "WORKFLOW_ELICIT_NO_MESSAGE"),
            "expected WORKFLOW_ELICIT_NO_MESSAGE, got: {errs:?}"
        );
    }

    #[test]
    fn rejects_cycle() {
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
        let wf = parse_workflow_yaml(yaml).unwrap();
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(
            errs.iter().any(|e| e.code == "WORKFLOW_CYCLE"),
            "expected WORKFLOW_CYCLE in {errs:?}"
        );
    }

    #[test]
    fn rejects_unknown_input_ref() {
        let yaml = r#"
steps:
  - id: g
    kind: llm
    prompt: "{{ inputs.missing }}"
"#;
        let wf = parse_workflow_yaml(yaml).unwrap();
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(errs.iter().any(|e| e.code == "WORKFLOW_UNKNOWN_INPUT_REF"));
    }

    #[test]
    fn rejects_unknown_step_ref_in_output() {
        let yaml = r#"
steps:
  - id: g
    kind: llm
    prompt: "x"
outputs:
  - name: o
    from: "{{ nope.output }}"
"#;
        let wf = parse_workflow_yaml(yaml).unwrap();
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(errs.iter().any(|e| e.code == "WORKFLOW_UNKNOWN_STEP_REF"));
    }

    #[test]
    fn chained_step_output_ref_not_bad_step_field() {
        // C1: `{{ confirm.output.proceed }}` (object readback) +
        // `{{ fan.output[0] }}` (array index) must NOT trip
        // WORKFLOW_BAD_STEP_FIELD — the leading field is `output`, the
        // deeper chain is template-resolvable + type-checked elsewhere.
        let yaml = r#"
steps:
  - id: confirm
    kind: elicit
    message: "go?"
    schema:
      type: object
      properties:
        proceed: { type: boolean }
      required: [proceed]
  - id: fan
    kind: llm_map
    for_each: "{{ inputs.qs }}"
    item_var: q
    prompt: "{{ q }}"
    depends_on: [confirm]
  - id: use
    kind: llm
    prompt: "go={{ confirm.output.proceed }} first={{ fan.output[0] }}"
    depends_on: [confirm, fan]
inputs:
  - name: qs
    default: ["a", "b"]
"#;
        let wf = parse_workflow_yaml(yaml).unwrap();
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(
            !errs.iter().any(|e| e.code == "WORKFLOW_BAD_STEP_FIELD"),
            "chained step.output refs must not trip WORKFLOW_BAD_STEP_FIELD: {errs:?}"
        );
    }

    #[test]
    fn rejects_prompt_and_prompt_file() {
        let yaml = r#"
steps:
  - id: g
    kind: llm
    prompt: "inline"
    prompt_file: "prompts/x.md"
"#;
        let wf = parse_workflow_yaml(yaml).unwrap();
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(errs.iter().any(|e| e.code == "WORKFLOW_PROMPT_BOTH"));
    }

    #[test]
    fn rejects_unsafe_prompt_file() {
        let yaml = r#"
steps:
  - id: g
    kind: llm
    prompt_file: "../../etc/passwd"
"#;
        let wf = parse_workflow_yaml(yaml).unwrap();
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(errs.iter().any(|e| e.code == "WORKFLOW_PROMPT_FILE_UNSAFE"));
    }

    #[test]
    fn rejects_unknown_flavor() {
        let yaml = r#"
sandbox:
  flavor: galactic
steps:
  - id: r
    kind: sandbox
    run: "echo"
"#;
        let wf = parse_workflow_yaml(yaml).unwrap();
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(errs.iter().any(|e| e.code == "WORKFLOW_UNKNOWN_FLAVOR"));
    }

    #[test]
    fn sandbox_step_requires_flavor_decl() {
        let yaml = r#"
steps:
  - id: r
    kind: sandbox
    run: "echo"
"#;
        let wf = parse_workflow_yaml(yaml).unwrap();
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(errs.iter().any(|e| e.code == "WORKFLOW_SANDBOX_FLAVOR_REQUIRED"));
    }

    #[test]
    fn rejects_mock_in_non_dev() {
        let yaml = r#"
steps:
  - id: g
    kind: llm
    prompt: "x"
    mock: "canned response"
"#;
        let wf = parse_workflow_yaml(yaml).unwrap();
        let tmp = tempdir().unwrap();
        let errs_pub = validate_collecting(&wf, tmp.path(), false);
        assert!(errs_pub.iter().any(|e| e.code == "WORKFLOW_MOCK_IN_PUBLISHED"));
        // Allowed when is_dev = true.
        let errs_dev = validate_collecting(&wf, tmp.path(), true);
        assert!(!errs_dev.iter().any(|e| e.code == "WORKFLOW_MOCK_IN_PUBLISHED"));
    }

    #[test]
    fn topo_sort_returns_valid_order() {
        let yaml = r#"
steps:
  - id: a
    kind: llm
    prompt: "x"
  - id: b
    kind: llm
    prompt: "y"
    depends_on: [a]
  - id: c
    kind: llm
    prompt: "z"
    depends_on: [b]
"#;
        let wf = parse_workflow_yaml(yaml).unwrap();
        let order = topo_sort_steps(&wf).unwrap();
        // a must come before b before c.
        let pos_a = order.iter().position(|&i| wf.steps[i].id == "a").unwrap();
        let pos_b = order.iter().position(|&i| wf.steps[i].id == "b").unwrap();
        let pos_c = order.iter().position(|&i| wf.steps[i].id == "c").unwrap();
        assert!(pos_a < pos_b && pos_b < pos_c);
    }
}
