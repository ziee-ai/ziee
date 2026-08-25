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
    /// Workflow-declared wall-clock cap in seconds. `None` → the engine default
    /// (`RUN_WALL_CLOCK`). `Some(0)` → UNBOUNDED (no wall-clock — for long runs on
    /// a user-owned machine). The effective value is live-adjustable per run via
    /// `PUT /workflow-runs/{id}/timeout`. The per-run token + output-byte caps stay
    /// as the resource backstops regardless.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_runtime_secs: Option<u64>,
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
    /// Author-facing, template-rendered label of what this step does
    /// ("Search the web for {{ inputs.topic }}"). Surfaced as the step
    /// title in the run progress UI for every kind. Distinct from
    /// `message` (the elicit prompt / dynamic status line). Optional;
    /// capped at `MAX_STEP_DESCRIPTION_CHARS`. Ref-checked at install
    /// like `message`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
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
        /// D2: optional seed for the elicit form, template-rendered against the
        /// run context with the SAME type-preserving renderer as `tool`
        /// arguments (a whole-value `{{ ref }}` resolves to its native JSON
        /// type). Lets a prior step's output (e.g. an AI screening table)
        /// pre-fill the reviewer's form. Surfaced on the pending record + SSE.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
        /// How long to wait for the human to submit, in ms. `0` = no timeout:
        /// the run parks indefinitely (a durable human gate that survives an
        /// app restart via durable resume). Any non-zero value is capped at
        /// `ELICIT_TIMEOUT_HARD_CAP_MS` (30 min). Default 5 min — unbounded
        /// must be explicit.
        #[serde(default = "default_elicit_timeout_ms")]
        timeout_ms: u32,
    },
    /// Call an MCP tool on an accessible server (A6). `server` is a stable
    /// NAME resolved at run time against the running user's accessible servers
    /// (built-ins by name, or own/group-assigned). `arguments` is a templated
    /// JSON object rendered against the run context (type-preserving for
    /// whole-value `{{ ref }}` substitutions).
    Tool {
        server: String,
        tool: String,
        #[serde(default)]
        arguments: serde_json::Value,
    },
    /// Run the shared `agent-core` loop as a workflow step (ITEM-18). The model
    /// drives an autonomous tool-use loop against the `servers` allow-list until
    /// it produces a final answer, hits `max_steps`, or a token cap. `prompt`
    /// (inline) / `prompt_file` (bundle-relative) is the initial user task;
    /// `system` is the optional system directive. Both `prompt`/`system` are
    /// template-rendered against the run context.
    Agent {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prompt: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prompt_file: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        system: Option<String>,
        /// Allow-list of MCP server NAMES the agent may call (resolved at run
        /// time against the user's accessible servers, exactly like a `tool`
        /// step's `server`). Empty ⇒ the agent runs with no tools.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        servers: Vec<String>,
        /// Iteration ceiling for the loop. Defaults to the `agent_admin_settings`
        /// `default_max_steps` (DEC-7 = 30) at authoring time; the dispatcher
        /// clamps it to the admin cap at run time.
        #[serde(default = "default_agent_max_steps")]
        max_steps: u32,
        #[serde(default)]
        output_format: OutputFormat,
    },
}

/// DEC-7: the agent-step iteration default (mirrors `agent_admin_settings`).
fn default_agent_max_steps() -> u32 {
    30
}

impl StepConfig {
    pub fn kind_str(&self) -> &'static str {
        match self {
            StepConfig::Llm { .. } => "llm",
            StepConfig::LlmMap { .. } => "llm_map",
            StepConfig::Sandbox { .. } => "sandbox",
            StepConfig::Elicit { .. } => "elicit",
            StepConfig::Tool { .. } => "tool",
            StepConfig::Agent { .. } => "agent",
        }
    }

    /// The `(prompt, prompt_file)` pair for the kinds that HAVE one, `None` for
    /// the kinds that do not.
    ///
    /// Exhaustive on purpose. Every site that needs this pair used to write its
    /// own open `if let`/`match` with a silent fallthrough (`_ => None`,
    /// `_ => (None, None)`), so a NEW step kind carrying a prompt would have been
    /// quietly skipped by the validator and the runner alike — the same
    /// two-places-decide-separately shape that produced the validate/run
    /// disagreement this rule exists to prevent. Adding a kind here is a compile
    /// error exactly once.
    pub fn prompt_fields(&self) -> Option<(&Option<String>, &Option<String>)> {
        match self {
            StepConfig::Llm {
                prompt, prompt_file, ..
            }
            | StepConfig::LlmMap {
                prompt, prompt_file, ..
            }
            | StepConfig::Agent {
                prompt, prompt_file, ..
            } => Some((prompt, prompt_file)),
            StepConfig::Sandbox { .. } | StepConfig::Elicit { .. } | StepConfig::Tool { .. } => {
                None
            }
        }
    }

    /// [`prompt_source`] for this step, or `None` for a kind with no prompt.
    ///
    /// Preferred over calling `prompt_source(a, b)` directly: the two arguments
    /// there are adjacent, same-typed and opposite in meaning, so swapping them
    /// compiles and turns the prompt text into a file path.
    pub fn prompt_source(&self) -> Option<PromptSource<'_>> {
        self.prompt_fields().map(|(p, f)| prompt_source(p, f))
    }
}

fn default_max_parallel() -> u32 {
    5
}
pub const MAX_PARALLEL_HARD_CAP: u32 = 20;
/// Max chars for a step's `description` template (raw, pre-render).
pub const MAX_STEP_DESCRIPTION_CHARS: usize = 200;
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

/// Every `ValidationError` code this module + `ref_check` can emit.
///
/// This is the REGISTRY half of the humanisation contract (ITEM-2). The
/// validator's own `message` is written in the wire vocabulary on purpose —
/// `validate_for_install` turns it into an `AppError` for install/import/run,
/// and `workflow_mcp::validate_workflow` serializes it for the MODEL — so the
/// author-facing rewording lives in the builder UI, keyed off these codes
/// (`ui/src/modules/workflow/components/builder/validationCopy.ts`).
///
/// The `validation_codes_are_registered_and_humanised` test below keeps all
/// three in lockstep: an emit site whose code is not listed here fails, and a
/// listed code with no human copy in the UI fails. That is what makes "no raw
/// schema language ever reaches the author" a checked property rather than a
/// one-off fix.
///
/// **`#[cfg(test)]` on purpose.** Nothing in production reads this list — the
/// guard derives the emitted set from the source itself, so a `pub` const here
/// would be public API with no caller (CODING_GUIDELINES §15) whose `pub` also
/// suppresses the `dead_code` lint. It is `pub(crate)` rather than private only
/// so the workflow module's own tests can compare against it (see `dispatch.rs`'s
/// PROMPT_CODES guard) — crate-internal, so the no-public-API-without-a-caller
/// reasoning above is unaffected. It stays as a hand-curated test fixture
/// because it is the guard's CANARY: the bidirectional `stale` assertion means
/// any future regression that makes the source scanner see FEWER emit sites
/// (a new file it forgets, a call shape it mis-lexes) fails loudly instead of
/// making the humanisation half vacuously pass.
#[cfg(test)]
pub(crate) const VALIDATION_CODES: &[&str] = &[
    // validate.rs — storability
    "WORKFLOW_NUL_CHARACTER",
    // validate.rs — whole-workflow shape
    "WORKFLOW_NO_STEPS",
    "WORKFLOW_TOO_MANY_STEPS",
    "WORKFLOW_SANDBOX_FLAVOR_REQUIRED",
    "WORKFLOW_UNKNOWN_FLAVOR",
    "WORKFLOW_CYCLE",
    "WORKFLOW_DUPLICATE_OUTPUT_NAME",
    // validate.rs — step identity
    "WORKFLOW_BAD_STEP_ID",
    "WORKFLOW_DUPLICATE_STEP_ID",
    "WORKFLOW_STEP_DESCRIPTION_TOO_LONG",
    // validate.rs — prompts
    "WORKFLOW_PROMPT_MISSING",
    "WORKFLOW_PROMPT_BOTH",
    "WORKFLOW_PROMPT_FILE_MISSING",
    "WORKFLOW_PROMPT_FILE_UNSAFE",
    "WORKFLOW_PROMPT_FILE_ESCAPE",
    // validate.rs — per-kind configuration
    "WORKFLOW_DEAD_TOOLS_FIELD",
    "WORKFLOW_SANDBOX_NO_RUN",
    "WORKFLOW_TOOL_NO_SERVER",
    "WORKFLOW_TOOL_NO_TOOL",
    "WORKFLOW_ELICIT_NO_MESSAGE",
    "WORKFLOW_ELICIT_TIMEOUT_CAP",
    "WORKFLOW_PARALLEL_CAP",
    "WORKFLOW_PARALLEL_ZERO",
    "WORKFLOW_FOR_EACH_EMPTY",
    "WORKFLOW_FOR_EACH_NOT_TEMPLATE",
    "WORKFLOW_ITEM_VAR_EMPTY",
    // validate.rs — dependencies
    "WORKFLOW_UNKNOWN_DEPENDENCY",
    "WORKFLOW_SELF_DEPENDENCY",
    // validate.rs — references
    "WORKFLOW_TEMPLATE_SYNTAX",
    "WORKFLOW_UNKNOWN_INPUT_REF",
    "WORKFLOW_UNKNOWN_STEP_REF",
    "WORKFLOW_BAD_STEP_FIELD",
    // validate.rs — security / publishing
    "WORKFLOW_ARTIFACT_PATH_UNSAFE",
    "WORKFLOW_MOCK_IN_PUBLISHED",
    // ref_check.rs — type-aware reference checking
    "WORKFLOW_FOR_EACH_TYPE_UNRESOLVED",
    "WORKFLOW_FOR_EACH_NOT_ARRAY",
    "WORKFLOW_PATH_ACCESS",
    "WORKFLOW_REF_INDEX_UNRESOLVED",
    "WORKFLOW_REF_INDEX_NON_ARRAY",
    "WORKFLOW_REF_UNKNOWN_FIELD",
    "WORKFLOW_REF_FIELD_UNRESOLVED",
    "WORKFLOW_REF_FIELD_NON_OBJECT",
];

/// Severity of a validation finding. Errors BLOCK install; warnings are
/// surfaced (e.g. via the `/validate` endpoint's `warnings` array) but do
/// NOT fail install — they preserve the Phase-1 escape hatch for
/// under-specified workflows (plan §4.1 pattern (b)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
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

// serde `default = "default_severity"` fn: invoked only when a `ValidationError`
// is deserialized (client-bound errors are usually only serialized).
#[allow(dead_code)]
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
    serde_norway::from_str::<WorkflowDef>(yaml).map_err(|e| {
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

/// `validate_for_install` for an async caller holding a REAL bundle.
///
/// `validate_collecting` reads every `prompt_file:` (bounded at
/// `MAX_PROMPT_FILE_BYTES` each) with blocking `std::fs`, so calling it directly
/// from an async fn parks a tokio worker for the duration.
///
/// EVERY caller holding a real bundle goes through here — `spawn_run` and
/// `resume_run` (runner), workflow install (hub), the dev import/write handlers,
/// and `workflow_mcp`'s workspace verbs, which carry the largest such root of all
/// (the conversation's sandbox workspace). The only callers left on the sync form
/// are the DRAFT-validation handlers, which pass a bundle root that does not
/// exist and therefore read nothing at all. Mirrors the same `spawn_blocking` the
/// runner already uses for the dispatch-side read.
pub async fn validate_for_install_async(
    workflow: &WorkflowDef,
    bundle_root: &Path,
    is_dev: bool,
) -> Result<(), AppError> {
    let wf = workflow.clone();
    let root = bundle_root.to_path_buf();
    tokio::task::spawn_blocking(move || validate_for_install(&wf, &root, is_dev))
        .await
        .map_err(|e| AppError::internal_error(format!("workflow: validation task failed: {e}")))?
}

/// `validate_collecting` for an async caller holding a REAL bundle.
///
/// Same reasoning as [`validate_for_install_async`]: this reads every
/// `prompt_file:` with blocking `std::fs`, so an async caller with a real bundle
/// must not run it inline.
pub async fn validate_collecting_async(
    workflow: &WorkflowDef,
    bundle_root: &Path,
    is_dev: bool,
) -> Result<Vec<ValidationError>, AppError> {
    let wf = workflow.clone();
    let root = bundle_root.to_path_buf();
    tokio::task::spawn_blocking(move || validate_collecting(&wf, &root, is_dev))
        .await
        .map_err(|e| AppError::internal_error(format!("workflow: validation task failed: {e}")))
}

/// Same as `validate_for_install` but returns ALL errors. Used by
/// `/validate` REST endpoint (B6).
pub fn validate_collecting(
    workflow: &WorkflowDef,
    bundle_root: &Path,
    is_dev: bool,
) -> Vec<ValidationError> {
    let mut out = Vec::new();
    // Layer 1 — storability. Runs first: a NUL makes the row unwritable
    // regardless of whether the def is otherwise semantically valid.
    out.extend(check_no_nul(workflow));
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
        // Step description cap (the raw template, pre-render). Keeps the
        // progress-UI label bounded; the rendered string is also capped FE-side.
        if let Some(desc) = s.description.as_deref() {
            if desc.chars().count() > MAX_STEP_DESCRIPTION_CHARS {
                out.push(ValidationError::at(
                    "semantic",
                    "WORKFLOW_STEP_DESCRIPTION_TOO_LONG",
                    format!(
                        "step '{}' description is {} chars (max {})",
                        s.id,
                        desc.chars().count(),
                        MAX_STEP_DESCRIPTION_CHARS
                    ),
                    &s.id,
                ));
            }
        }
        // Prompt vs prompt_file mutual exclusion (defense in depth on
        // top of #[serde(flatten)] which doesn't enforce oneOf).
        //
        // Decided by `prompt_source` — the SAME rule `dispatch.rs::load_raw_prompt`
        // uses — so a definition this check passes cannot fail the run for a
        // prompt reason, and one it rejects cannot quietly run (INV-1). The two
        // codes and their exact messages are unchanged: the builder's
        // author-facing copy is keyed off them.
        if let Some(source) = s.config.prompt_source() {
            match source {
                PromptSource::Both => out.push(ValidationError::at(
                    "semantic",
                    "WORKFLOW_PROMPT_BOTH",
                    PROMPT_BOTH_MESSAGE,
                    &s.id,
                )),
                PromptSource::Missing => out.push(ValidationError::at(
                    "semantic",
                    "WORKFLOW_PROMPT_MISSING",
                    PROMPT_MISSING_MESSAGE,
                    &s.id,
                )),
                PromptSource::Inline(_) | PromptSource::File(_) => {}
            }
        }
        // E6: the `tools:` field on llm/llm_map is dead (never read — the
        // LlmDispatcher builds its ChatRequest with no tools). Reject it with a
        // clear pointer to the `tool` step kind instead of silently ignoring it.
        if let StepConfig::Llm { tools, .. } | StepConfig::LlmMap { tools, .. } = &s.config
            && !tools.is_empty()
        {
            out.push(ValidationError::at(
                "semantic",
                "WORKFLOW_DEAD_TOOLS_FIELD",
                "`tools:` on an llm/llm_map step does nothing — use a separate \
                 `kind: tool` step to call an MCP tool",
                &s.id,
            ));
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
        if let StepConfig::Tool { server, tool, .. } = &s.config {
            if server.trim().is_empty() {
                out.push(ValidationError::at(
                    "semantic",
                    "WORKFLOW_TOOL_NO_SERVER",
                    "tool step has empty server:",
                    &s.id,
                ));
            }
            if tool.trim().is_empty() {
                out.push(ValidationError::at(
                    "semantic",
                    "WORKFLOW_TOOL_NO_TOOL",
                    "tool step has empty tool:",
                    &s.id,
                ));
            }
        }
        if let StepConfig::Elicit { timeout_ms, .. } = &s.config {
            // `0` is the explicit "no timeout / wait indefinitely" sentinel and
            // is always allowed; only a non-zero value is bounded by the cap.
            if *timeout_ms != 0 && *timeout_ms > ELICIT_TIMEOUT_HARD_CAP_MS {
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

    // `item_var` is the per-item binding of an llm_map step — valid as a head
    // ONLY inside that step's own prompt (e.g. `{{ aspect }}` /
    // `{{ aspect.field }}`); the item is opaque JSON so any field on it is
    // allowed and left to runtime.
    let mut check = |loc: &str, body: &str, item_var: Option<&str>| {
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
            if Some(head.as_str()) == item_var {
                continue;
            }
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
        // (loc, body, item_var-valid-in-this-body)
        let bodies: Vec<(String, &str, Option<&str>)> = match &s.config {
            StepConfig::Llm { prompt, .. } => prompt
                .as_deref()
                .map(|p| vec![(format!("{}.prompt", s.id), p, None)])
                .unwrap_or_default(),
            StepConfig::LlmMap {
                prompt,
                for_each,
                item_var,
                ..
            } => {
                // `for_each` is evaluated BEFORE the item is bound, so the
                // item_var is NOT in scope there — only in the per-item prompt.
                let mut v: Vec<(String, &str, Option<&str>)> =
                    vec![(format!("{}.for_each", s.id), for_each.as_str(), None)];
                if let Some(p) = prompt.as_deref() {
                    v.push((format!("{}.prompt", s.id), p, Some(item_var.as_str())));
                }
                v
            }
            StepConfig::Sandbox { run, stdin, .. } => {
                let mut v: Vec<(String, &str, Option<&str>)> =
                    vec![(format!("{}.run", s.id), run.as_str(), None)];
                if let Some(st) = stdin.as_deref() {
                    v.push((format!("{}.stdin", s.id), st, None));
                }
                v
            }
            // Elicit's prompt is the shared StepDef.message, scanned below; its
            // `data:` seed is template-rendered, so scan every string in it.
            StepConfig::Elicit { data, .. } => data
                .as_ref()
                .map(|d| {
                    collect_template_strings(d)
                        .into_iter()
                        .map(|s_ref| (format!("{}.data", s.id), s_ref, None))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            // Tool `arguments` is a templated JSON value — scan every string in it
            // so a typo'd `{{ ref }}` is caught at install, not at run time.
            StepConfig::Tool { arguments, .. } => collect_template_strings(arguments)
                .into_iter()
                .map(|s_ref| (format!("{}.arguments", s.id), s_ref, None))
                .collect(),
            // Agent's `prompt` + `system` are template-rendered (no item_var).
            StepConfig::Agent { prompt, system, .. } => {
                let mut v: Vec<(String, &str, Option<&str>)> = Vec::new();
                if let Some(p) = prompt.as_deref() {
                    v.push((format!("{}.prompt", s.id), p, None));
                }
                if let Some(sys) = system.as_deref() {
                    v.push((format!("{}.system", s.id), sys, None));
                }
                v
            }
        };
        for (loc, body, item_var) in bodies {
            check(&loc, body, item_var);
        }
        // The step message renders at step-start (before any item is bound),
        // so item_var is not in scope there.
        if let Some(msg) = s.message.as_deref() {
            check(&format!("{}.message", s.id), msg, None);
        }
        // `description` renders at step-start too (full ctx) + at run-start
        // (inputs only); same scope as message — no item_var.
        if let Some(desc) = s.description.as_deref() {
            check(&format!("{}.description", s.id), desc, None);
        }
    }
    for o in &workflow.outputs {
        check(&format!("outputs[{}].from", o.name), &o.from, None);
    }
    out
}

/// Collect every string leaf in a JSON value (recursively through arrays +
/// objects). Used to scan `tool.arguments` / `elicit.data` for `{{ }}` refs.
fn collect_template_strings(v: &serde_json::Value) -> Vec<&str> {
    fn walk<'a>(v: &'a serde_json::Value, out: &mut Vec<&'a str>) {
        match v {
            serde_json::Value::String(s) => out.push(s.as_str()),
            serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            serde_json::Value::Object(m) => m.values().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(v, &mut out);
    out
}

/// Where a step's wording comes from.
///
/// See [`prompt_source`] — this is the vocabulary of the ONE rule that both the
/// validator and the runner use to answer that question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptSource<'a> {
    /// An inline `prompt:`.
    Inline(&'a str),
    /// A bundle-relative `prompt_file:`.
    File(&'a str),
    /// Neither — the step has no wording at all.
    Missing,
    /// Both — `prompt:` and `prompt_file:` are mutually exclusive.
    Both,
}

/// The SINGLE source of truth for "where does this step's prompt come from".
///
/// `validate.rs` (which turns the answer into `WORKFLOW_PROMPT_BOTH` /
/// `WORKFLOW_PROMPT_MISSING`) and `dispatch.rs::load_raw_prompt` (which turns it
/// into the string actually sent to the model) MUST both go through here.
/// They used not to: the validator normalised an empty `prompt:` to "absent"
/// while the runner matched `(Option, Option)` raw, so a step carrying
/// `prompt: ""` beside a `prompt_file:` validated GREEN and then failed the RUN
/// with `has invalid prompt config` — which the builder's own
/// `WORKFLOW_PROMPT_BOTH` remedy ("clear the prompt box here to use the file")
/// told authors to produce. Deriving both from this one function is what makes
/// that class of disagreement unrepresentable rather than merely fixed.
///
/// **An EMPTY string is ABSENT, for both fields.** `prompt: ""` is how the
/// builder's own `WORKFLOW_PROMPT_BOTH` remedy ("clear the prompt box here to use
/// the file") used to reach the wire, and an empty `prompt_file:` names the bundle
/// directory itself, which can never be read. Whitespace is deliberately NOT
/// trimmed: `prompt: "   "` is a real (if odd) prompt, both sides already agreed
/// on it, and trimming would be an unforced behaviour change (DEC-3).
pub fn prompt_source<'a>(
    prompt: &'a Option<String>,
    prompt_file: &'a Option<String>,
) -> PromptSource<'a> {
    let inline = prompt.as_deref().filter(|s| !s.is_empty());
    let file = prompt_file_ref(prompt_file);
    match (inline, file) {
        (Some(p), None) => PromptSource::Inline(p),
        (None, Some(f)) => PromptSource::File(f),
        (Some(_), Some(_)) => PromptSource::Both,
        (None, None) => PromptSource::Missing,
    }
}

/// The text for `WORKFLOW_PROMPT_BOTH`. Shared with `dispatch.rs`, which reports
/// the same condition at run time when validation was bypassed — two hand-written
/// copies of one sentence is how the wording drifts.
pub const PROMPT_BOTH_MESSAGE: &str = "step has both prompt: and prompt_file: (mutually exclusive)";
/// The text for `WORKFLOW_PROMPT_MISSING`. Shared with `dispatch.rs` — see above.
pub const PROMPT_MISSING_MESSAGE: &str = "step has neither prompt: nor prompt_file:";

/// The `prompt_file:` half of [`prompt_source`], for the one caller that needs
/// the PATH regardless of whether the step also carries an inline `prompt:`
/// (`check_prompt_files`, which reports on the file even in a both-state).
///
/// Kept as the single definition of "is there a file here" so the emptiness rule
/// is not written twice: an empty path is ABSENT, because it resolves to the
/// bundle directory itself, which is never readable.
pub fn prompt_file_ref(prompt_file: &Option<String>) -> Option<&str> {
    prompt_file.as_deref().filter(|s| !s.is_empty())
}

/// Why a `prompt_file:` cannot be used as a step's wording.
///
/// Each variant maps 1:1 onto a validator finding code, and every one of them
/// also makes [`read_prompt_file`] fail — which is what keeps the validator's
/// verdict and the runner's outcome in step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptFileError {
    /// Path SHAPE is unsafe (`..` or absolute). Decidable without a bundle.
    Unsafe,
    /// Resolves outside the bundle root (e.g. via a symlink).
    Escape,
    /// Cannot be read as text: absent, a directory, not valid UTF-8, or
    /// unreadable. Carries the underlying reason.
    Unreadable(String),
    /// Reads successfully but is EMPTY — a prompt file with no prompt in it.
    Empty,
    /// Larger than `MAX_PROMPT_FILE_BYTES`. A prompt is text an author wrote;
    /// anything of this size is a mistake or an attempt to make the validator do
    /// unbounded work on every launch.
    TooLarge(u64),
}

/// Ceiling on a `prompt_file:`'s size.
///
/// The validator reads every `prompt_file:` on every install AND on every
/// `spawn_run`/`resume_run`, so without a cap an author-controlled file makes
/// both the validator and the runner do unbounded work and allocate unbounded
/// memory. 1 MiB is ~250k tokens of prose — far beyond any model's context and
/// far beyond any prompt anyone writes.
pub const MAX_PROMPT_FILE_BYTES: u64 = 1024 * 1024;

/// Open `rel` CONFINED beneath `root`, in ONE path resolution.
///
/// The confinement must survive an attacker who can rename directories under
/// `root` while this runs, and for the workspace surfaces that attacker exists:
/// `run_from_workspace` / `validate_from_workspace` pass the conversation's
/// code_sandbox workspace as the bundle root, and that directory is bind-mounted
/// READ-WRITE into the sandbox. A resolve-then-check-then-open sequence loses to
/// it no matter how the check is written — `canonicalize` + `starts_with` +
/// `O_NOFOLLOW` guards only the FINAL component, so swapping an INTERMEDIATE
/// directory for a symlink (`mv prompts prompts.bak; ln -s /etc prompts`) between
/// the check and the open reads a host file the server uid can see.
///
/// On Linux the kernel settles it in a single call: `openat2` with
/// `RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS` resolves relative to a directory fd
/// and refuses, atomically and on every component, to leave that directory.
/// `O_NONBLOCK` additionally makes the open of a FIFO return immediately instead
/// of parking the thread forever — the type is then rejected by the `fstat` on
/// the returned fd, which describes the very file that was opened rather than
/// whatever the path names a moment later.
///
/// Elsewhere — every non-Linux host, and Linux without `openat2` — it falls back
/// to: refuse a non-directory or symlinked ANCHOR, canonicalize, confine, then
/// open refusing a final-component symlink where the platform can. That closes
/// the swapped-root attack and the final-component one; it does NOT close a
/// racing INTERMEDIATE swap, because nothing short of a single confined
/// resolution can. Stated plainly rather than argued away: the confinement is
/// performed by the SERVER's kernel, so "the sandbox guest is Linux" does not
/// make the fallback safe — it is weaker, and the residual window is a race
/// against a directory rename inside the bundle root.
#[cfg(target_os = "linux")]
fn open_confined(root: &Path, rel: &str) -> Result<std::fs::File, PromptFileError> {
    use std::ffi::CString;
    use std::os::fd::FromRawFd;

    // The anchor must not be attacker-swappable either. A plain `File::open`
    // FOLLOWS symlinks, so `RESOLVE_BENEATH` would be enforced beneath whatever
    // `root` resolves to at that instant — and for the workspace surfaces the
    // LAST component of `root` is inside the directory bwrap bind-mounts
    // read-write at `/home/sandboxuser`. A sandbox step doing
    // `mv flow flowbak && ln -s / flow` therefore re-anchors the whole
    // resolution at `/` on the next open, and `prompt_file: "etc/passwd"` reads
    // the host file — no race needed, and it defeats the absolute-path ban too,
    // since "relative to /" IS absolute. `O_NOFOLLOW` refuses exactly that
    // swapped final component, and `O_DIRECTORY` refuses anything that is not a
    // directory.
    //
    // SCOPE, precisely: this covers the root's FINAL component only. INTERMEDIATE
    // components of `root` are resolved by the caller's own path lookup and
    // cannot be checked from in here, so the CALLER owes an intermediate the
    // model cannot rename. `resolve_conversation_workspace_dir` is what supplies
    // it — but note WHERE: it requires the CANONICALIZED root to be the
    // conversation workspace root or a DIRECT child of it. Its `dir`-STRING
    // check does NOT establish this and must not be relied on for it, because
    // `canonicalize` expands symlinks and a one-component string can resolve to
    // a nested root. The two rules are one mechanism: this refuses a swapped
    // final component, that makes the final component the only one the model
    // can swap.
    let dir = {
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC)
                .open(root)
        }
        #[cfg(not(unix))]
        {
            std::fs::File::open(root)
        }
    }
    .map_err(|e| PromptFileError::Unreadable(format!("bundle root: {e}")))?;
    let c_rel = CString::new(rel.as_bytes()).map_err(|_| PromptFileError::Unsafe)?;
    // `open_how` is `#[non_exhaustive]` (the kernel may grow it), so it is built
    // zeroed — which is also what openat2(2) requires of any field this build
    // does not know about — and then filled in.
    // SAFETY: `open_how` is a plain repr(C) struct of integers; all-zero is a
    // valid, and the documented-neutral, value for every field.
    let mut how: libc::open_how = unsafe { std::mem::zeroed() };
    // No `O_NOFOLLOW` here: it applies to the FINAL component, so an in-bundle
    // symlink to a prompt file would fail with ELOOP and be reported as a
    // security ESCAPE — a false verdict, and precisely the Linux/fallback
    // divergence this function exists to avoid. `RESOLVE_BENEATH` is the
    // confinement, and it covers symlink targets.
    how.flags = (libc::O_RDONLY | libc::O_NONBLOCK | libc::O_CLOEXEC) as u64;
    // RESOLVE_BENEATH is the confinement, and the kernel enforces it on every
    // component INCLUDING symlink targets — so a symlink that stays inside the
    // bundle still resolves, exactly as the non-Linux fallback allows, while one
    // that leaves it fails with EXDEV. RESOLVE_NO_SYMLINKS is deliberately NOT
    // set: it would additionally refuse in-bundle symlinks, which are legitimate
    // and which the fallback accepts, so Linux and non-Linux would disagree about
    // whether a bundle is valid. RESOLVE_NO_MAGICLINKS stays — /proc magic links
    // are not a bundle file by any reading.
    how.resolve = libc::RESOLVE_BENEATH | libc::RESOLVE_NO_MAGICLINKS;
    // SAFETY: `dir` is an open directory fd we own for the duration of the call,
    // `c_rel` is a NUL-terminated path, and `how` is a fully-initialised
    // `open_how` whose size we pass explicitly, per openat2(2).
    let fd = unsafe {
        libc::syscall(
            libc::SYS_openat2,
            std::os::fd::AsRawFd::as_raw_fd(&dir),
            c_rel.as_ptr(),
            &how as *const libc::open_how,
            std::mem::size_of::<libc::open_how>(),
        )
    };
    if fd < 0 {
        let err = std::io::Error::last_os_error();
        return Err(match err.raw_os_error() {
            // The kernel's own words for "that path left the root". Reported as
            // an ESCAPE, which is what it is. (ELOOP is included because a
            // magic-link rejection surfaces as one; an ordinary symlink loop is
            // also not a readable prompt.)
            Some(libc::EXDEV) | Some(libc::ELOOP) => PromptFileError::Escape,
            // openat2 landed in 5.6, and a seccomp filter that does not list it
            // (Docker's default profile predates it) answers EPERM. Both are
            // deployment facts, not author errors — refusing every `prompt_file:`
            // in the deployment with copy that blames the author's file would be
            // the worst possible answer, so fall back instead.
            Some(libc::ENOSYS) | Some(libc::EPERM) => {
                return open_confined_fallback(root, rel);
            }
            _ => PromptFileError::Unreadable(err.to_string()),
        });
    }
    // SAFETY: `fd` is a fresh, valid, owned descriptor returned by openat2.
    Ok(unsafe { std::fs::File::from_raw_fd(fd as std::os::fd::RawFd) })
}

#[cfg(not(target_os = "linux"))]
fn open_confined(root: &Path, rel: &str) -> Result<std::fs::File, PromptFileError> {
    open_confined_fallback(root, rel)
}

/// Non-Linux (and pre-5.6-kernel) resolution: canonicalize, confine, then open
/// refusing a final-component symlink. Documented as weaker than `openat2` —
/// an intermediate directory swapped between the check and the open is not
/// caught here.
fn open_confined_fallback(root: &Path, rel: &str) -> Result<std::fs::File, PromptFileError> {
    // The ANCHOR first, exactly as the Linux path does. Without this the whole
    // check is circular: if the root itself was replaced by a symlink, `canon`
    // and `root_canon` both resolve under the attacker's target and
    // `starts_with` passes. `symlink_metadata` does NOT follow the last
    // component, so it sees the symlink rather than what it points at.
    let root_meta = std::fs::symlink_metadata(root)
        .map_err(|e| PromptFileError::Unreadable(format!("bundle root: {e}")))?;
    if !root_meta.is_dir() {
        return Err(PromptFileError::Unreadable(
            "bundle root is not a directory".to_string(),
        ));
    }
    let canon = root
        .join(rel)
        .canonicalize()
        .map_err(|e| PromptFileError::Unreadable(e.to_string()))?;
    let root_canon = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    if !canon.starts_with(&root_canon) {
        return Err(PromptFileError::Escape);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&canon)
            .map_err(|e| PromptFileError::Unreadable(e.to_string()))
    }
    #[cfg(not(unix))]
    {
        std::fs::File::open(&canon).map_err(|e| PromptFileError::Unreadable(e.to_string()))
    }
}

impl PromptFileError {
    /// Author-facing detail, given the path as written.
    ///
    /// The CODE and LAYER deliberately do NOT live here: `prompt_file_finding`
    /// assigns them with string LITERALS, because the crate's code-drift guard
    /// reads those call sites textually. Providing them here as well would be a
    /// second copy of the same mapping — the very shape this module is being
    /// fixed to remove — so there is exactly one.
    pub fn message(&self, rel: &str) -> String {
        match self {
            Self::Unsafe => format!(
                "prompt_file '{rel}' must be a bundle-relative path: no '..', no leading '/', \
                 no drive letter, and no backslash (a separator on Windows and a legal \
                 filename character on Unix, so it cannot mean the same file on both)"
            ),
            Self::Escape => format!("prompt_file '{rel}' resolves outside bundle"),
            Self::Unreadable(why) => {
                format!("prompt_file '{rel}' cannot be read from the bundle: {why}")
            }
            Self::Empty => format!("prompt_file '{rel}' is empty"),
            Self::TooLarge(n) => format!(
                "prompt_file '{rel}' is {n} bytes, over the {MAX_PROMPT_FILE_BYTES}-byte limit"
            ),
        }
    }
}

/// The path-SHAPE half of [`read_prompt_file`] — the only part decidable
/// WITHOUT a materialized bundle, which is why it is separable (the draft
/// validation surfaces have no bundle; see [`check_prompt_files`]).
pub fn check_prompt_file_shape(rel: &str) -> Result<(), PromptFileError> {
    // `..` anywhere, and every ABSOLUTE form. The absolute test is deliberately
    // not `Path::is_absolute()`: that is platform-dependent, and a bundle
    // authored on one OS is validated and run on another, so a Windows-absolute
    // path must be refused on Linux too (and vice versa). Backslash is rejected
    // outright — it is a separator on Windows and a legal filename character on
    // Unix, so allowing it would mean the same string names different files on
    // the two platforms.
    let windows_absolute = rel
        .as_bytes()
        .get(1)
        .is_some_and(|b| *b == b':' && rel.as_bytes()[0].is_ascii_alphabetic());
    if rel.contains("..")
        || rel.starts_with('/')
        || rel.contains('\\')
        || windows_absolute
    {
        return Err(PromptFileError::Unsafe);
    }
    Ok(())
}

/// The SINGLE source of truth for "can this `prompt_file:` be used, and what
/// does it say" — shape check, confinement, read, emptiness, in one place.
///
/// Both the validator (`check_prompt_files`, which turns an `Err` into a
/// finding) and the runner (`dispatch.rs::load_raw_prompt`, which uses the `Ok`
/// string) go through here. That is what makes the file half of INV-1 hold in
/// BOTH directions:
///
/// * the runner cannot succeed where the validator would refuse — previously
///   `load_raw_prompt` did a bare `bundle_root.join(rel)` with no shape or
///   confinement check at all, so `prompt_file: "../../etc/passwd"` was
///   `WORKFLOW_PROMPT_FILE_UNSAFE` to the validator and `Ok(<file contents>)` to
///   the runner;
/// * and the validator cannot pass something the runner then fails on — an
///   existence-only check said yes to a directory, to a non-UTF-8 file and to a
///   zero-byte file, each of which failed (or degenerated) at run.
///
/// Reading the file IS the check: it is the same operation the runner performs,
/// so no weaker proxy for it can drift from it.
///
/// The runner calls this ITSELF rather than trusting that validation ran — the
/// two statements are not in tension: `spawn_run`/`resume_run` DO re-validate
/// immediately before dispatch, but `POST /workflows/{id}/test` does not, so the
/// runner cannot treat a prior validation as a precondition.
///
/// Confinement is enforced INSIDE this call by [`open_confined`], not by an
/// earlier check a caller is trusted to have made, and every subsequent check
/// interrogates the resulting file DESCRIPTOR rather than the path. The
/// validate→run gap remains (a different file may legitimately be there by run
/// time) but it is not a confinement hole: the run re-resolves under the same
/// kernel constraints and refuses the same things.
pub fn read_prompt_file(bundle_root: &Path, rel: &str) -> Result<String, PromptFileError> {
    check_prompt_file_shape(rel)?;
    // ONE resolution, confined by the kernel where it can be. Everything after
    // this point interrogates the FILE DESCRIPTOR, never the path again — so no
    // check can be invalidated by a rename racing between two path lookups.
    let mut file = open_confined(bundle_root, rel)?;
    let meta = file
        .metadata()
        .map_err(|e| PromptFileError::Unreadable(e.to_string()))?;
    // `fstat` on the OPENED fd. The open used `O_NONBLOCK`, so a FIFO returned
    // immediately instead of parking the thread until a writer appears — and it
    // is rejected here by TYPE, on the very file that was opened, rather than by
    // a stat of a path that may have changed since. Directories, sockets and
    // devices go the same way. This matters because for the workspace surfaces
    // the bundle root is bind-mounted read-write into the sandbox.
    if !meta.is_file() {
        return Err(PromptFileError::Unreadable(
            "not a regular file".to_string(),
        ));
    }
    if meta.len() > MAX_PROMPT_FILE_BYTES {
        return Err(PromptFileError::TooLarge(meta.len()));
    }
    // Bounded even if the file grew between the fstat and the read. The
    // capacity hint is CLAMPED: `meta.len()` comes from the file, so trusting it
    // for an allocation would let a (claimed) huge size reserve that much memory
    // before a single byte is read — the size REJECT above and this clamp guard
    // different things, which is why both exist.
    let mut buf = Vec::with_capacity(meta.len().min(MAX_PROMPT_FILE_BYTES) as usize);
    std::io::Read::read_to_end(
        &mut std::io::Read::take(&mut file, MAX_PROMPT_FILE_BYTES + 1),
        &mut buf,
    )
        .map_err(|e| PromptFileError::Unreadable(e.to_string()))?;
    if buf.len() as u64 > MAX_PROMPT_FILE_BYTES {
        return Err(PromptFileError::TooLarge(buf.len() as u64));
    }
    let body = String::from_utf8(buf)
        .map_err(|e| PromptFileError::Unreadable(format!("not valid UTF-8: {e}")))?;
    if body.is_empty() {
        // Symmetry with `prompt: ""`: an empty prompt is not a prompt, whichever
        // field it arrives in. Without this, the file half quietly shipped the
        // degenerate empty LLM call the inline half refuses.
        return Err(PromptFileError::Empty);
    }
    Ok(body)
}

/// Turn a [`PromptFileError`] into its author-facing finding.
///
/// Deliberately a `match` with LITERAL layer/code arguments at each
/// `ValidationError::at` call, rather than `e.layer()`/`e.code()`. The
/// crate-wide drift guard (`humanisation_contract`) parses these call sites
/// textually to prove every emitted code is registered AND has author-facing
/// copy in `validationCopy.ts`; a computed argument is invisible to it, so
/// passing `e.code()` here silently removed these three codes from BOTH halves
/// of that guard. The `message()` argument is free-form and not scanned.
fn prompt_file_finding(e: &PromptFileError, rel: &str, step_id: &str) -> ValidationError {
    match e {
        PromptFileError::Unsafe => ValidationError::at(
            "security",
            "WORKFLOW_PROMPT_FILE_UNSAFE",
            e.message(rel),
            step_id,
        ),
        PromptFileError::Escape => ValidationError::at(
            "security",
            "WORKFLOW_PROMPT_FILE_ESCAPE",
            e.message(rel),
            step_id,
        ),
        PromptFileError::Unreadable(_)
        | PromptFileError::Empty
        | PromptFileError::TooLarge(_) => ValidationError::at(
            "semantic",
            "WORKFLOW_PROMPT_FILE_MISSING",
            e.message(rel),
            step_id,
        ),
    }
}

/// `prompt_file:` checks.
///
/// Two DIFFERENT kinds of check live here, and only one of them needs a bundle:
///
/// * the path-SHAPE reject (`..` / absolute) is purely textual — decidable
///   anywhere, and it is a security check, so it always runs;
/// * the EXISTENCE / confinement pair (`WORKFLOW_PROMPT_FILE_MISSING`,
///   `WORKFLOW_PROMPT_FILE_ESCAPE`) can only be decided against a real,
///   materialized bundle.
///
/// The draft-validation surfaces (`POST /validate` on YAML text and
/// `POST /validate-def` on a posted `WorkflowDef`, both in `handlers/dev.rs`)
/// have NO bundle: they deliberately pass a unique path that was never created,
/// so a `WorkflowsRead` caller cannot probe real filesystem contents through
/// them. Statting a `prompt_file` under such a root can only ever fail — which
/// used to report `WORKFLOW_PROMPT_FILE_MISSING` for EVERY `prompt_file:` step,
/// a verdict the endpoint had no way to reach. In the builder that false finding
/// became a confident human sentence, a red step marker and a permanently
/// disabled Save on any imported `prompt_file:` workflow, with no form field
/// able to clear it.
///
/// So when the bundle root does not exist, the existence half is SKIPPED rather
/// than answered wrongly: it is a draft check, and the install/import path
/// (which passes the real extracted bundle dir) re-validates authoritatively
/// before anything is written. No finding is invented for a question this call
/// cannot answer.
fn check_prompt_files(workflow: &WorkflowDef, bundle_root: &Path) -> Vec<ValidationError> {
    let mut out = Vec::new();
    let bundle_present = bundle_root.is_dir();
    // One READ per distinct path, not per step. Nothing stops a definition
    // pointing fifty steps at the same 1 MiB file, and this function runs on
    // every install AND every launch — without this, ~1 KB of authored YAML buys
    // fifty megabytes of repeated disk reads each time. The verdict is unchanged
    // either way, so caching it is purely removing the amplification.
    let mut seen: std::collections::HashMap<&str, Option<PromptFileError>> =
        std::collections::HashMap::new();
    for s in &workflow.steps {
        // Filtered through the SAME emptiness rule the XOR check and the runner
        // use: an empty `prompt_file:` is ABSENT, not a path. Left unfiltered it
        // resolved to `bundle_root.join("")` — i.e. the bundle directory, which
        // always exists — so the step validated GREEN and then failed the run
        // with "Is a directory". Absent here, it is reported by the XOR check as
        // `WORKFLOW_PROMPT_MISSING`, which is what it actually is.
        let Some(p) = s.config.prompt_fields().and_then(|(_, f)| prompt_file_ref(f)) else {
            continue;
        };
        // Shape is decidable with no bundle, and it is a SECURITY check, so it
        // always runs.
        if let Err(e) = check_prompt_file_shape(p) {
            out.push(prompt_file_finding(&e, p, &s.id));
            continue;
        }
        if !bundle_present {
            // No bundle to resolve against — see this fn's doc comment.
            continue;
        }
        // Everything else is decided by the SAME call the runner makes, so the
        // validator cannot pass a file the run then fails on, nor refuse one the
        // run would have read (INV-1, file half).
        let verdict = match seen.get(p) {
            Some(cached) => cached.clone(),
            None => {
                let v = read_prompt_file(bundle_root, p).err();
                seen.insert(p, v.clone());
                v
            }
        };
        if let Some(e) = verdict {
            out.push(prompt_file_finding(&e, p, &s.id));
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

/// Reject U+0000 anywhere in the definition's author text.
///
/// Postgres cannot store a NUL in `text` and cannot convert the ` `
/// escape inside `jsonb` (`22P05`), and `workflows.compiled_ir_json` carries
/// the author's input names + `default` VALUES, output names + `from`
/// expressions and step ids/descriptions — so a NUL in any of those reached
/// the INSERT and `AppError::database_error` flattened it into a generic 500.
///
/// The walk is over the SERIALIZED def rather than a hand-written field list:
/// the set of author-supplied strings grows with every new step kind, and a
/// per-field list silently misses the next one added. Location is the JSON
/// path of the offending value so the builder can point at the field.
fn check_no_nul(workflow: &WorkflowDef) -> Vec<ValidationError> {
    fn walk(value: &serde_json::Value, path: &str, out: &mut Vec<ValidationError>) {
        match value {
            serde_json::Value::String(s) => {
                if s.contains('\0') {
                    out.push(ValidationError::at(
                        "schema",
                        "WORKFLOW_NUL_CHARACTER",
                        format!("{path} contains a NUL character, which cannot be stored"),
                        path.to_string(),
                    ));
                }
            }
            serde_json::Value::Array(items) => {
                for (i, item) in items.iter().enumerate() {
                    walk(item, &format!("{path}[{i}]"), out);
                }
            }
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    // An object KEY can carry a NUL too (an input `default:`
                    // is free-form JSON, and its keys land in the IR's
                    // inferred object type).
                    if k.contains('\0') {
                        out.push(ValidationError::at(
                            "schema",
                            "WORKFLOW_NUL_CHARACTER",
                            format!("a field name under {path} contains a NUL character"),
                            path.to_string(),
                        ));
                    }
                    walk(v, &format!("{path}.{k}"), out);
                }
            }
            _ => {}
        }
    }

    // A def that cannot be serialized cannot reach the DB either, so there is
    // nothing to guard; the install path surfaces that failure on its own.
    let Ok(value) = serde_json::to_value(workflow) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    walk(&value, "workflow", &mut out);
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
    fn tool_step_parses_with_kind_tool() {
        let yaml = r#"
steps:
  - id: search
    kind: tool
    server: web_search
    tool: web_search
    arguments:
      query: "{{ inputs.topic }}"
inputs:
  - name: topic
    required: true
"#;
        let wf = parse_workflow_yaml(yaml).expect("parse");
        assert_eq!(wf.steps[0].config.kind_str(), "tool");
    }

    #[test]
    fn tool_step_empty_server_rejected() {
        let yaml = r#"
steps:
  - id: search
    kind: tool
    server: "  "
    tool: web_search
"#;
        let wf = parse_workflow_yaml(yaml).expect("parse");
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(
            errs.iter().any(|e| e.code == "WORKFLOW_TOOL_NO_SERVER"),
            "expected WORKFLOW_TOOL_NO_SERVER, got: {errs:?}"
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

    /// Change A: `timeout_ms: 0` is the explicit "no timeout / wait indefinitely"
    /// sentinel (a durable gate) and must validate — NOT be treated as exceeding
    /// any bound.
    #[test]
    fn elicit_timeout_zero_validates() {
        let yaml = r#"
steps:
  - id: confirm
    kind: elicit
    message: "proceed?"
    schema:
      type: object
      properties:
        ok: { type: boolean }
      required: [ok]
    timeout_ms: 0
"#;
        let wf = parse_workflow_yaml(yaml).expect("parse");
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(
            !errs.iter().any(|e| e.code == "WORKFLOW_ELICIT_TIMEOUT_CAP"),
            "timeout_ms: 0 (unbounded) must NOT trip the cap check, got: {errs:?}"
        );
    }

    #[test]
    fn elicit_timeout_over_cap_rejected() {
        let yaml = format!(
            r#"
steps:
  - id: confirm
    kind: elicit
    message: "proceed?"
    schema:
      type: object
      properties:
        ok: {{ type: boolean }}
      required: [ok]
    timeout_ms: {}
"#,
            ELICIT_TIMEOUT_HARD_CAP_MS + 1
        );
        let wf = parse_workflow_yaml(&yaml).expect("parse");
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(
            errs.iter().any(|e| e.code == "WORKFLOW_ELICIT_TIMEOUT_CAP"),
            "a non-zero timeout over the 30-min cap must be rejected, got: {errs:?}"
        );
    }

    #[test]
    fn elicit_timeout_at_cap_accepted() {
        let yaml = format!(
            r#"
steps:
  - id: confirm
    kind: elicit
    message: "proceed?"
    schema:
      type: object
      properties:
        ok: {{ type: boolean }}
      required: [ok]
    timeout_ms: {}
"#,
            ELICIT_TIMEOUT_HARD_CAP_MS
        );
        let wf = parse_workflow_yaml(&yaml).expect("parse");
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(
            !errs.iter().any(|e| e.code == "WORKFLOW_ELICIT_TIMEOUT_CAP"),
            "a timeout exactly at the cap must be accepted, got: {errs:?}"
        );
    }

    #[test]
    fn dead_tools_field_on_llm_rejected() {
        // E6: `tools:` on an llm step is dead — must be rejected with a clear code.
        let yaml = r#"
steps:
  - id: gen
    kind: llm
    prompt: "hi"
    tools: ["web_search"]
"#;
        let wf = parse_workflow_yaml(yaml).expect("parse");
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(
            errs.iter().any(|e| e.code == "WORKFLOW_DEAD_TOOLS_FIELD"),
            "expected WORKFLOW_DEAD_TOOLS_FIELD, got: {errs:?}"
        );
    }

    #[test]
    fn elicit_step_with_data_seed_parses() {
        // D2: the optional `data:` seed deserializes onto the elicit config.
        let yaml = r#"
steps:
  - id: review
    kind: elicit
    message: "Review"
    schema:
      type: object
    data: "{{ inputs.seed }}"
inputs:
  - name: seed
"#;
        let wf = parse_workflow_yaml(yaml).expect("parse");
        match &wf.steps[0].config {
            StepConfig::Elicit { data, .. } => {
                assert!(data.is_some(), "data seed should parse onto the elicit config");
            }
            other => panic!("expected elicit config, got {other:?}"),
        }
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
    fn accepts_step_description_with_input_ref() {
        let yaml = r#"
inputs:
  - name: topic
    required: true
steps:
  - id: g
    kind: llm
    prompt: "x"
    description: "Summarize {{ inputs.topic }}"
"#;
        let wf = parse_workflow_yaml(yaml).unwrap();
        // The field is captured (not silently dropped) + valid.
        assert_eq!(
            wf.steps[0].description.as_deref(),
            Some("Summarize {{ inputs.topic }}")
        );
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(!errs
            .iter()
            .any(|e| e.code == "WORKFLOW_STEP_DESCRIPTION_TOO_LONG"));
        assert!(!errs.iter().any(|e| e.code == "WORKFLOW_UNKNOWN_INPUT_REF"));
    }

    #[test]
    fn rejects_overlong_step_description() {
        let long = "x".repeat(MAX_STEP_DESCRIPTION_CHARS + 1);
        let yaml = format!(
            "steps:\n  - id: g\n    kind: llm\n    prompt: \"x\"\n    description: \"{long}\"\n"
        );
        let wf = parse_workflow_yaml(&yaml).unwrap();
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(errs
            .iter()
            .any(|e| e.code == "WORKFLOW_STEP_DESCRIPTION_TOO_LONG"));
    }

    #[test]
    fn rejects_bad_ref_in_step_description() {
        let yaml = r#"
steps:
  - id: g
    kind: llm
    prompt: "x"
    description: "uses {{ nope.output }}"
"#;
        let wf = parse_workflow_yaml(yaml).unwrap();
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(errs.iter().any(|e| e.code == "WORKFLOW_UNKNOWN_STEP_REF"));
    }

    #[test]
    fn llm_map_item_var_is_valid_in_its_prompt() {
        // Regression: the per-item binding (`item_var`) must be a valid head
        // inside the llm_map step's own prompt — `{{ aspect }}` and
        // `{{ aspect.field }}`. Without this, every real seed llm_map
        // workflow (deep-research, code-review, …) fails to install.
        let yaml = r#"
inputs:
  - name: topic
    required: true
steps:
  - id: list
    kind: llm
    prompt: "list aspects of {{ inputs.topic }}"
    output_format: json
  - id: each
    kind: llm_map
    for_each: "{{ list.output }}"
    item_var: aspect
    prompt: "describe {{ aspect }} (field {{ aspect.name }}) of {{ inputs.topic }}"
    depends_on: [list]
outputs:
  - name: o
    from: "{{ each.output }}"
"#;
        let wf = parse_workflow_yaml(yaml).unwrap();
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(
            !errs.iter().any(|e| e.code == "WORKFLOW_UNKNOWN_STEP_REF"),
            "item_var must not be flagged as an unknown step ref: {errs:?}"
        );
    }

    #[test]
    fn item_var_out_of_scope_in_for_each_and_other_steps() {
        // The item_var is in scope ONLY in its own prompt — referencing it in
        // for_each or in another step must still error.
        let yaml = r#"
steps:
  - id: each
    kind: llm_map
    for_each: "{{ aspect.output }}"
    item_var: aspect
    prompt: "x {{ aspect }}"
"#;
        let wf = parse_workflow_yaml(yaml).unwrap();
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(
            errs.iter().any(|e| e.code == "WORKFLOW_UNKNOWN_STEP_REF"),
            "item_var in for_each must error: {errs:?}"
        );
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

    /// **TEST-2** — the shared rule's normalisation table, asserted directly.
    ///
    /// This is the single point both the validator and
    /// `dispatch.rs::load_raw_prompt` consult, so its boundaries ARE the
    /// contract; DEC-3's deliberate non-trimming is pinned here so a later
    /// "tidy-up" to `trim()` shows up as a red test rather than a silent
    /// behaviour change.
    #[test]
    fn prompt_source_treats_empty_as_absent_but_not_whitespace() {
        let some = |s: &str| Some(s.to_string());
        let none: Option<String> = None;

        // Empty is ABSENT — on either field.
        assert_eq!(
            prompt_source(&some(""), &some("p.md")),
            PromptSource::File("p.md"),
            "a cleared prompt box beside a prompt_file: must resolve to the FILE"
        );
        assert_eq!(
            prompt_source(&some(""), &none),
            PromptSource::Missing,
            "a cleared prompt box with no prompt_file: is a step with no wording"
        );
        assert_eq!(
            prompt_source(&none, &some("")),
            PromptSource::Missing,
            "an empty prompt_file: names the bundle dir, not a prompt"
        );
        assert_eq!(prompt_source(&some(""), &some("")), PromptSource::Missing);
        // A VERDICT THAT MOVED, deliberately: `prompt: "x"` + `prompt_file: ""`
        // used to be `WORKFLOW_PROMPT_BOTH` (install-blocking) because the old
        // rule read `has_file = prompt_file.is_some()`. An empty path is not a
        // second prompt source, so this is now simply an inline prompt — and the
        // run agrees, which is the point (DEC-5).
        assert_eq!(prompt_source(&some("x"), &some("")), PromptSource::Inline("x"));

        // Whitespace is NOT trimmed (DEC-3) — it stays a real prompt, and a
        // whitespace-only PATH stays a path (it is then reported
        // WORKFLOW_PROMPT_FILE_MISSING, not WORKFLOW_PROMPT_MISSING). Both
        // directions are pinned: a `trim()` creeping into either half of
        // `prompt_source` changes which finding the author sees.
        assert_eq!(prompt_source(&some("   "), &none), PromptSource::Inline("   "));
        assert_eq!(prompt_source(&some("   "), &some("p.md")), PromptSource::Both);
        assert_eq!(prompt_source(&none, &some("   ")), PromptSource::File("   "));
        assert_eq!(prompt_file_ref(&some("   ")), Some("   "));

        // The ordinary cases are unchanged.
        assert_eq!(prompt_source(&some("hi"), &none), PromptSource::Inline("hi"));
        assert_eq!(prompt_source(&none, &some("p.md")), PromptSource::File("p.md"));
        assert_eq!(prompt_source(&some("hi"), &some("p.md")), PromptSource::Both);
        assert_eq!(prompt_source(&none, &none), PromptSource::Missing);
    }

    /// **Rust ↔ TypeScript drift guard for the prompt-source rule.**
    ///
    /// `prompt_source` (here) and `promptSuppliedByFile`
    /// (`ui/.../builder/stepForms.ts`) implement the SAME emptiness rule in two
    /// languages, and nothing else connects them: a TS unit test can only assert
    /// its own side, so a change here would silently desync the builder's
    /// required-field marker from the backend's verdict — a client-side rerun of
    /// exactly the two-places-decide-separately defect this rule exists to
    /// prevent.
    ///
    /// Same shape as this file's existing `validationCopy.ts` guard: read the TS
    /// at RUNTIME (never `include_str!`, which would freeze a stale snapshot) and
    /// fail the BACKEND suite on drift.
    ///
    /// It is ONE-DIRECTIONAL by construction, and that is worth being explicit
    /// about: it catches the client drifting from the rule, not the rule drifting
    /// from the client. The Rust side is pinned separately by TEST-2, which
    /// asserts the same two boundaries on `prompt_source`/`prompt_file_ref`
    /// directly — so a change to either half is caught, but by different tests.
    /// Asserted here:
    ///
    /// * the predicate rejects the empty string, and
    /// * it does NOT trim — because `prompt_source` filters on `is_empty()`, a
    ///   client that trimmed would report "a prompt is required" where the
    ///   backend reports `WORKFLOW_PROMPT_FILE_MISSING`.
    #[test]
    fn client_prompt_file_predicate_mirrors_prompt_source() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../ui/src/modules/workflow/components/builder/stepForms.ts");
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let start = src
            .find("export function promptSuppliedByFile")
            .unwrap_or_else(|| {
                panic!(
                    "no `export function promptSuppliedByFile` in {} — the drift guard \
                     for the prompt-source rule has lost its subject; re-point it",
                    path.display()
                )
            });
        let body = &src[start..];
        let end = body.find("\n}").expect("unterminated promptSuppliedByFile");
        let body = &body[..end];

        assert!(
            body.contains("length > 0") || body.contains("!== ''") || body.contains("length !== 0"),
            "promptSuppliedByFile must treat an EMPTY prompt_file as ABSENT, matching \
             `prompt_source`'s `!s.is_empty()` filter — otherwise the builder lifts the \
             prompt requirement on a step this validator reports incomplete. Body was:\n{body}"
        );
        assert!(
            !body.contains(".trim()"),
            "promptSuppliedByFile must NOT trim: `prompt_source` filters on `is_empty()`, \
             so a whitespace-only prompt_file IS a file to this validator (reported \
             WORKFLOW_PROMPT_FILE_MISSING, not WORKFLOW_PROMPT_MISSING). Trimming on the \
             client makes the two surfaces disagree. Body was:\n{body}"
        );
    }

    /// **TEST-12** — the resource guards on `read_prompt_file`.
    ///
    /// These are the only security controls in this change and nothing pinned
    /// them: an audit round deleted the type check, both size checks and the
    /// bounded read at once and the whole suite stayed green.
    ///
    /// Honest limit, because it was measured: for a STATIC file the fstat size
    /// reject and the post-read size reject are mutually redundant — delete
    /// either alone and the other still refuses, so neither is INDIVIDUALLY
    /// falsifiable here. They guard different things (a lying/growing size vs the
    /// bytes actually delivered) and only a file that changes size mid-read would
    /// separate them, which is not constructible in a unit test. What this test
    /// does prove is the PROPERTY they exist for: nothing over the cap is ever
    /// returned, whichever guard fires.
    #[test]
    fn read_prompt_file_refuses_what_it_must_not_read() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();

        // A FIFO. `open(2)` on one blocks until a writer appears; the bundle root
        // is bind-mounted read-write into the code sandbox for the workspace
        // surfaces, so a model can create one. The open is O_NONBLOCK and the
        // type is judged from the resulting fd, so this returns rather than
        // parking the thread — a test that HANGS here is the regression.
        #[cfg(unix)]
        {
            let fifo = root.join("fifo.md");
            let c = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
            // SAFETY: a NUL-terminated path in a temp dir we own.
            let rc = unsafe { libc::mkfifo(c.as_ptr(), 0o644) };
            assert_eq!(rc, 0, "mkfifo failed: {}", std::io::Error::last_os_error());
            let err = read_prompt_file(root, "fifo.md")
                .expect_err("a FIFO must never be read as a prompt");
            assert!(
                matches!(err, PromptFileError::Unreadable(ref why) if why.contains("regular")),
                "expected a not-a-regular-file rejection, got {err:?}"
            );
        }

        // A directory, judged the same way.
        std::fs::create_dir_all(root.join("adir")).unwrap();
        assert!(
            matches!(read_prompt_file(root, "adir"), Err(PromptFileError::Unreadable(_))),
            "a directory is not a prompt"
        );

        // Over the cap. The validator reads every prompt file on every launch, so
        // an author-controlled size is author-controlled work and memory.
        let big = root.join("big.md");
        std::fs::write(&big, vec![b'x'; (MAX_PROMPT_FILE_BYTES + 1) as usize]).unwrap();
        match read_prompt_file(root, "big.md") {
            Err(PromptFileError::TooLarge(n)) => {
                assert!(n > MAX_PROMPT_FILE_BYTES, "reported size {n}")
            }
            other => panic!("an over-cap prompt_file must be refused, got {other:?}"),
        }
        // Exactly at the cap is fine — the boundary is inclusive.
        let atcap = root.join("atcap.md");
        std::fs::write(&atcap, vec![b'y'; MAX_PROMPT_FILE_BYTES as usize]).unwrap();
        assert_eq!(
            read_prompt_file(root, "atcap.md").map(|b| b.len()),
            Ok(MAX_PROMPT_FILE_BYTES as usize),
            "a prompt_file exactly at the cap must still be readable"
        );

        // And a normal file still reads.
        std::fs::write(root.join("ok.md"), "BODY").unwrap();
        assert_eq!(read_prompt_file(root, "ok.md").unwrap(), "BODY");
    }

    /// **TEST-14** — the ANCHOR itself must not be swappable.
    ///
    /// The kernel confinement is only as good as the directory it is anchored
    /// to. For the workspace surfaces the LAST component of the bundle root sits
    /// inside the directory bwrap bind-mounts read-write into the sandbox, so a
    /// sandbox step can run `mv flow flowbak && ln -s / flow` and, if the root is
    /// opened by a symlink-following path lookup, every subsequent resolution is
    /// anchored at `/` — `prompt_file: "etc/passwd"` then reads the host file
    /// while still satisfying the "no absolute paths" rule, because relative-to-`/`
    /// is not spelled absolutely. No race is required.
    ///
    /// `#[cfg(unix)]` only because the test needs `symlink` to build the attack.
    /// BOTH resolution paths are asserted, and deliberately not by relying on
    /// which one this platform takes: `read_prompt_file` exercises whichever is
    /// live (on Linux, `openat2`), and `open_confined_fallback` is then CALLED
    /// BY NAME below — on Linux that is the only way its anchor guard runs at
    /// all, since `read_prompt_file` never reaches it there. Each leg has a
    /// positive control on the same tree, so a guard that refuses everything
    /// cannot pass either. Mutating `symlink_metadata` to `metadata` in the
    /// fallback, or dropping `O_NOFOLLOW` from the Linux anchor open, each turns
    /// this test red on Linux.
    #[cfg(unix)]
    #[test]
    fn read_prompt_file_refuses_a_bundle_root_that_became_a_symlink() {
        let tmp = tempdir().unwrap();
        let real_root = tmp.path().join("flow");
        std::fs::create_dir_all(&real_root).unwrap();
        std::fs::write(real_root.join("task.md"), "REAL PROMPT").unwrap();
        // Sanity, both legs: an ordinary root reads through the live path AND
        // through the fallback.
        assert_eq!(read_prompt_file(&real_root, "task.md").unwrap(), "REAL PROMPT");
        {
            use std::io::Read;
            let mut buf = String::new();
            open_confined_fallback(&real_root, "task.md")
                .expect("the fallback must read an ordinary root")
                .read_to_string(&mut buf)
                .unwrap();
            assert_eq!(buf, "REAL PROMPT");
        }

        // Now the sandbox swaps the root for a symlink to somewhere else. The
        // target is a real directory holding a real file, so nothing downstream
        // can notice by type or content — only the anchor open can refuse it.
        let elsewhere = tmp.path().join("elsewhere");
        std::fs::create_dir_all(elsewhere.join("etc")).unwrap();
        std::fs::write(elsewhere.join("etc/passwd"), "HOST SECRET").unwrap();
        std::fs::rename(&real_root, tmp.path().join("flowbak")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &real_root).unwrap();

        let err = read_prompt_file(&real_root, "etc/passwd")
            .expect_err("a bundle root that is now a symlink must not be used as the anchor");
        assert!(
            matches!(err, PromptFileError::Unreadable(_)),
            "expected the anchor open to refuse the swapped root, got {err:?}"
        );
        assert!(
            !err.message("etc/passwd").contains("HOST SECRET"),
            "the swapped root's contents must never be reached"
        );

        // The fallback's OWN anchor guard, driven directly. On Linux the call
        // above took the `openat2` path, so without this the fallback — the
        // entire non-Linux and pre-5.6 story — is never executed by any test.
        // A successful open here IS the escape: the returned fd would be
        // `<elsewhere>/etc/passwd`.
        match open_confined_fallback(&real_root, "etc/passwd") {
            Err(PromptFileError::Unreadable(_)) => {}
            Ok(mut f) => {
                use std::io::Read;
                let mut buf = String::new();
                let _ = f.read_to_string(&mut buf);
                panic!("the fallback opened a bundle root that is now a symlink; it read {buf:?}");
            }
            Err(other) => panic!(
                "expected the fallback's anchor open to refuse the swapped root, got {other:?}"
            ),
        }
    }

    /// **TEST-13** — the path-SHAPE rejects, including the two that are not
    /// Unix-shaped. A bundle authored on one OS is validated and run on another,
    /// so a Windows-absolute path has to be refused on Linux too; nothing pinned
    /// the backslash and drive-letter clauses.
    #[test]
    fn prompt_file_shape_refuses_every_absolute_and_traversing_form() {
        for bad in [
            "../secrets.md",
            "prompts/../../secrets.md",
            "/etc/passwd",
            "C:\\Windows\\win.ini",
            "c:/Windows/win.ini",
            "prompts\\task.md",
        ] {
            assert_eq!(
                check_prompt_file_shape(bad),
                Err(PromptFileError::Unsafe),
                "{bad:?} must be refused by shape alone, on every platform"
            );
        }
        for ok in ["prompts/task.md", "task.md", "a/b/c.md", "weird name.md"] {
            assert_eq!(
                check_prompt_file_shape(ok),
                Ok(()),
                "{ok:?} is a legitimate bundle-relative path"
            );
        }
    }

    /// **TEST-3** — the validator's verdicts on the states that used to validate
    /// GREEN and then fail the run, plus a guard that the pre-existing verdicts
    /// did not move.
    #[test]
    fn validator_verdicts_on_the_empty_and_directory_prompt_states() {
        let tmp = tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("prompts")).unwrap();
        std::fs::write(tmp.path().join("prompts/real.md"), "body").unwrap();
        std::fs::create_dir_all(tmp.path().join("prompts/adir")).unwrap();
        std::fs::write(tmp.path().join("prompts/empty.md"), "").unwrap();
        std::fs::write(tmp.path().join("prompts/binary.bin"), [0xff_u8, 0xfe, 0x00]).unwrap();

        let codes = |yaml: &str| -> Vec<&'static str> {
            let wf = parse_workflow_yaml(yaml).unwrap();
            validate_collecting(&wf, tmp.path(), false)
                .into_iter()
                .map(|e| e.code)
                .collect()
        };

        // A cleared prompt box beside a real prompt_file: is a COMPLETE step.
        let c = codes(
            "steps:\n  - id: g\n    kind: llm\n    prompt: \"\"\n    prompt_file: \"prompts/real.md\"\n",
        );
        assert!(
            !c.contains(&"WORKFLOW_PROMPT_BOTH") && !c.contains(&"WORKFLOW_PROMPT_MISSING"),
            "an empty prompt beside a prompt_file: is neither 'both' nor 'missing': {c:?}"
        );
        assert!(!c.contains(&"WORKFLOW_PROMPT_FILE_MISSING"), "{c:?}");

        // An empty prompt_file: is no prompt source at all.
        let c = codes("steps:\n  - id: g\n    kind: llm\n    prompt_file: \"\"\n");
        assert!(
            c.contains(&"WORKFLOW_PROMPT_MISSING"),
            "an empty prompt_file: must be reported as a missing prompt, not accepted: {c:?}"
        );

        // A prompt_file: naming a DIRECTORY cannot be read at run time.
        let c = codes("steps:\n  - id: g\n    kind: llm\n    prompt_file: \"prompts/adir\"\n");
        assert!(
            c.contains(&"WORKFLOW_PROMPT_FILE_MISSING"),
            "a prompt_file: naming a directory must be rejected: {c:?}"
        );

        // A prompt_file: that exists and is readable but holds NOTHING is not a
        // prompt — symmetric with `prompt: ""`, and it stops the file half
        // shipping the degenerate empty LLM call the inline half refuses.
        let c = codes("steps:\n  - id: g\n    kind: llm\n    prompt_file: \"prompts/empty.md\"\n");
        assert!(
            c.contains(&"WORKFLOW_PROMPT_FILE_MISSING"),
            "a zero-byte prompt_file must be rejected: {c:?}"
        );

        // A prompt_file: that is a real file but not TEXT passes an
        // existence/is-file check and then fails the run on `read_to_string`.
        let c = codes("steps:\n  - id: g\n    kind: llm\n    prompt_file: \"prompts/binary.bin\"\n");
        assert!(
            c.contains(&"WORKFLOW_PROMPT_FILE_MISSING"),
            "a non-UTF-8 prompt_file must be rejected at validate, not at run: {c:?}"
        );

        // Over the cap reaches the VALIDATOR's verdict too, not only
        // `read_prompt_file`'s error — the mapping to a finding is its own step.
        let big = tmp.path().join("prompts/big.md");
        std::fs::write(&big, vec![b'x'; (MAX_PROMPT_FILE_BYTES + 1) as usize]).unwrap();
        let c = codes("steps:\n  - id: g\n    kind: llm\n    prompt_file: \"prompts/big.md\"\n");
        assert!(
            c.contains(&"WORKFLOW_PROMPT_FILE_MISSING"),
            "an over-cap prompt_file must reach an author-facing verdict: {c:?}"
        );

        // A VERDICT THAT MOVED, deliberately (DEC-5): `prompt:` + an EMPTY
        // `prompt_file:` used to be WORKFLOW_PROMPT_BOTH because the old rule
        // read `has_file = prompt_file.is_some()`. An empty path is not a second
        // prompt source, and the run agrees — so it is now simply valid.
        let c = codes(
            "steps:\n  - id: g\n    kind: llm\n    prompt: \"inline\"\n    prompt_file: \"\"\n",
        );
        assert!(
            !c.contains(&"WORKFLOW_PROMPT_BOTH") && !c.contains(&"WORKFLOW_PROMPT_MISSING"),
            "an inline prompt beside an EMPTY prompt_file is just an inline prompt: {c:?}"
        );

        // Pre-existing verdicts unmoved.
        let c = codes(
            "steps:\n  - id: g\n    kind: llm\n    prompt: \"inline\"\n    prompt_file: \"prompts/real.md\"\n",
        );
        assert!(c.contains(&"WORKFLOW_PROMPT_BOTH"), "{c:?}");
        let c = codes("steps:\n  - id: g\n    kind: llm\n");
        assert!(c.contains(&"WORKFLOW_PROMPT_MISSING"), "{c:?}");
        let c = codes("steps:\n  - id: g\n    kind: llm\n    prompt_file: \"prompts/nope.md\"\n");
        assert!(c.contains(&"WORKFLOW_PROMPT_FILE_MISSING"), "{c:?}");
        let c = codes("steps:\n  - id: g\n    kind: llm\n    prompt_file: \"../../etc/passwd\"\n");
        assert!(c.contains(&"WORKFLOW_PROMPT_FILE_UNSAFE"), "{c:?}");
    }

    #[test]
    fn prompt_file_missing_in_bundle_is_reported() {
        // S6: a SAFE, bundle-relative prompt_file that doesn't exist in the
        // bundle → WORKFLOW_PROMPT_FILE_MISSING (canonicalize fails). Distinct
        // from the cheap textual `..`/`/` reject (WORKFLOW_PROMPT_FILE_UNSAFE).
        let yaml = r#"
steps:
  - id: g
    kind: llm
    prompt_file: "prompts/nope.md"
"#;
        let wf = parse_workflow_yaml(yaml).unwrap();
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(
            errs.iter().any(|e| e.code == "WORKFLOW_PROMPT_FILE_MISSING"),
            "absent prompt_file must trip MISSING: {errs:?}"
        );
    }

    #[test]
    fn draft_validation_without_a_bundle_reports_no_prompt_file_verdict() {
        // FIX round 4 / finding 2. `handlers/dev.rs`'s two DRAFT surfaces
        // (`POST /validate` and `POST /validate-def`) pass a unique path that is
        // never created as the bundle root, so a `WorkflowsRead` caller cannot
        // probe the filesystem through them. Against such a root every
        // `prompt_file:` step used to come back `WORKFLOW_PROMPT_FILE_MISSING` —
        // a verdict the endpoint cannot actually reach — which the builder
        // amplified into a human sentence, a red step marker and a permanently
        // disabled Save that no form field could clear.
        //
        // The root below is built EXACTLY as `validate_workflow_def` builds it.
        let yaml = r#"
steps:
  - id: g
    kind: llm
    prompt_file: "prompts/step.md"
"#;
        let wf = parse_workflow_yaml(yaml).unwrap();
        let never_created =
            std::env::temp_dir().join(format!("ziee-wf-validate-{}", uuid::Uuid::new_v4()));
        assert!(
            !never_created.exists(),
            "the fixture root must not exist, or this test proves nothing"
        );

        let errs = validate_collecting(&wf, &never_created, true);

        assert!(
            !errs
                .iter()
                .any(|e| e.code == "WORKFLOW_PROMPT_FILE_MISSING"),
            "a draft check with no bundle must not claim the prompt file is missing: {errs:?}"
        );
        assert!(
            !errs.iter().any(|e| e.code == "WORKFLOW_PROMPT_FILE_ESCAPE"),
            "a draft check with no bundle cannot decide confinement either: {errs:?}"
        );
        // …and the step is not blocked at all: nothing else about it is wrong.
        assert!(
            !errs.iter().any(|e| e.severity == Severity::Error),
            "a valid draft step must leave Save reachable: {errs:?}"
        );
    }

    #[test]
    fn draft_validation_without_a_bundle_still_rejects_an_unsafe_prompt_path() {
        // The SHAPE reject is textual, so it is decidable with no bundle — and it
        // is the security half. Skipping the existence check must not take it.
        let yaml = r#"
steps:
  - id: g
    kind: llm
    prompt_file: "../../etc/passwd"
"#;
        let wf = parse_workflow_yaml(yaml).unwrap();
        let never_created =
            std::env::temp_dir().join(format!("ziee-wf-validate-{}", uuid::Uuid::new_v4()));
        let errs = validate_collecting(&wf, &never_created, true);
        assert!(
            errs.iter().any(|e| e.code == "WORKFLOW_PROMPT_FILE_UNSAFE"),
            "the textual path reject must survive the no-bundle skip: {errs:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn prompt_file_escaping_bundle_via_symlink_is_reported() {
        // S6: a relative, dotdot-free prompt_file that CANONICALIZES outside
        // the bundle (via a symlink) trips the post-canonicalize confinement
        // guard WORKFLOW_PROMPT_FILE_ESCAPE — distinct from the textual reject.
        use std::os::unix::fs::symlink;
        let outside = tempdir().unwrap();
        let target = outside.path().join("secret.md");
        std::fs::write(&target, b"secret").unwrap();

        let bundle = tempdir().unwrap();
        // bundle/escape.md -> <outside>/secret.md (resolves outside the bundle).
        symlink(&target, bundle.path().join("escape.md")).unwrap();

        let yaml = r#"
steps:
  - id: g
    kind: llm
    prompt_file: "escape.md"
"#;
        let wf = parse_workflow_yaml(yaml).unwrap();
        let errs = validate_collecting(&wf, bundle.path(), false);
        assert!(
            errs.iter().any(|e| e.code == "WORKFLOW_PROMPT_FILE_ESCAPE"),
            "a prompt_file symlink escaping the bundle must trip ESCAPE: {errs:?}"
        );
    }

    #[test]
    fn rejects_unsafe_artifact_path() {
        // S6: a step artifact decl whose path escapes the workspace →
        // WORKFLOW_ARTIFACT_PATH_UNSAFE.
        let yaml = r#"
steps:
  - id: g
    kind: llm
    prompt: "x"
    artifacts:
      - path: "../escape.txt"
"#;
        let wf = parse_workflow_yaml(yaml).unwrap();
        let tmp = tempdir().unwrap();
        let errs = validate_collecting(&wf, tmp.path(), false);
        assert!(
            errs.iter().any(|e| e.code == "WORKFLOW_ARTIFACT_PATH_UNSAFE"),
            "unsafe artifact path must trip ARTIFACT_PATH_UNSAFE: {errs:?}"
        );
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

    /// The vendored SR hub workflows must parse + pass install-time validation
    /// (serde shape + semantic + template ref-check). Reads the committed loose
    /// `workflow.yaml` source — the seed stores these as source; `build.rs`
    /// (`build_helper/hub_seed.rs`) packs them into the bundle tarball at build.
    #[test]
    fn sr_seed_workflows_parse_and_validate() {
        let yamls = [(
            "sr-review",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/hub-seed/workflows/io.github.ziee/sr-review/workflow.yaml"
            )),
        )];
        for (name, yaml) in yamls {
            let wf = parse_workflow_yaml(yaml)
                .unwrap_or_else(|e| panic!("{name}: parse failed: {e:?}"));
            validate_for_install(&wf, std::path::Path::new("/tmp/sr"), false)
                .unwrap_or_else(|e| panic!("{name}: validate failed: {e:?}"));
        }
    }

    /// The build-time packer (`build_helper/hub_seed.rs`) tars the committed
    /// source `workflow.yaml` into the bundle `.tar.gz` and rewrites the
    /// manifest's sha256/size into the BAKED seed (`binaries/hub-seed/`, the
    /// `include_dir!` source). Assert that baked pair is self-consistent and
    /// packs the committed source verbatim — a regression net for the packer
    /// that makes bundle↔manifest drift impossible by construction. (The baked
    /// dir is populated by `build.rs` before this crate compiles, same as the
    /// `include_dir!` in `hub_manager.rs`.)
    #[test]
    fn sr_seed_bundles_are_internally_consistent() {
        use sha2::{Digest, Sha256};
        fn check(name: &str, tar_gz: &[u8], manifest: &str, source_yaml: &str) {
            let m: serde_json::Value =
                serde_json::from_str(manifest).unwrap_or_else(|e| panic!("{name}: manifest: {e}"));
            let want_sha = m["bundle"]["sha256"].as_str().expect("manifest sha256");
            let want_size = m["bundle"]["size_bytes"].as_u64().expect("manifest size_bytes");
            let got_sha: String = {
                let mut h = Sha256::new();
                h.update(tar_gz);
                h.finalize().iter().map(|b| format!("{b:02x}")).collect()
            };
            assert_eq!(got_sha, want_sha, "{name}: baked tar.gz sha256 != baked manifest");
            assert_eq!(
                tar_gz.len() as u64,
                want_size,
                "{name}: baked tar.gz size != baked manifest"
            );

            let gz = flate2::read::GzDecoder::new(tar_gz);
            let mut ar = tar::Archive::new(gz);
            let mut packed: Option<String> = None;
            for entry in ar.entries().expect("tar entries") {
                let mut e = entry.expect("tar entry");
                let path = e.path().expect("entry path").to_string_lossy().into_owned();
                if path == "workflow.yaml" {
                    let mut s = String::new();
                    std::io::Read::read_to_string(&mut e, &mut s).expect("read packed yaml");
                    packed = Some(s);
                }
            }
            let packed =
                packed.unwrap_or_else(|| panic!("{name}: no workflow.yaml in baked tarball"));
            assert_eq!(packed, source_yaml, "{name}: packed workflow.yaml != committed source");
        }
        macro_rules! sr {
            ($n:literal) => {
                check(
                    $n,
                    include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/binaries/hub-seed/workflows/io.github.ziee/",
                        $n,
                        "/1.0.0.tar.gz"
                    )),
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/binaries/hub-seed/workflows/io.github.ziee/",
                        $n,
                        "/1.0.0.json"
                    )),
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/resources/hub-seed/workflows/io.github.ziee/",
                        $n,
                        "/workflow.yaml"
                    )),
                )
            };
        }
        sr!("sr-review");
    }
}

// ============================================================================
// ITEM-2 / TEST-1 / TEST-15 — the humanisation drift guard.
//
// These live OUTSIDE the big `mod tests` above so they read as a distinct
// contract: they do not test validation behaviour, they keep the validator's
// machine-readable codes in lockstep with the AUTHOR-FACING copy the workflow
// builder shows. The mechanism mirrors `openapi::emit_ts::tests::types_ts_parity`
// — a backend test that reads a committed frontend file and fails when the two
// drift apart.
// ============================================================================
#[cfg(test)]
mod humanisation_contract {
    use super::VALIDATION_CODES;
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    /// The layers a finding may carry (`ValidationError::layer`). An emit site
    /// using anything else FAILS the guard rather than being skipped: a silent
    /// skip is exactly how a new layer's codes would escape BOTH halves of the
    /// contract (registry + human copy) and reach the author as raw wire text.
    const KNOWN_LAYERS: &[&str] = &["schema", "semantic", "security"];

    /// The `ValidationError` fields that decide what this guard vouches for.
    ///
    /// The struct's fields are `pub` (every consumer — `handlers/dev.rs`,
    /// `workflow_mcp/tools.rs`, `ref_check.rs` — READS them), and Rust has no
    /// read-only-public field, so the type cannot forbid
    ///
    /// ```ignore
    /// let mut e = ValidationError::err("semantic", "REAL_CODE", "…");
    /// e.code = "SNEAKY_CODE";           // ← re-labels the finding
    /// ```
    ///
    /// The scanner reads the CONSTRUCTOR, so all three checks (emitted set,
    /// registry, human copy) would agree on `REAL_CODE` while `SNEAKY_CODE` is
    /// what reaches the author — the same silent-agreement mode the `Self::`
    /// hole had. Narrowing the field's visibility cannot close it (the scan is
    /// crate-wide and `pub(crate)` still permits assignment), so it is made
    /// LOUD instead: assigning any of these post-construction is reported.
    const FINDING_FIELDS: &[&str] = &["code", "layer", "severity"];

    fn manifest_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    /// The builder's author-facing copy map. Read at RUNTIME (not `include_str!`)
    /// so it can never be a stale compiled-in snapshot.
    fn ui_copy_path() -> PathBuf {
        manifest_dir().join("../ui/src/modules/workflow/components/builder/validationCopy.ts")
    }

    /// Every Rust source that could construct a `ValidationError`.
    ///
    /// The whole SERVER crate is scanned, not a hand-listed pair of files:
    /// `ValidationError::{err,at,warn}` are `pub(crate)` and the struct's fields
    /// are `pub`, so any module — present or future — can emit a finding, and a
    /// hardcoded file list would leave it invisible to both halves of the guard.
    /// The desktop crate is included when present for the same reason (it
    /// depends on `ziee`, and the struct's `pub` fields make a struct literal
    /// possible there too).
    fn scan_roots() -> Vec<PathBuf> {
        let mut roots = vec![manifest_dir().join("src")];
        let desktop = manifest_dir().join("../desktop/tauri/src");
        if desktop.is_dir() {
            roots.push(desktop);
        } else {
            // The desktop crate is genuinely OPTIONAL (a server-only checkout has
            // no `../desktop` at all), but "the crate is there and its source dir
            // moved" must be LOUD: silently dropping the root would stop the guard
            // seeing that crate's emit sites while still passing.
            assert!(
                !manifest_dir().join("../desktop").is_dir(),
                "the desktop workspace exists but {} does not — the drift guard \
                 would silently stop scanning the desktop crate's `ValidationError` \
                 emit sites. Point `scan_roots` at the new source dir.",
                desktop.display()
            );
        }
        roots
    }

    fn collect_rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
        let entries = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("cannot read source dir {}: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                collect_rust_sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    // ── a Rust-source scanner that understands comments, strings and tests ──

    /// A `ValidationError::{err,at,warn}("<layer>", "<CODE>", …)` call found in
    /// REAL code — not in a comment, not inside a string literal, and not inside
    /// a `#[cfg(test)]` item (a test fixture must not inject a phantom code).
    struct Scan {
        codes: BTreeSet<String>,
        /// Constructions the guard could not read a `(layer, code)` pair out of.
        /// Each is a hole in the contract, so they FAIL the test loudly instead
        /// of being dropped.
        problems: Vec<String>,
    }

    fn is_ident_char(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }

    fn is_screaming_snake(s: &str) -> bool {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
    }

    fn starts_with(ch: &[char], i: usize, pat: &str) -> bool {
        pat.chars().enumerate().all(|(k, p)| ch.get(i + k) == Some(&p))
    }

    fn read_ident(ch: &[char], i: &mut usize) -> String {
        let start = *i;
        while ch.get(*i).is_some_and(|c| is_ident_char(*c)) {
            *i += 1;
        }
        ch[start..*i].iter().collect()
    }

    /// Whitespace + `//` line comments + nesting `/* */` block comments.
    fn skip_trivia(ch: &[char], i: &mut usize, line: &mut usize) {
        loop {
            match ch.get(*i) {
                Some('\n') => {
                    *line += 1;
                    *i += 1;
                }
                Some(c) if c.is_whitespace() => *i += 1,
                Some('/') if ch.get(*i + 1) == Some(&'/') => {
                    while ch.get(*i).is_some_and(|c| *c != '\n') {
                        *i += 1;
                    }
                }
                Some('/') if ch.get(*i + 1) == Some(&'*') => skip_block_comment(ch, i, line),
                _ => return,
            }
        }
    }

    fn skip_block_comment(ch: &[char], i: &mut usize, line: &mut usize) {
        let mut nest = 0usize;
        while *i < ch.len() {
            if starts_with(ch, *i, "/*") {
                nest += 1;
                *i += 2;
            } else if starts_with(ch, *i, "*/") {
                nest -= 1;
                *i += 2;
                if nest == 0 {
                    return;
                }
            } else {
                if ch[*i] == '\n' {
                    *line += 1;
                }
                *i += 1;
            }
        }
    }

    /// `"…"` with `\` escapes. Returns the content; `None` if `*i` is not on a
    /// `"` (a non-literal argument — the caller turns that into a problem).
    fn read_string_literal(ch: &[char], i: &mut usize, line: &mut usize) -> Option<String> {
        if ch.get(*i) != Some(&'"') {
            return None;
        }
        *i += 1;
        let mut out = String::new();
        while let Some(&c) = ch.get(*i) {
            match c {
                '\\' => {
                    if let Some(&e) = ch.get(*i + 1) {
                        if e == '\n' {
                            *line += 1;
                        }
                        out.push(e);
                    }
                    *i += 2;
                }
                '"' => {
                    *i += 1;
                    return Some(out);
                }
                '\n' => {
                    *line += 1;
                    out.push(c);
                    *i += 1;
                }
                _ => {
                    out.push(c);
                    *i += 1;
                }
            }
        }
        None
    }

    /// `r"…"` / `r#"…"#` / `br"…"`: returns the number of chars to the opening
    /// quote plus the hash count, or `None` when this is an ordinary identifier.
    fn raw_string_prefix(ch: &[char], i: usize) -> Option<(usize, usize)> {
        let mut j = i;
        if ch.get(j) == Some(&'b') {
            j += 1;
        }
        if ch.get(j) != Some(&'r') {
            return None;
        }
        j += 1;
        let mut hashes = 0usize;
        while ch.get(j) == Some(&'#') {
            hashes += 1;
            j += 1;
        }
        if ch.get(j) != Some(&'"') {
            return None;
        }
        Some((j + 1 - i, hashes))
    }

    fn skip_raw_string(ch: &[char], i: &mut usize, line: &mut usize, offset: usize, hashes: usize) {
        *i += offset;
        while *i < ch.len() {
            if ch[*i] == '"' && (1..=hashes).all(|k| ch.get(*i + k) == Some(&'#')) {
                *i += 1 + hashes;
                return;
            }
            if ch[*i] == '\n' {
                *line += 1;
            }
            *i += 1;
        }
    }

    /// Distinguish a char literal (`'x'`, `'\n'`, `'\u{1f600}'`) from a lifetime
    /// (`'a`). Advances past a char literal; leaves a lifetime to the ident path.
    fn skip_char_literal_or_lifetime(ch: &[char], i: &mut usize) {
        if ch.get(*i + 1) == Some(&'\\') {
            *i += 2;
            while ch.get(*i).is_some_and(|c| *c != '\'') {
                *i += 1;
            }
            *i += 1;
        } else if ch.get(*i + 2) == Some(&'\'') {
            *i += 3;
        } else {
            *i += 1; // a lifetime
        }
    }

    /// Scan one Rust source for `ValidationError` constructions.
    fn scan_rust_source(file: &str, src: &str, scan: &mut Scan) {
        let ch: Vec<char> = src.chars().collect();
        let mut i = 0usize;
        let mut line = 1usize;
        let mut depth: i32 = 0;
        // `Some(d)` while inside a `#[cfg(test)]` item body opened at depth `d`.
        let mut test_body_from: Option<i32> = None;
        let mut pending_cfg_test = false;
        // `Some(d)` while inside an `impl … ValidationError` body opened at depth
        // `d` — the ONE region where the bare keyword `Self` names the type, and
        // therefore where `Self::at("semantic", "CODE", …)` is a real emit site.
        let mut impl_ve_from: Option<i32> = None;
        let mut pending_impl_ve = false;
        let mut prev_word = String::new();

        while i < ch.len() {
            let c = ch[i];

            if c == '\n' {
                line += 1;
                i += 1;
                continue;
            }
            if c.is_whitespace() {
                i += 1;
                continue;
            }
            if c == '/' && (ch.get(i + 1) == Some(&'/') || ch.get(i + 1) == Some(&'*')) {
                skip_trivia(&ch, &mut i, &mut line);
                continue;
            }
            if let Some((offset, hashes)) = raw_string_prefix(&ch, i) {
                skip_raw_string(&ch, &mut i, &mut line, offset, hashes);
                prev_word.clear();
                continue;
            }
            if c == '"' {
                let mut sink_line = line;
                let _ = read_string_literal(&ch, &mut i, &mut sink_line);
                line = sink_line;
                prev_word.clear();
                continue;
            }
            if c == '\'' {
                skip_char_literal_or_lifetime(&ch, &mut i);
                continue;
            }
            if c == '#' && starts_with(&ch, i, "#[cfg(test)]") {
                pending_cfg_test = true;
                i += "#[cfg(test)]".chars().count();
                continue;
            }
            if c == '{' {
                if pending_cfg_test && test_body_from.is_none() {
                    test_body_from = Some(depth);
                }
                if pending_impl_ve && impl_ve_from.is_none() {
                    impl_ve_from = Some(depth);
                }
                pending_cfg_test = false;
                pending_impl_ve = false;
                depth += 1;
                i += 1;
                continue;
            }
            if c == '}' {
                depth -= 1;
                if test_body_from == Some(depth) {
                    test_body_from = None;
                }
                if impl_ve_from == Some(depth) {
                    impl_ve_from = None;
                }
                i += 1;
                continue;
            }
            if c == ';' {
                // A `#[cfg(test)] use …;` has no body to skip.
                pending_cfg_test = false;
                pending_impl_ve = false;
                i += 1;
                continue;
            }
            if c == '-' && ch.get(i + 1) == Some(&'>') {
                // Return-type position: `fn f() -> ValidationError {` is a
                // signature, not a struct literal.
                prev_word = "->".to_string();
                i += 2;
                continue;
            }
            if c == '.' {
                // Recorded so the ident that follows is known to be a FIELD
                // ACCESS (`e.code`) rather than a binding, a struct-literal key
                // or a path segment. `..` (range / struct update) is recorded
                // distinctly so it can never be read as a field access.
                let dots = if ch.get(i + 1) == Some(&'.') { 2 } else { 1 };
                prev_word = ".".repeat(dots);
                i += dots;
                continue;
            }
            if is_ident_char(c) && !c.is_ascii_digit() {
                let word = read_ident(&ch, &mut i);
                if word == "type" && test_body_from.is_none() {
                    // `type VE = ValidationError;` — the OTHER way to name the
                    // type something the scanner does not look for. Checked with
                    // a non-committing lookahead, so the tokens are still scanned
                    // normally afterwards.
                    scan_type_alias(&ch, i, line, file, scan);
                }
                if prev_word == "."
                    && test_body_from.is_none()
                    && FINDING_FIELDS.contains(&word.as_str())
                {
                    // `e.code = "…"` — a finding re-labelled AFTER the
                    // constructor this scanner read. Pure lookahead.
                    scan_field_assignment(&ch, i, line, file, &word, scan);
                }
                if word == "ValidationError" && test_body_from.is_none() {
                    let opens_impl = scan_validation_error_use(
                        &ch, &mut i, &mut line, file, &prev_word, scan,
                    );
                    if opens_impl {
                        pending_impl_ve = true;
                    }
                }
                if word == "Self" && test_body_from.is_none() && impl_ve_from.is_some() {
                    // Inside `impl … ValidationError`, `Self` IS the type — so a
                    // finding built through it must be exactly as visible to this
                    // scanner as one built through the spelled-out name.
                    scan_self_use(&ch, &mut i, &mut line, file, &prev_word, scan);
                }
                prev_word = word;
                continue;
            }
            i += 1;
        }
    }

    /// Called with `i` just past a `code` / `layer` / `severity` identifier that
    /// followed a `.` — i.e. a FIELD ACCESS on something.
    ///
    /// Reports an ASSIGNMENT to it (`e.code = "SNEAKY"`). A read, a comparison
    /// (`e.code == "…"` / `!=`), a method call (`e.code.to_string()`) and a
    /// `match e.code {` are all left alone — the consumers of findings do
    /// exactly those, and a guard that cried wolf on them would be turned off.
    ///
    /// Deliberately NOT type-aware: this lexer cannot know the receiver's type,
    /// so it over-approximates to "a field with one of these names, in a file
    /// that mentions `ValidationError` at all". That is the safe direction — a
    /// false positive is loud and one line to fix, a false negative re-opens the
    /// silent-agreement hole. Purely a lookahead: nothing is consumed.
    fn scan_field_assignment(
        ch: &[char],
        i: usize,
        line: usize,
        file: &str,
        field: &str,
        scan: &mut Scan,
    ) {
        let mut j = i;
        skip_trivia_no_line(ch, &mut j);
        if ch.get(j) != Some(&'=') {
            return; // a read, a call, a match scrutinee, …
        }
        if ch.get(j + 1) == Some(&'=') {
            return; // `==` is a comparison, not an assignment
        }
        scan.problems.push(format!(
            "{file}:{line}: a finding's `{field}` is assigned after construction. This \
             guard reads the `ValidationError::{{err,at,warn}}` CALL, so the code it \
             vouches for is the one the constructor named — a later `{field} = …` \
             re-labels the finding behind BOTH the registry and the author-facing-copy \
             check, and a wire code with no human copy reaches the author. Pass the \
             final values to the constructor instead."
        ));
    }

    /// A path (`a::b::ValidationError`) starting at `j`: returns its LAST
    /// segment and leaves `j` just past it. Empty when `j` is not on an ident.
    fn read_path_tail(ch: &[char], j: &mut usize) -> String {
        let mut last = String::new();
        loop {
            skip_trivia_no_line(ch, j);
            let seg = read_ident(ch, j);
            if seg.is_empty() {
                return last;
            }
            last = seg;
            let mut k = *j;
            skip_trivia_no_line(ch, &mut k);
            if !(ch.get(k) == Some(&':') && ch.get(k + 1) == Some(&':')) {
                return last;
            }
            *j = k + 2;
        }
    }

    /// `skip_trivia` for lookaheads that must not disturb the caller's line
    /// counter (nothing is committed, so the line number never changes).
    fn skip_trivia_no_line(ch: &[char], i: &mut usize) {
        let mut sink = 0usize;
        skip_trivia(ch, i, &mut sink);
    }

    /// Called with `i` just past the keyword `type`. Reports
    /// `type Alias = …::ValidationError;` — a second name for the type, under
    /// which `Alias::err("semantic", "NEW_CODE", …)` is invisible to the scanner.
    fn scan_type_alias(ch: &[char], i: usize, line: usize, file: &str, scan: &mut Scan) {
        let mut j = i;
        skip_trivia_no_line(ch, &mut j);
        let alias = read_ident(ch, &mut j);
        if alias.is_empty() {
            return; // not `type <Name> = …` (e.g. the field `r#type: …`)
        }
        skip_trivia_no_line(ch, &mut j);
        // Skip a generic parameter list so `type A<T> = …` is still seen.
        if ch.get(j) == Some(&'<') {
            let mut depth = 0usize;
            while let Some(&c) = ch.get(j) {
                match c {
                    '<' => depth += 1,
                    '>' => {
                        depth -= 1;
                        if depth == 0 {
                            j += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            skip_trivia_no_line(ch, &mut j);
        }
        if ch.get(j) != Some(&'=') {
            return;
        }
        j += 1;
        // Only a BARE alias (`= …::ValidationError;`) renames the type. A
        // wrapper (`= Result<(), ValidationError>;`) does not, and must not be
        // reported — the guard's problems have to stay true.
        let tail = read_path_tail(ch, &mut j);
        skip_trivia_no_line(ch, &mut j);
        if tail == "ValidationError" && ch.get(j) == Some(&';') {
            scan.problems.push(format!(
                "{file}:{line}: `type {alias} = …ValidationError;` aliases the type. \
                 A finding built through the alias (`{alias}::err(\"semantic\", \"CODE\", …)`) \
                 is invisible to this scanner, so its code escapes BOTH the registry and \
                 the author-facing-copy check. Name the type `ValidationError` at the \
                 construction site."
            ));
        }
    }

    /// Called with `*i` just past a `ValidationError` identifier in real code.
    ///
    /// Returns `true` when this occurrence is the HEAD of an `impl … ValidationError`
    /// — i.e. the `{` the caller is about to see opens a body in which the keyword
    /// `Self` names this type (see `scan_self_use`).
    fn scan_validation_error_use(
        ch: &[char],
        i: &mut usize,
        line: &mut usize,
        file: &str,
        prev_word: &str,
        scan: &mut Scan,
    ) -> bool {
        let mut j = *i;
        let mut jline = *line;
        skip_trivia(ch, &mut j, &mut jline);

        // `ValidationError { … }` — a struct literal. The guard cannot read a
        // code out of it, so it must never be how a finding is built.
        if ch.get(j) == Some(&'{') {
            if !matches!(
                prev_word,
                "struct" | "impl" | "enum" | "union" | "trait" | "for" | "->"
            ) {
                scan.problems.push(format!(
                    "{file}:{line}: `ValidationError {{ … }}` is built as a struct literal — \
                     the drift guard cannot read its `code`. Construct findings through \
                     `ValidationError::{{err,at,warn}}(\"<layer>\", \"<CODE>\", …)`."
                ));
            }
            // `impl ValidationError {` / `impl Trait for ValidationError {` open a
            // body where `Self` is an ALIAS for the type. Report it upward so the
            // caller can scan that body's `Self::…` uses.
            return matches!(prev_word, "impl" | "for");
        }

        // `use …::ValidationError as VE;` (or `use …::{ValidationError as VE}`).
        // Every later `VE::err("semantic", "NEW_CODE", …)` is invisible to this
        // scanner — it keys on the `ValidationError` identifier — so the code
        // would escape BOTH halves of the contract (the registry AND the
        // author-facing copy) SILENTLY. Every other hole here is loud; this one
        // was not.
        {
            let mut k = j;
            let mut kline = jline;
            let word = read_ident(ch, &mut k);
            if word == "as" {
                skip_trivia(ch, &mut k, &mut kline);
                let alias = read_ident(ch, &mut k);
                scan.problems.push(format!(
                    "{file}:{line}: `ValidationError` is imported under the alias \
                     '{alias}'. A finding built through it (`{alias}::err(\"semantic\", \
                     \"CODE\", …)`) is invisible to this scanner, so its code escapes \
                     BOTH the registry and the author-facing-copy check. Import the type \
                     under its own name."
                ));
                *i = k;
                *line = kline;
                return false;
            }
        }

        if !(ch.get(j) == Some(&':') && ch.get(j + 1) == Some(&':')) {
            return false; // a type mention (`Vec<ValidationError>`, a return type, …)
        }
        *i = j + 2;
        *line = jline;
        scan_ctor_call(ch, i, line, file, "ValidationError", scan);
        false
    }

    /// Called with `*i` just past a `Self` identifier inside an `impl …
    /// ValidationError` body, where `Self` IS `ValidationError`.
    ///
    /// Without this the idiomatic convenience constructor
    ///
    /// ```ignore
    /// impl ValidationError {
    ///     pub(crate) fn dead_tools(loc: &str) -> Self {
    ///         Self::at("semantic", "WORKFLOW_NEW_CODE", "…", loc)
    ///     }
    /// }
    /// ```
    ///
    /// emits a real code this scanner never sees — and a code missing from the
    /// emitted set AND from the humanisation demand at the same time makes the
    /// two halves AGREE: no copy is required, the guard is green, and a raw wire
    /// code reaches the author. Every other hole here is loud; that one was
    /// silent, which is the exact failure mode the guard exists to prevent.
    fn scan_self_use(
        ch: &[char],
        i: &mut usize,
        line: &mut usize,
        file: &str,
        prev_word: &str,
        scan: &mut Scan,
    ) {
        let mut j = *i;
        let mut jline = *line;
        skip_trivia(ch, &mut j, &mut jline);

        if ch.get(j) == Some(&'{') {
            // A TYPE position (`-> Self {`, `impl … for Self {`): the brace opens
            // a body, not a literal. Anywhere else `Self { … }` is a struct
            // literal of `ValidationError`, which the guard cannot read a code out
            // of — but the type's OWN `err`/`at`/`warn` constructors are built
            // that way and thread their `code` PARAMETER through, so only a
            // literal `code: "…"` is a finding built behind the guard's back.
            if !matches!(
                prev_word,
                "struct" | "impl" | "enum" | "union" | "trait" | "for" | "->"
            ) {
                scan_self_struct_literal(ch, j, *line, file, scan);
            }
            return; // leave the brace to the caller's depth accounting
        }

        if !(ch.get(j) == Some(&':') && ch.get(j + 1) == Some(&':')) {
            return; // `Option<Self>`, `Self,`, … — a type mention
        }
        *i = j + 2;
        *line = jline;
        scan_ctor_call(ch, i, line, file, "Self", scan);
    }

    /// A `Self { … }` struct literal inside `impl ValidationError`, starting at
    /// its `{`. Reports one carrying a LITERAL `code: "…"` field — that is a
    /// finding built without going through `err`/`at`/`warn`, so its code would
    /// escape both halves of the contract. Purely a lookahead: nothing is
    /// consumed, so the caller's depth accounting is untouched.
    fn scan_self_struct_literal(
        ch: &[char],
        brace: usize,
        line: usize,
        file: &str,
        scan: &mut Scan,
    ) {
        let mut j = brace + 1;
        let mut depth = 0usize;
        let mut sink = 0usize;
        while let Some(&c) = ch.get(j) {
            if c == '/' && (ch.get(j + 1) == Some(&'/') || ch.get(j + 1) == Some(&'*')) {
                skip_trivia(ch, &mut j, &mut sink);
                continue;
            }
            if let Some((offset, hashes)) = raw_string_prefix(ch, j) {
                skip_raw_string(ch, &mut j, &mut sink, offset, hashes);
                continue;
            }
            if c == '"' {
                let _ = read_string_literal(ch, &mut j, &mut sink);
                continue;
            }
            if c == '\'' {
                skip_char_literal_or_lifetime(ch, &mut j);
                continue;
            }
            match c {
                '{' | '(' | '[' => {
                    depth += 1;
                    j += 1;
                    continue;
                }
                '}' | ')' | ']' => {
                    if depth == 0 {
                        return; // end of this literal
                    }
                    depth -= 1;
                    j += 1;
                    continue;
                }
                _ => {}
            }
            if depth == 0 && is_ident_char(c) && !c.is_ascii_digit() {
                let word = read_ident(ch, &mut j);
                if word == "code" {
                    let mut k = j;
                    skip_trivia_no_line(ch, &mut k);
                    if ch.get(k) == Some(&':') {
                        k += 1;
                        skip_trivia_no_line(ch, &mut k);
                        let mut ksink = 0usize;
                        if let Some(code) = read_string_literal(ch, &mut k, &mut ksink) {
                            scan.problems.push(format!(
                                "{file}:{line}: `Self {{ …, code: \"{code}\", … }}` builds a \
                                 finding as a struct literal inside `impl ValidationError`, \
                                 bypassing the constructors the drift guard reads. Build it \
                                 through `ValidationError::{{err,at,warn}}(\"<layer>\", \
                                 \"<CODE>\", …)` (or `Self::…`) so the code is registered and \
                                 author-facing copy is demanded for it."
                            ));
                            return;
                        }
                    }
                }
                continue;
            }
            j += 1;
        }
    }

    /// The shared tail of both emit-site readers: called with `*i` just past the
    /// `::` of `<Type>::<ctor>(…)`, where `type_name` is how the type was spelled
    /// at this site (`ValidationError` or `Self`).
    fn scan_ctor_call(
        ch: &[char],
        i: &mut usize,
        line: &mut usize,
        file: &str,
        type_name: &str,
        scan: &mut Scan,
    ) {
        let mut j = *i;
        let mut jline = *line;
        skip_trivia(ch, &mut j, &mut jline);
        let ctor = read_ident(ch, &mut j);
        if !matches!(ctor.as_str(), "err" | "at" | "warn") {
            scan.problems.push(format!(
                "{file}:{line}: `{type_name}::{ctor}` is not one of the \
                 `err`/`at`/`warn` constructors the drift guard can read a code out of. \
                 Either build the finding through one of those, or teach \
                 `scan_ctor_call` about the new constructor."
            ));
            *i = j;
            *line = jline;
            return;
        }
        skip_trivia(ch, &mut j, &mut jline);
        if ch.get(j) != Some(&'(') {
            scan.problems.push(format!(
                "{file}:{line}: `{type_name}::{ctor}` is not followed by a call — \
                 the drift guard cannot read its layer/code."
            ));
            *i = j;
            *line = jline;
            return;
        }
        j += 1;

        // Argument 0 = layer, argument 1 = code. BOTH must be plain string
        // literals; anything else (a variable, a `const`, a `format!`) is a
        // finding the guard cannot verify, and is reported rather than skipped.
        skip_trivia(ch, &mut j, &mut jline);
        let Some(layer) = read_string_literal(ch, &mut j, &mut jline) else {
            scan.problems.push(format!(
                "{file}:{line}: the 1st argument of `{type_name}::{ctor}` is not a \
                 string literal, so the drift guard cannot tell which layer/code it emits."
            ));
            *i = j;
            *line = jline;
            return;
        };
        skip_trivia(ch, &mut j, &mut jline);
        if ch.get(j) != Some(&',') {
            scan.problems.push(format!(
                "{file}:{line}: `{type_name}::{ctor}(\"{layer}\" …)` has an \
                 unexpected argument shape — the drift guard cannot read its code."
            ));
            *i = j;
            *line = jline;
            return;
        }
        j += 1;
        skip_trivia(ch, &mut j, &mut jline);
        let Some(code) = read_string_literal(ch, &mut j, &mut jline) else {
            scan.problems.push(format!(
                "{file}:{line}: the 2nd argument of `{type_name}::{ctor}` is not a \
                 string literal, so the drift guard cannot register/humanise its code."
            ));
            *i = j;
            *line = jline;
            return;
        };

        *i = j;
        *line = jline;

        if !KNOWN_LAYERS.contains(&layer.as_str()) {
            scan.problems.push(format!(
                "{file}:{line}: `{type_name}::{ctor}` emits code '{code}' on the \
                 UNKNOWN layer '{layer}'. Add the layer to `KNOWN_LAYERS` (and to \
                 `ValidationError::layer`'s doc) — until then the guard refuses to \
                 vouch for the codes it emits."
            ));
            return;
        }
        if !is_screaming_snake(&code) {
            scan.problems.push(format!(
                "{file}:{line}: `{type_name}::{ctor}` emits '{code}', which is not a \
                 SCREAMING_SNAKE code — the builder keys its author-facing copy off the \
                 code, so it must be a stable identifier."
            ));
            return;
        }
        scan.codes.insert(code);
    }

    /// Every code emitted anywhere in the scanned crates, plus every emit site
    /// the scanner could not verify.
    fn scan_emitted_codes() -> Scan {
        let mut files = Vec::new();
        for root in scan_roots() {
            assert!(
                root.is_dir(),
                "scan root {} does not exist — the drift guard would silently stop \
                 seeing that crate's emit sites",
                root.display()
            );
            collect_rust_sources(&root, &mut files);
        }
        assert!(
            !files.is_empty(),
            "the drift guard found no Rust sources to scan — its roots have drifted"
        );

        let mut scan = Scan {
            codes: BTreeSet::new(),
            problems: Vec::new(),
        };
        let manifest = manifest_dir();
        for path in files {
            let src = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
            // Cheap pre-filter; the lexer below is the authority.
            if !src.contains("ValidationError") {
                continue;
            }
            let label = path
                .strip_prefix(&manifest)
                .unwrap_or(path.as_path())
                .display()
                .to_string();
            scan_rust_source(&label, &src, &mut scan);
        }
        scan
    }

    // ── the builder's copy map ─────────────────────────────────────────────

    /// A `'…'` / `"…"` / `` `…` `` literal, skipping `${…}` interpolations.
    fn read_ts_string(ch: &[char], i: &mut usize) -> Option<String> {
        let quote = *ch.get(*i)?;
        *i += 1;
        let mut out = String::new();
        while let Some(&c) = ch.get(*i) {
            if c == '\\' {
                if let Some(&e) = ch.get(*i + 1) {
                    out.push(e);
                }
                *i += 2;
                continue;
            }
            if c == quote {
                *i += 1;
                return Some(out);
            }
            if quote == '`' && c == '$' && ch.get(*i + 1) == Some(&'{') {
                *i += 2;
                let mut d = 1usize;
                while let Some(&k) = ch.get(*i) {
                    match k {
                        '{' => {
                            d += 1;
                            *i += 1;
                        }
                        '}' => {
                            d -= 1;
                            *i += 1;
                            if d == 0 {
                                break;
                            }
                        }
                        '"' | '\'' | '`' => {
                            read_ts_string(ch, i)?;
                        }
                        _ => *i += 1,
                    }
                }
                continue;
            }
            out.push(c);
            *i += 1;
        }
        None
    }

    fn skip_ts_trivia(ch: &[char], i: &mut usize) {
        loop {
            match ch.get(*i) {
                Some(c) if c.is_whitespace() => *i += 1,
                Some('/') if ch.get(*i + 1) == Some(&'/') => {
                    while ch.get(*i).is_some_and(|c| *c != '\n') {
                        *i += 1;
                    }
                }
                Some('/') if ch.get(*i + 1) == Some(&'*') => {
                    *i += 2;
                    while *i < ch.len() && !starts_with(ch, *i, "*/") {
                        *i += 1;
                    }
                    *i += 2;
                }
                _ => return,
            }
        }
    }

    fn find_chars(ch: &[char], pat: &str, from: usize) -> Option<usize> {
        (from..ch.len()).find(|&k| starts_with(ch, k, pat))
    }

    /// The KEYS of the builder's `HUMAN_COPY` map — i.e. the codes
    /// `humaniseFinding` can actually restate.
    ///
    /// This is deliberately a KEY parse, not a substring search over the file:
    /// `humaniseFinding` does `HUMAN_COPY[finding.code]` and falls back to the
    /// raw wire message when the lookup misses, so a code mentioned only in a
    /// comment (or in an unrelated array) must NOT satisfy the guard. Parsed
    /// against the map's STRUCTURE so reformatting/reordering the file is free.
    fn human_copy_keys(src: &str) -> Result<BTreeSet<String>, String> {
        let ch: Vec<char> = src.chars().collect();
        let decl = find_chars(&ch, "const HUMAN_COPY", 0).ok_or_else(|| {
            "no `const HUMAN_COPY` declaration in validationCopy.ts — the drift guard \
             reads that map's KEYS to know which codes the builder can restate"
                .to_string()
        })?;
        let mut i = decl + "const HUMAN_COPY".chars().count();
        while ch.get(i).is_some_and(|c| *c != '=') {
            i += 1;
        }
        if ch.get(i).is_none() {
            return Err("`const HUMAN_COPY` has no initializer".to_string());
        }
        i += 1;
        skip_ts_trivia(&ch, &mut i);
        if ch.get(i) != Some(&'{') {
            return Err("`const HUMAN_COPY` is not initialized with an object literal — \
                        the drift guard cannot enumerate its keys"
                .to_string());
        }
        i += 1;

        let mut keys = BTreeSet::new();
        let mut depth = 0usize;
        let mut expect_key = true;
        loop {
            skip_ts_trivia(&ch, &mut i);
            let Some(&c) = ch.get(i) else {
                return Err("unterminated `HUMAN_COPY` object literal".to_string());
            };
            if depth == 0 && c == '}' {
                break;
            }
            if expect_key && depth == 0 {
                let key = if c == '"' || c == '\'' || c == '`' {
                    read_ts_string(&ch, &mut i)
                        .ok_or_else(|| "unterminated key string in `HUMAN_COPY`".to_string())?
                } else if is_ident_char(c) && !c.is_ascii_digit() {
                    read_ident(&ch, &mut i)
                } else {
                    return Err(format!(
                        "`HUMAN_COPY` entry starting with `{c}` is neither a quoted nor a \
                         plain identifier key. A spread (`...OTHER`) or a computed key would \
                         hide codes from this guard — keep the keys literal."
                    ));
                };
                skip_ts_trivia(&ch, &mut i);
                if ch.get(i) != Some(&':') {
                    return Err(format!("`HUMAN_COPY` key '{key}' is not followed by ':'"));
                }
                i += 1;
                keys.insert(key);
                expect_key = false;
                continue;
            }
            match c {
                '{' | '(' | '[' => {
                    depth += 1;
                    i += 1;
                }
                '}' | ')' | ']' => {
                    if depth == 0 {
                        return Err(format!(
                            "unbalanced `{c}` while parsing `HUMAN_COPY` values"
                        ));
                    }
                    depth -= 1;
                    i += 1;
                }
                ',' if depth == 0 => {
                    expect_key = true;
                    i += 1;
                }
                '"' | '\'' | '`' => {
                    read_ts_string(&ch, &mut i)
                        .ok_or_else(|| "unterminated string in `HUMAN_COPY`".to_string())?;
                }
                _ => i += 1,
            }
        }
        Ok(keys)
    }

    // ── the tests ──────────────────────────────────────────────────────────

    /// TEST-1b — the SCANNER itself, on the source shapes it has to survive.
    ///
    /// Why this exists: the two set-comparisons below are only meaningful while
    /// the ~400-line hand-written Rust lexer above actually FINDS the emit sites.
    /// An UNDER-SCAN of an existing code is loud (both directions of the
    /// emitted-vs-registered comparison are asserted), but a NEW emit site the
    /// lexer fails to reach is invisible to BOTH sets at once — they agree, no
    /// copy is demanded, and a raw wire code ships green. So the lexer's
    /// behaviour is pinned here directly, on the shapes most likely to desync it:
    /// lifetimes vs char literals, raw/byte strings, and the `#[cfg(test)]` match.
    #[test]
    fn scanner_reads_awkward_source_shapes() {
        fn scan_of(src: &str) -> Scan {
            let mut scan = Scan {
                codes: BTreeSet::new(),
                problems: Vec::new(),
            };
            scan_rust_source("fixture.rs", src, &mut scan);
            scan
        }
        fn codes(src: &str) -> Vec<String> {
            scan_of(src).codes.into_iter().collect()
        }

        // 1. Lifetimes and char literals must not swallow the emit site that
        //    follows them. `'}'` would break brace/depth accounting and `'"'`
        //    would open a phantom string that eats the rest of the file.
        let lifetimes = "\
fn f<'a, 'b>(x: &'a str, y: &'b str) -> Vec<ValidationError> {
    let brace = '}';
    let quote = '\"';
    let esc = '\\'';
    let nl = '\\n';
    let uni = '\\u{1f600}';
    vec![ValidationError::err(\"semantic\", \"CODE_AFTER_CHAR_LITERALS\", \"m\")]
}
";
        let scan = scan_of(lifetimes);
        assert!(
            scan.problems.is_empty(),
            "char literals / lifetimes desynced the scanner: {:?}",
            scan.problems
        );
        assert_eq!(
            scan.codes.into_iter().collect::<Vec<_>>(),
            vec!["CODE_AFTER_CHAR_LITERALS".to_string()],
            "an emit site after a lifetime + char-literal run was not found"
        );

        // 2. Raw / raw-byte strings are SKIPPED (a code quoted inside one is not
        //    an emit site) and do not swallow the real emit site after them.
        let raws = concat!(
            "fn g() -> ValidationError {\n",
            "    let _sql = br#\"SELECT '\"' FROM t WHERE a = 1\"#;\n",
            "    let _doc = r##\"ValidationError::err(\"semantic\", \"FAKE_IN_RAW\", \"x\")\"##;\n",
            "    let _b = b\"bytes \\\" still bytes\";\n",
            "    ValidationError::at(\"schema\", \"CODE_AFTER_RAW_STRINGS\", \"m\", \"loc\")\n",
            "}\n",
        );
        let scan = scan_of(raws);
        assert!(
            scan.problems.is_empty(),
            "raw/byte strings desynced the scanner: {:?}",
            scan.problems
        );
        assert_eq!(
            scan.codes.into_iter().collect::<Vec<_>>(),
            vec!["CODE_AFTER_RAW_STRINGS".to_string()],
            "a raw string either hid the emit site after it, or injected a phantom code"
        );

        // 3. `#[cfg(test)]` bodies are skipped — a fixture must not inject a
        //    phantom code …
        let cfg_test = "\
#[cfg(test)]
mod tests {
    fn t() {
        let _ = ValidationError::err(\"semantic\", \"PHANTOM_FROM_TEST\", \"m\");
    }
}
";
        assert!(
            codes(cfg_test).is_empty(),
            "a `#[cfg(test)]` fixture injected a phantom code"
        );

        // … but the match is the LITERAL `#[cfg(test)]`, so any other cfg shape
        // is scanned as REAL code. That is the safe direction (the code shows up
        // as emitted → the registry/copy checks demand an entry) and it is pinned
        // here so a future "fix" cannot quietly turn it into a silent skip.
        let cfg_all = "\
#[cfg(all(test, feature = \"extra\"))]
mod tests {
    fn t() {
        let _ = ValidationError::err(\"semantic\", \"CODE_IN_CFG_ALL_TEST\", \"m\");
    }
}
";
        assert_eq!(
            codes(cfg_all),
            vec!["CODE_IN_CFG_ALL_TEST".to_string()],
            "`#[cfg(all(test, …))]` must not be treated as a silent skip — an emit \
             site there has to reach the registry/copy checks"
        );

        // 4. ALIASES — the two ways to give the type another name, under which
        //    every later `Alias::err(…)` is invisible to this scanner.
        let use_alias = "use crate::modules::workflow::validate::ValidationError as VE;\n\
                         fn h() { let _ = VE::err(\"semantic\", \"HIDDEN_CODE\", \"m\"); }\n";
        let scan = scan_of(use_alias);
        assert_eq!(
            scan.problems.len(),
            1,
            "an aliased import must be reported, not silently skipped: {:?}",
            scan.problems
        );
        assert!(
            scan.problems[0].contains("alias") && scan.problems[0].contains("VE"),
            "the alias problem must name the alias: {}",
            scan.problems[0]
        );
        assert!(scan.codes.is_empty());

        let type_alias = "type VE = crate::modules::workflow::validate::ValidationError;\n";
        let scan = scan_of(type_alias);
        assert_eq!(
            scan.problems.len(),
            1,
            "a `type` alias must be reported: {:?}",
            scan.problems
        );
        assert!(scan.problems[0].contains("VE"));

        // Negative controls: a WRAPPER type is not a rename, and neither is a
        // plain mention. Reporting those would make the guard cry wolf.
        for benign in [
            "type Findings = Vec<ValidationError>;\n",
            "type Checked = Result<(), ValidationError>;\n",
            "struct S { r#type: String, e: Vec<ValidationError> }\n",
            "fn k(v: &[ValidationError]) -> Option<&ValidationError> { v.first() }\n",
        ] {
            let scan = scan_of(benign);
            assert!(
                scan.problems.is_empty(),
                "benign source reported a problem ({benign:?}): {:?}",
                scan.problems
            );
            assert!(scan.codes.is_empty());
        }

        // 4b. `Self` inside `impl … ValidationError` — the THIRD renaming
        //     dialect, and the only SILENT one: `use … as VE` and `type VE = …`
        //     are both reported, but a convenience constructor written inside the
        //     type's own impl emits a code that used to be invisible to this
        //     scanner, so it went missing from the emitted set AND from the
        //     humanisation demand at once — the two agreed and the guard passed.
        let self_ctor = "\
impl ValidationError {
    pub(crate) fn dead_tools(loc: &str) -> Self {
        Self::at(\"semantic\", \"CODE_VIA_SELF\", \"m\", loc)
    }
}
";
        let scan = scan_of(self_ctor);
        assert!(
            scan.problems.is_empty(),
            "a `Self::at` constructor inside `impl ValidationError` reported a problem: {:?}",
            scan.problems
        );
        assert_eq!(
            scan.codes.into_iter().collect::<Vec<_>>(),
            vec!["CODE_VIA_SELF".to_string()],
            "a code emitted through `Self::at` inside `impl ValidationError` was invisible \
             to the scanner — it would escape BOTH the registry and the copy check silently"
        );

        // The trait-impl spelling opens the same `Self` alias.
        let self_in_trait_impl = "\
impl Default for ValidationError {
    fn default() -> Self {
        Self::err(\"schema\", \"CODE_VIA_SELF_TRAIT_IMPL\", \"m\")
    }
}
";
        assert_eq!(
            codes(self_in_trait_impl),
            vec!["CODE_VIA_SELF_TRAIT_IMPL".to_string()],
            "`impl Trait for ValidationError` also aliases `Self` to the type"
        );

        // A non-`err/at/warn` `Self::` constructor is refused, exactly like the
        // spelled-out form — and the message names `Self`, not the type.
        let self_other = "\
impl ValidationError {
    fn x() -> Self { Self::other(\"semantic\", \"X\") }
}
";
        let scan = scan_of(self_other);
        assert!(
            scan.problems.iter().any(|p| p.contains("`Self::other`")),
            "an unknown `Self::` constructor must be reported: {:?}",
            scan.problems
        );

        // A hand-rolled struct literal with a LITERAL code bypasses the
        // constructors entirely — also reported.
        let self_literal = "\
impl ValidationError {
    fn y() -> Self {
        Self { layer: \"semantic\", code: \"CODE_VIA_SELF_LITERAL\", message: String::new(),
               location: None, severity: Severity::Error }
    }
}
";
        let scan = scan_of(self_literal);
        assert!(
            scan.problems
                .iter()
                .any(|p| p.contains("struct literal") && p.contains("CODE_VIA_SELF_LITERAL")),
            "a `Self {{ code: \"…\" }}` literal must be reported: {:?}",
            scan.problems
        );
        assert!(scan.codes.is_empty());

        // Negative controls. The type's OWN constructors ARE `Self { … }`
        // literals threading a `code` PARAMETER through — reporting those would
        // make the guard cry wolf on the very file it guards. And `Self` outside
        // an `impl … ValidationError` belongs to some other type entirely.
        let real_ctor = "\
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
}
";
        let scan = scan_of(real_ctor);
        assert!(
            scan.problems.is_empty(),
            "the type's own constructor was reported: {:?}",
            scan.problems
        );
        assert!(scan.codes.is_empty());

        let self_elsewhere = "\
impl SomethingElse {
    fn f() -> Self { Self::at(\"semantic\", \"NOT_A_VALIDATION_CODE\", \"m\", \"l\") }
}
fn after() -> Vec<ValidationError> {
    vec![ValidationError::err(\"semantic\", \"CODE_AFTER_OTHER_IMPL\", \"m\")]
}
";
        let scan = scan_of(self_elsewhere);
        assert!(
            scan.problems.is_empty(),
            "`Self` in an unrelated impl was mis-read: {:?}",
            scan.problems
        );
        assert_eq!(
            scan.codes.into_iter().collect::<Vec<_>>(),
            vec!["CODE_AFTER_OTHER_IMPL".to_string()],
            "`Self` in an unrelated impl must contribute no code — and must not \
             desync the scanner for the emit site after it"
        );

        // 5. The constructions the guard already refuses stay refused.
        for (src, needle) in [
            (
                "fn a() { let _ = ValidationError { layer: \"semantic\".into(), code: \"X\".into() }; }\n",
                "struct literal",
            ),
            (
                "fn b() { let _ = ValidationError::other(\"semantic\", \"X\"); }\n",
                "not one of the",
            ),
            (
                "const C: &str = \"X\";\nfn c() { let _ = ValidationError::err(\"semantic\", C, \"m\"); }\n",
                "2nd argument",
            ),
            (
                "fn d() { let _ = ValidationError::err(\"runtime\", \"X\", \"m\"); }\n",
                "UNKNOWN layer",
            ),
            (
                "fn e() { let _ = ValidationError::err(\"semantic\", \"lower_case\", \"m\"); }\n",
                "SCREAMING_SNAKE",
            ),
        ] {
            let scan = scan_of(src);
            assert!(
                scan.problems.iter().any(|p| p.contains(needle)),
                "expected a problem containing {needle:?} for {src:?}, got {:?}",
                scan.problems
            );
        }

        // 6. FIX round 4 / finding 4 — POST-CONSTRUCTION field assignment.
        //    `ValidationError`'s fields are `pub`, so a finding can be built
        //    through a perfectly legitimate `::err(…)` (which the scanner reads,
        //    and whose code the registry vouches for) and then have its `code`
        //    OVERWRITTEN on the next line. The scanner records the constructor's
        //    code, the registry lists it, the copy map has an entry for it — all
        //    three agree — and the code that actually reaches the author is one
        //    nobody has ever written copy for. Same silent-agreement failure the
        //    `Self::` fix closed, through a different door, so it has to be as
        //    LOUD as every other hole here.
        for (src, needle) in [
            (
                "fn m() {\n    let mut e = ValidationError::err(\"semantic\", \"REAL_CODE\", \"m\");\n    e.code = \"SNEAKY_CODE\";\n}\n",
                "code",
            ),
            (
                "fn m() {\n    let mut e = ValidationError::err(\"semantic\", \"REAL_CODE\", \"m\");\n    e.layer = \"runtime\";\n}\n",
                "layer",
            ),
            (
                "fn m() {\n    let mut e = ValidationError::err(\"semantic\", \"REAL_CODE\", \"m\");\n    e.severity = Severity::Warning;\n}\n",
                "severity",
            ),
        ] {
            let scan = scan_of(src);
            assert!(
                scan.problems
                    .iter()
                    .any(|p| p.contains("assigned after construction") && p.contains(needle)),
                "a post-construction `{needle}` assignment was invisible to the guard \
                 ({src:?}), so it could re-label a finding behind every check's back: {:?}",
                scan.problems
            );
        }

        // Negative controls: reading, comparing and matching are not mutation,
        // and neither is a field named `code` on some OTHER type in a file that
        // merely mentions `ValidationError`. Reporting those would make the
        // guard cry wolf on the code that CONSUMES findings.
        for benign in [
            "fn r(e: &ValidationError) -> bool { e.code == \"WORKFLOW_NO_STEPS\" }\n",
            "fn r(e: &ValidationError) -> bool { e.code != \"WORKFLOW_NO_STEPS\" }\n",
            "fn r(e: &ValidationError) -> String { e.code.to_string() }\n",
            "fn r(e: &ValidationError) { match e.code { _ => () } }\n",
            "fn r(v: Vec<ValidationError>) -> usize { v.iter().filter(|e| e.severity == Severity::Error).count() }\n",
            "struct Row { code: String }\nfn r(e: &ValidationError) -> Row { Row { code: e.code.to_string() } }\n",
        ] {
            let scan = scan_of(benign);
            assert!(
                scan.problems.is_empty(),
                "reading a finding's fields was reported as mutation ({benign:?}): {:?}",
                scan.problems
            );
        }
    }

    // ── the two set-comparison tests ───────────────────────────────────────

    /// TEST-15 — the registry itself is well-formed, so the set comparisons
    /// below actually mean something.
    #[test]
    fn validation_codes_registry_is_well_formed() {
        let mut seen = BTreeSet::new();
        for code in VALIDATION_CODES {
            assert!(
                seen.insert(*code),
                "VALIDATION_CODES lists '{code}' twice — remove the duplicate"
            );
            assert!(
                is_screaming_snake(code),
                "VALIDATION_CODES entry '{code}' is not SCREAMING_SNAKE"
            );
        }
    }

    /// TEST-1 (acceptance, INV-1) — no raw schema language can reach the author.
    ///
    /// Three directions, all required:
    ///
    /// 1. every emit site is one the scanner can READ (no unverifiable
    ///    construction, no unknown layer, no non-literal code);
    /// 2. the emitted set and `VALIDATION_CODES` are the same set;
    /// 3. every registered code is a KEY of the builder's `HUMAN_COPY`.
    ///
    /// Adding a finding with a wire-vocabulary message is fine — shipping one
    /// the builder cannot restate in human language is not.
    #[test]
    fn validation_codes_are_registered_and_humanised() {
        let registered: BTreeSet<String> =
            VALIDATION_CODES.iter().map(|s| s.to_string()).collect();

        let scan = scan_emitted_codes();

        assert!(
            scan.problems.is_empty(),
            "the drift guard found {} `ValidationError` construction(s) it cannot verify. \
             Each one escapes BOTH the registry and the author-facing-copy check, so it \
             would reach the person building the workflow as a raw wire message:\n  - {}",
            scan.problems.len(),
            scan.problems.join("\n  - ")
        );

        let emitted = scan.codes;
        assert!(
            !emitted.is_empty(),
            "the emit-site scanner found no codes at all — it has drifted from \
             the `ValidationError::at(\"layer\", \"CODE\", …)` call shape and is \
             no longer guarding anything"
        );

        let unregistered: Vec<_> = emitted.difference(&registered).cloned().collect();
        assert!(
            unregistered.is_empty(),
            "these validation codes are emitted but NOT listed in \
             `VALIDATION_CODES` (validate.rs): {unregistered:?}\n\
             Add them there, then add author-facing copy for each in \
             src-app/ui/src/modules/workflow/components/builder/validationCopy.ts"
        );

        let stale: Vec<_> = registered.difference(&emitted).cloned().collect();
        assert!(
            stale.is_empty(),
            "these codes are listed in `VALIDATION_CODES` but no longer emitted \
             anywhere: {stale:?} — remove them from the registry (and from \
             validationCopy.ts) so the guard keeps meaning something"
        );

        let copy_path = ui_copy_path();
        let copy_src = std::fs::read_to_string(&copy_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", copy_path.display()));
        let humanised = human_copy_keys(&copy_src).unwrap_or_else(|e| {
            panic!(
                "cannot enumerate `HUMAN_COPY`'s keys in {}: {e}",
                copy_path.display()
            )
        });
        assert!(
            !humanised.is_empty(),
            "`HUMAN_COPY` parsed to zero keys — the key parser has drifted from the \
             map's shape and is no longer guarding anything"
        );

        let unhumanised: Vec<_> = registered.difference(&humanised).cloned().collect();
        assert!(
            unhumanised.is_empty(),
            "these validation codes have NO author-facing copy: {unhumanised:?}\n\
             The workflow builder would show the raw validator message (wire \
             vocabulary such as \"step has neither prompt: nor prompt_file:\") to \
             the person building the workflow. Add an entry for each to \
             src-app/ui/src/modules/workflow/components/builder/validationCopy.ts \
             (`HUMAN_COPY`), keyed by the quoted code literal."
        );
    }
}
