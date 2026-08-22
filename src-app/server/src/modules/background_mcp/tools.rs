//! Tool descriptors + dispatch for the built-in background_mcp server.
//!
//! The uniform background-run surface (ITEM-17) on the `workflow_runs`-backed
//! backbone: `spawn_background` (a WRITE — launches a detached run, routed
//! through approval) + `check_status` / `collect_result` (owner-scoped READS,
//! approval-bypassed). Ownership + the background boundary are enforced at every
//! read via `repository::find_background_run_for_owner` (a cross-user run — or a
//! classic `job_kind='workflow'` run — → 404, never leaks, never reads a workflow
//! run through the background surface).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use agent_core::{
    AgentEvent, AgentTurnRequest, Budget, CancelToken, EventSink, GateAsk, GateOutcome, HumanGate,
    ModelClient, ProviderModelClient, ReviewDecision, StopReason, ToolScope, TurnSeed,
};
use ai_providers::{ChatMessage, ContentBlock, Role};

use crate::common::AppError;
use crate::modules::chat::core::ai_provider::create_provider_from_model_id;
use crate::modules::notification::models::NewNotification;
use crate::modules::workflow::agent_dispatch::{
    build_detached_agent_core, DetachedAgentCoreArgs, RunNoteSteerPort,
};
use crate::modules::workflow::models::{CreateBackgroundRun, JobKind, WorkflowRunStatus};
use crate::modules::workflow::registry;
use crate::modules::workflow::repository;
use crate::modules::workflow::runner::{self, BackgroundOutcome};

use ziee_notification::create_and_emit;

/// Serialized-output paging cap for `collect_result` (mirrors `tool_result_mcp`).
const COLLECT_MAX_CHARS_CAP: usize = 100_000;
const COLLECT_DEFAULT_MAX_CHARS: usize = 20_000;

/// Copyable literal-JSON example carried by every `spec`-SHAPE refusal.
///
/// It uses ONLY keys `spec` actually advertises. It previously showed a
/// `"label"` key that no code path reads and no schema declares — an example
/// that names a field the server does not implement is this module's own defect
/// one level down, and it would now be refused by the unknown-key check below.
const BACKGROUND_SPEC_EXAMPLE: &str = r#"{"task":"Summarise the attached report"}"#;

// =====================================================================
// The `spawn_background` argument contract
// =====================================================================
//
// `kind` is declared as a TOP-LEVEL sibling of `spec`, but `spec` is an object
// whose visible properties are the per-kind fields (`command`, `flavor`, `task`,
// `system`) — so a model routinely nests `kind` beside them. The old dispatch
// read `args["kind"]` only, and `unwrap_or("subagent")` swallowed the nested one.
// That produced the two failures this contract exists to remove:
//
//   {"spec":{"kind":"sandbox_exec","command":"python hello.py"}}
//     → refused with "spec.task must be a non-empty string" — demanding a field
//       the caller deliberately did not send, never mentioning `kind`.
//   {"spec":{"kind":"sandbox_exec","task":"…"}}
//     → SUCCEEDED, running a sub-agent instead of the requested command, with no
//       error at all. Silent wrong-thing is the worse half.
//
// The resolution, the per-kind key rules, the refusal text and the dispatch are
// all derived from `KIND_CONTRACTS` below, so a refusal cannot drift from the
// contract it describes. What is NOT generated is the tool schema itself — it is
// a static JSON literal, so adding a kind means editing it too. That edit is not
// left to memory: `unadvertised_spec_keys_are_refused_and_advertised_ones_accepted`
// fails if the schema's `kind` enums or `spec` properties disagree with this
// table — which is precisely the advertisement-vs-enforcement gap this module is
// being fixed for, one level up.

/// One background kind's whole model-facing contract.
struct KindContract {
    /// The value of `kind` as the schema advertises it.
    name: &'static str,
    /// The run row's `job_kind`. Carried HERE, not hardcoded in each spawner:
    /// the dispatch matches on this field and each spawner writes the row from
    /// the same field, so the arm that was chosen and the row that gets written
    /// cannot disagree. (They previously could — a spawner took a
    /// `&KindContract` used only for refusals while hardcoding its own
    /// `JobKind`, so the wrong contract compiled cleanly and produced a
    /// `subagent` run that demanded `spec.command`: the exact silent-mismatch
    /// class this module is being fixed for.)
    job_kind: JobKind,
    /// The `spec` field this kind cannot run without.
    required_field: &'static str,
    /// The `spec` fields this kind accepts but does not require.
    optional_fields: &'static [&'static str],
    /// Error code for a missing `required_field`. Preserved verbatim from the
    /// pre-existing refusals so no caller's error-code handling changes.
    missing_code: &'static str,
    /// What this kind DOES, phrased to complete "If you meant to …".
    intent: &'static str,
    /// A literal-JSON `arguments` object the model can copy verbatim.
    example: &'static str,
}

impl KindContract {
    /// Every `spec` key valid for THIS kind, in schema order.
    fn own_fields(&self) -> impl Iterator<Item = &'static str> + '_ {
        std::iter::once(self.required_field).chain(self.optional_fields.iter().copied())
    }
}

const KIND_CONTRACTS: &[KindContract] = &[
    KindContract {
        name: "subagent",
        job_kind: JobKind::SubAgent,
        required_field: "task",
        optional_fields: &["system"],
        missing_code: "BACKGROUND_TASK_REQUIRED",
        intent: "run a detached sub-agent on a self-contained task",
        example: r#"{"kind":"subagent","spec":{"task":"Summarise the attached report"}}"#,
    },
    KindContract {
        name: "sandbox_exec",
        job_kind: JobKind::SandboxExec,
        required_field: "command",
        optional_fields: &["flavor"],
        missing_code: "BACKGROUND_COMMAND_REQUIRED",
        intent: "run a shell command in this conversation's code sandbox",
        example: r#"{"kind":"sandbox_exec","spec":{"command":"python hello.py"}}"#,
    },
];

/// The kind used when NO `kind` is supplied anywhere.
///
/// Only an ABSENT `kind` falls here. A `kind` the caller actually supplied — in
/// either location — is never replaced by it.
const DEFAULT_KIND: &str = "subagent";

/// Is `key` a `spec` field the RESOLVED kind can actually act on?
///
/// The accepted set is `kind` + this kind's own fields + the OTHER kinds'
/// REQUIRED fields. That last clause is narrow on purpose: the other kind's
/// required field is the signature of a MISPLACED `kind`, and letting it through
/// to [`missing_spec_field`] buys a precise diagnosis ("`spec.command` belongs to
/// `kind: sandbox_exec`") that a blunt "unknown key" cannot give.
///
/// Everything else is refused — including the other kind's OPTIONAL fields
/// (`flavor` on a sub-agent spec, `system` on a sandbox spec). Those carry no
/// diagnostic value: accepting them meant reading a field the spawner never
/// looks at and telling the caller nothing, which is the same silent-ignore this
/// module exists to remove. (An earlier revision accepted the whole union and a
/// test certified that silence.)
fn spec_key_is_valid(kind: &'static KindContract, key: &str) -> bool {
    key == "kind"
        || kind.own_fields().any(|f| f == key)
        || KIND_CONTRACTS
            .iter()
            .any(|k| k.name != kind.name && k.required_field == key)
}

/// The `spec` keys the resolved kind accepts, rendered for a refusal.
fn accepted_spec_keys(kind: &'static KindContract) -> String {
    let mut parts: Vec<String> = kind.own_fields().map(|f| format!("`{f}`")).collect();
    parts.push("`kind`".to_string());
    parts.join(", ")
}

/// How many offending key names a single unknown-field refusal lists.
///
/// The key set is model-supplied and unbounded; naming a few is enough to fix
/// the call, and the cap keeps a runaway payload from inflating the message.
const MAX_REPORTED_UNKNOWN_KEYS: usize = 5;

/// `` `subagent`, `sandbox_exec` `` — the advertised kinds, quoted, for a refusal.
fn quoted_kind_names() -> String {
    KIND_CONTRACTS
        .iter()
        .map(|k| format!("`{}`", k.name))
        .collect::<Vec<_>>()
        .join(", ")
}

fn find_kind(name: &str) -> Option<&'static KindContract> {
    KIND_CONTRACTS.iter().find(|k| k.name == name)
}

/// The canonical example, used by refusals that are not yet kind-specific.
fn default_example() -> &'static str {
    KIND_CONTRACTS
        .iter()
        .find(|k| k.name == DEFAULT_KIND)
        .map(|k| k.example)
        // Unreachable while DEFAULT_KIND names a table entry; a fallback rather
        // than an `expect` so a future table edit degrades the MESSAGE instead
        // of panicking on model-supplied input.
        .unwrap_or(BACKGROUND_SPEC_EXAMPLE)
}

/// The ONE `BACKGROUND_KIND_UNKNOWN` message builder.
///
/// Three hand-maintained copies of this text existed briefly (the empty-kind
/// arm, `parse_spawn_args`, and the fail-closed dispatch arm) and could drift
/// apart with nothing going red. `location` is `""` for the top-level argument
/// or `" (inside \`spec\`)"` for the nested one.
fn unknown_kind_error(received: &str, location: &str) -> AppError {
    AppError::bad_request(
        "BACKGROUND_KIND_UNKNOWN",
        format!(
            "`kind`{location} was `{received}`, but it must be one of {kinds}. `kind` \
             is a top-level sibling of `spec` (supplying it inside `spec` is also \
             accepted). Example: {example}",
            received = crate::common::tool_args::truncate_for_message(received),
            kinds = quoted_kind_names(),
            example = default_example(),
        ),
    )
}

/// Read a `kind` value out of `container`.
///
/// `Ok(None)` = absent or explicit `null`, so the caller's own default applies.
/// A PRESENT non-string is refused, never defaulted: a supplied argument is
/// never silently replaced by the default.
fn read_kind(container: &Value, location: &str) -> Result<Option<String>, AppError> {
    match container.get("kind") {
        None | Some(Value::Null) => Ok(None),
        // An empty / whitespace-only string is refused HERE, as an invalid kind.
        // Two wrong alternatives were considered: letting it through as `Some("")`
        // makes it "disagree" with a real sibling value and produces a
        // CONFLICT refusal naming a contradiction that does not exist
        // (`subagent` vs ``); mapping it to `None` silently substitutes the
        // default for a value the caller supplied, which INV-2 forbids. Refusing
        // it as an unknown kind is the only reading that is both accurate and
        // non-silent.
        Some(Value::String(s)) if s.trim().is_empty() => Err(unknown_kind_error(s.trim(), location)),
        Some(Value::String(s)) => Ok(Some(s.trim().to_string())),
        Some(other) => Err(AppError::bad_request(
            "BACKGROUND_KIND_INVALID",
            format!(
                "`kind`{location} arrived as {received}, but a string naming the \
                 background job kind is required — one of {kinds}. Example: {example}",
                received = crate::common::tool_args::type_word(other),
                kinds = quoted_kind_names(),
                example = default_example(),
            ),
        )),
    }
}

/// The validated result of `spawn_background`'s arguments: the resolved kind and
/// the `spec` with its (consumed) `kind` removed.
struct SpawnArgs {
    kind: &'static KindContract,
    spec: Value,
}

/// Validate + resolve `spawn_background{kind, spec}` BEFORE any dispatch.
///
/// * `kind` is read from the top level OR from inside `spec`. A nested-only
///   `kind` is HONOURED rather than dropped.
/// * Two DISAGREEING `kind`s are refused rather than resolved by preference: a
///   contradiction is not a value, and quietly picking a side is the same silent
///   resolution this function exists to remove.
/// * `spec` is held to the keys the schema advertises (which now declares
///   `additionalProperties: false`, so the advertisement and the enforcement are
///   the same statement), turning a silently-ignored typo into a refusal.
fn parse_spawn_args(args: &Value) -> Result<SpawnArgs, AppError> {
    // `spec` is a declared object argument, which models routinely JSON-ENCODE.
    // Left undecoded it survives a presence check as a `Value::String`, and the
    // per-kind readers then report "spec.task must be a non-empty string" — a
    // LIE, since the field was supplied. Decode it first so everything below
    // (including the nested-`kind` read) sees a real object.
    let mut spec = decode_spec_arg(args)?;

    let top = read_kind(args, "")?;
    let nested = read_kind(&spec, " (inside `spec`)")?;

    let resolved = match (top.as_deref(), nested.as_deref()) {
        (Some(a), Some(b)) if a != b => {
            return Err(AppError::bad_request(
                "BACKGROUND_KIND_CONFLICT",
                format!(
                    "`kind` was supplied twice with DIFFERENT values: `{a}` at the top \
                     level and `{b}` inside `spec`. Supply it once — `kind` is a \
                     top-level sibling of `spec`. Example: {example}",
                    a = crate::common::tool_args::truncate_for_message(a),
                    b = crate::common::tool_args::truncate_for_message(b),
                    example = default_example(),
                ),
            ));
        }
        (Some(a), _) => a,
        (None, Some(b)) => b,
        (None, None) => DEFAULT_KIND,
    };

    let kind = find_kind(resolved).ok_or_else(|| unknown_kind_error(resolved, ""))?;

    if let Some(map) = spec.as_object() {
        let offending: Vec<&String> = map.keys().filter(|k| !spec_key_is_valid(kind, k)).collect();
        if !offending.is_empty() {
            let shown: Vec<String> = offending
                .iter()
                .take(MAX_REPORTED_UNKNOWN_KEYS)
                .map(|k| format!("`{}`", crate::common::tool_args::truncate_for_message(k)))
                .collect();
            // The key set is model-supplied and unbounded, so only the first few
            // are named — but SAY that the list was cut, or a model that fixes
            // the named keys hits the same refusal again with no idea why.
            let more = offending.len().saturating_sub(shown.len());
            let tail = if more > 0 {
                format!(" (and {more} more)")
            } else {
                String::new()
            };
            return Err(AppError::bad_request(
                "BACKGROUND_SPEC_UNKNOWN_FIELD",
                format!(
                    "`spec` contains field(s) `kind: {name}` does not accept: {got}{tail}. \
                     For `kind: {name}`, `spec` accepts only {accepted}. Example: {example}",
                    name = kind.name,
                    got = shown.join(", "),
                    accepted = accepted_spec_keys(kind),
                    example = kind.example,
                ),
            ));
        }
    }

    // `kind` is consumed here. Remove it so the spec persisted as `inputs_json`
    // carries only the vocabulary the schema declares, and no later reader can
    // re-derive a kind from a stale copy of a value already resolved.
    if let Some(map) = spec.as_object_mut() {
        map.remove("kind");
    }

    Ok(SpawnArgs { kind, spec })
}

/// The refusal for a `spec` missing the field its kind requires.
///
/// When the spec instead carries the OTHER kind's required field, the message
/// names THAT as the real mistake — a misplaced `kind` — rather than demanding a
/// field the caller deliberately did not send. Demanding it
/// (`spec.task must be a non-empty string`, for a spec that supplied `command`)
/// is the reported symptom this whole contract exists to remove.
fn missing_spec_field(kind: &'static KindContract, spec: &Value) -> AppError {
    // "Supplied" must mean the same thing here as it does in `require_spec_field`
    // — a non-empty string. `is_some()` alone is true for an explicit `null` and
    // for a wrong type, which steered the model toward a kind whose required
    // field was ALSO null: a confident hint pointing at another dead end.
    let sibling = KIND_CONTRACTS
        .iter()
        .find(|k| k.name != kind.name && spec_field_supplied(spec, k.required_field));

    let message = match sibling {
        Some(other) => format!(
            "`spec.{req}` is required for `kind: {name}`, but `spec` supplied \
             `spec.{other_req}` instead — that field belongs to `kind: {other_name}`. \
             If you meant to {other_intent}, set `kind` to `{other_name}` (it is a \
             top-level sibling of `spec`); otherwise supply a non-empty \
             `spec.{req}` string. Example: {example}",
            req = kind.required_field,
            name = kind.name,
            other_req = other.required_field,
            other_name = other.name,
            other_intent = other.intent,
            example = other.example,
        ),
        None => format!(
            "`spec.{req}` is required for `kind: {name}` and must be a non-empty \
             string. Example: {example}",
            req = kind.required_field,
            name = kind.name,
            example = kind.example,
        ),
    };
    AppError::bad_request(kind.missing_code, message)
}

/// Was `field` supplied as a usable value — i.e. a non-empty string?
///
/// ONE definition, shared by [`require_spec_field`] (which consumes the value)
/// and [`missing_spec_field`] (which reasons about the sibling kind's field), so
/// the refusal cannot claim a field was "supplied" that the reader would reject.
fn spec_field_supplied(spec: &Value, field: &str) -> bool {
    spec.get(field)
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.trim().is_empty())
}

/// Read the non-empty string `spec.<kind.required_field>`, or refuse via
/// [`missing_spec_field`].
fn require_spec_field(kind: &'static KindContract, spec: &Value) -> Result<String, AppError> {
    spec.get(kind.required_field)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| missing_spec_field(kind, spec))
}

/// Decode the `spec` object argument, which models routinely JSON-ENCODE.
///
/// Extracted so the shape distribution can be driven through it directly.
fn decode_spec_arg(args: &Value) -> Result<Value, AppError> {
    crate::common::tool_args::coerce_arg(
        args,
        "spec",
        crate::common::tool_args::ArgShape::Object,
        BACKGROUND_SPEC_EXAMPLE,
    )
    .map_err(|e| AppError::bad_request("BACKGROUND_SPEC_INVALID", e.into_message()))?
    .ok_or_else(|| {
        AppError::bad_request(
            "BACKGROUND_SPEC_REQUIRED",
            // The example here must be a full ARGUMENTS object, not a `spec`-level
            // one: the mistake being corrected is a MISSING `spec` key, so an
            // example without that key is one a model can copy verbatim and hit
            // the identical error again. (Every sibling refusal in this contract
            // already carries a full arguments object.)
            format!(
                "`spec` was not supplied, but a JSON object describing the work is \
                 required. Example: {example}",
                example = default_example(),
            ),
        )
    })
}

/// Static tool descriptors emitted by `tools/list`.
pub fn tool_list() -> Value {
    json!({
        "tools": [
            {
                "name": "spawn_background",
                "description": "Launch background work DETACHED from this conversation. After you spawn, END YOUR TURN — do NOT poll or loop on check_status. When a 'subagent' run finishes you are AUTOMATICALLY re-engaged in this conversation with its result (a new turn arrives carrying the sub-agent's answer), so just stop and wait. Two kinds: 'subagent' runs a detached agent turn on a self-contained task (research a question, draft a section, analyze data) and auto-resumes this conversation with its result when done; 'sandbox_exec' runs a shell command in this conversation's isolated code sandbox as a background job (a long build, a data crunch, a test suite) — its completion drops a notification in the user's inbox (collect_result reads its output on demand). Use for a bounded unit of work whose answer you don't need inline right now. Returns an opaque `run_id`. Do NOT use for trivial things you can answer directly. This LAUNCHES work, so it requires approval before it starts.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "kind": {
                            "type": "string",
                            "enum": ["subagent", "sandbox_exec"],
                            "default": "subagent",
                            "description": "The background job kind. 'subagent' runs a detached agent turn on the spec; 'sandbox_exec' runs a shell command in this conversation's code sandbox as a background job. This is a TOP-LEVEL sibling of `spec` — that is the canonical place for it, though supplying it inside `spec` is also accepted. Do not supply it in both places with different values."
                        },
                        "spec": {
                            "type": "object",
                            "additionalProperties": false,
                            "description": "What the background job should do. The fields are PER-KIND and the server enforces that: 'subagent' accepts `task` (required) and `system`; 'sandbox_exec' accepts `command` (required) and `flavor`. Every field listed here carries its kind in parentheses. A field that is unrecognized, or that belongs to the other kind, is REFUSED rather than silently ignored — the one exception is the other kind's required field (`task`/`command`), which is accepted only so the error can tell you that your `kind` is wrong.",
                            "properties": {
                                "kind": {
                                    "type": "string",
                                    "enum": ["subagent", "sandbox_exec"],
                                    "description": "The same value as the top-level `kind`. Accepted here because this object's other fields are per-kind, which makes it a natural place to put it; the top level remains the canonical location. Supplying both with DIFFERENT values is refused."
                                },
                                "system": {
                                    "type": "string",
                                    "description": "(subagent) Optional system framing / role for the sub-agent."
                                },
                                "task": {
                                    "type": "string",
                                    "description": "(subagent) The concrete task the sub-agent must complete and report back on."
                                },
                                "command": {
                                    "type": "string",
                                    "description": "(sandbox_exec) The shell command to run in the conversation's code sandbox. The same isolated workspace + attachments the foreground `execute_command` tool sees."
                                },
                                "flavor": {
                                    "type": "string",
                                    "enum": ["minimal", "full"],
                                    "default": "minimal",
                                    "description": "(sandbox_exec) The rootfs flavor to run in. Defaults to 'minimal'; matches the foreground execute_command flavor lock for this conversation."
                                }
                            }
                        }
                    },
                    "required": ["spec"]
                }
            },
            {
                "name": "check_status",
                "description": "OPTIONAL one-off peek at the state of a background run you spawned, by its `run_id`. You do NOT need this to receive a sub-agent's result — a completed 'subagent' run re-engages this conversation automatically. Use it only if you want to check on a run out-of-band (e.g. a long 'sandbox_exec' job): it reports whether the run is still running, completed, failed, or waiting, WITHOUT fetching the full result (use `collect_result` for that). Do NOT call it in a loop. Only your own runs are visible; an unknown or foreign id returns not found.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "run_id": { "type": "string", "format": "uuid", "description": "The run_id returned by spawn_background." }
                    },
                    "required": ["run_id"]
                }
            },
            {
                "name": "collect_result",
                "description": "Read the final result of a background run by its `run_id`, on demand. Idempotent — safe to call repeatedly — and paged for large outputs via `offset`/`max_chars`. You do NOT need to call this to receive a 'subagent' result: a completed sub-agent re-engages this conversation automatically with its answer. Use collect_result to fetch a 'sandbox_exec' run's output, to re-read a large result page-by-page, or to look up a result you were notified about. If the run has not finished yet it returns its current status instead of a result. Only your own runs are visible; an unknown or foreign id returns not found.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "run_id": { "type": "string", "format": "uuid", "description": "The run_id returned by spawn_background." },
                        "offset": { "type": "integer", "minimum": 0, "default": 0, "description": "Character offset into the serialized final output (for paging large results)." },
                        "max_chars": { "type": "integer", "minimum": 1, "maximum": 100000, "default": 20000, "description": "Max characters of the final output to return in this page." }
                    },
                    "required": ["run_id"]
                }
            }
        ]
    })
}

/// Per-tool approval classifier (mirrors `control_mcp::handlers::control_call_needs_approval`).
/// `spawn_background` LAUNCHES a detached agent → it must go through the reviewer/
/// approval gate even under `ApprovalMode::AutoApprove` (the security posture).
/// `check_status` / `collect_result` are owner-scoped reads → auto-run. Anything
/// unrecognized fails safe → require approval. Consumed by the `is_background`
/// arm added to `mcp/chat_extension/mcp.rs`'s approval ladder.
pub fn background_call_needs_approval(tool_name: &str) -> bool {
    match tool_name {
        "check_status" | "collect_result" => false,
        // spawn_background (write) + anything unknown → approve (fail-safe).
        _ => true,
    }
}

/// Dispatch a `tools/call`. Returns the inner tool Value; the handler wraps it in
/// the MCP `content`/`structuredContent` envelope.
pub async fn call_tool(
    pool: &PgPool,
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    tool_name: &str,
    args: &Value,
) -> Result<Value, AppError> {
    match tool_name {
        "spawn_background" => spawn_background(pool, user_id, conversation_id, args).await,
        "check_status" => check_status(pool, user_id, args).await,
        "collect_result" => collect_result(pool, user_id, args).await,
        other => Err(AppError::bad_request(
            "BACKGROUND_UNKNOWN_TOOL",
            format!("unknown background tool '{other}'"),
        )),
    }
}

fn parse_run_id(args: &Value) -> Result<Uuid, AppError> {
    let raw = args
        .get("run_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::bad_request("BACKGROUND_RUN_ID_REQUIRED", "run_id is required"))?;
    Uuid::parse_str(raw).map_err(|_| AppError::bad_request("BACKGROUND_RUN_ID_INVALID", "run_id must be a valid UUID"))
}

/// `spawn_background{kind, spec}` — create + fire-and-forget a background run of
/// the given `JobKind`, returning an opaque owner-scoped `run_id` (DEC-36). The
/// run is driven to terminal by [`runner::spawn_background_run`] (shared
/// heartbeat + guarded transitions + `SyncEntity::WorkflowRun` completion
/// notify). Dispatch on `kind`: each kind's spec-parse + driver wiring lives in
/// its own `spawn_*` helper below, so adding a kind is additive (no central
/// spec-shape god-fn).
async fn spawn_background(
    pool: &PgPool,
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    args: &Value,
) -> Result<Value, AppError> {
    // Resolve + validate the whole argument contract in ONE place, before any
    // dispatch: `kind` from either location, no contradictions, no unadvertised
    // `spec` keys. See `parse_spawn_args`.
    let SpawnArgs { kind, spec } = parse_spawn_args(args)?;

    // Dispatch on the contract's OWN `job_kind`, which is also the value each
    // spawner writes to the run row — so the arm chosen and the row written
    // cannot disagree.
    match kind.job_kind {
        JobKind::SubAgent => spawn_subagent(pool, user_id, conversation_id, kind, spec).await,
        JobKind::SandboxExec => {
            spawn_sandbox_exec(pool, user_id, conversation_id, kind, spec).await
        }
        // `parse_spawn_args` already refused every name outside `KIND_CONTRACTS`,
        // so this arm is unreachable. It stays as a fail-closed guard rather than
        // a panic: the value originates in model-supplied input, and a future
        // table entry without a dispatch arm must refuse, not abort the request.
        // It shares the ONE message builder rather than re-formatting the text.
        _ => Err(unknown_kind_error(kind.name, "")),
    }
}

/// `spawn_background{kind:'subagent'}` — launch a detached [`JobKind::SubAgent`]
/// agent-core turn on `spec.{task,system}`.
async fn spawn_subagent(
    pool: &PgPool,
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    kind: &'static KindContract,
    spec: Value,
) -> Result<Value, AppError> {
    // From the contract that selected this arm — never a second hardcoded
    // literal that could disagree with the dispatch.
    let job_kind = kind.job_kind;
    let task = require_spec_field(kind, &spec)?;
    let system = spec
        .get("system")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Resolve the model the detached sub-agent runs on: the originating
    // conversation's model. (`create_provider_from_model_id` at turn time verifies
    // the provider is ENABLED, but NOT the user's group access — that fire-time
    // access re-check is done explicitly by the push-to-resume path, mirroring the
    // scheduler.) A conversation with no model set — or a spawn with no
    // conversation context — has nothing to run on, so reject clearly instead of
    // launching a doomed run. Recorded on the run row so the choice is durable +
    // auditable.
    let model_id = match conversation_id {
        Some(cid) => crate::core::Repos
            .chat
            .core
            .get_conversation(cid, user_id)
            .await?
            .and_then(|c| c.model_id),
        None => None,
    };
    let model_id = model_id.ok_or_else(|| {
        AppError::bad_request(
            "BACKGROUND_NO_MODEL",
            "no model is available for the background sub-agent (the originating conversation has no model set)",
        )
    })?;

    let request = CreateBackgroundRun {
        job_kind,
        conversation_id,
        user_id,
        model_id: Some(model_id),
        sandbox_flavor: None,
        // An LLM tool call from a conversation (mirrors workflow_mcp's
        // `wf_<slug>` convention for a chat-model-driven run).
        invocation_source: "conversation".into(),
        inputs_json: spec.clone(),
    };

    // Capture the spec into the detached driver (ITEM-7 / ITEM-9). The driver
    // runs OUTSIDE any per-conversation single-flight lock — this is
    // fire-and-forget, so the foreground chat stays interactive.
    let run_id = runner::spawn_background_run(pool, request, move |task_pool, run_id, handle| async move {
        execute_subagent_run(&task_pool, run_id, user_id, conversation_id, model_id, handle, &system, &task).await
    })
    .await?;

    Ok(json!({
        "run_id": run_id,
        "kind": job_kind.as_str(),
        "status": "pending",
        "note": "Background sub-agent started. END your turn now — do NOT poll. When it finishes, this conversation is automatically re-engaged with its result."
    }))
}

/// Quiet [`EventSink`] for a detached background sub-agent. Unlike the workflow
/// `kind: agent` host (which streams `StepProgress` over a live SSE channel), a
/// background run has no attached request stream — the foreground chat moved on —
/// so loop events are dropped. (Surfacing progress into `step_progress_json` for
/// `check_status` is a follow-up.) `check_status` / `collect_result` are the
/// owner-scoped read surface for a background run's state + result.
struct BackgroundEventSink;

#[async_trait]
impl EventSink for BackgroundEventSink {
    async fn emit(&self, _ev: AgentEvent) {}
}

/// Unattended [`HumanGate`] for a detached background sub-agent (DEC-117). A
/// background run has NO human to answer a prompt, so any call the approval
/// policy / reviewer routes to the gate is DENIED — the denial is fed back to the
/// model as an error `tool_result` and the agent CONTINUES without that tool
/// (deny-and-continue), never parking the run `waiting` forever (no orphan
/// pending). Read-only / trusted built-ins still auto-run (the approval policy
/// returns `Auto` and never reaches the gate); only calls that would require
/// human approval are dropped. This is the unattended safe-default: a background
/// agent never silently auto-approves a mutating/external tool.
struct UnattendedDenyGate;

#[async_trait]
impl HumanGate for UnattendedDenyGate {
    async fn request(&self, _run_id: Uuid, _ask: GateAsk) -> Result<GateOutcome, AppError> {
        Ok(GateOutcome::Decided(ReviewDecision::Denied))
    }
}

/// The SubAgent background driver (ITEM-7 / ITEM-9).
///
/// Wires the FULL durable run lifecycle end-to-end — a `workflow_runs` row →
/// `running` + heartbeat → terminal `completed` + `final_output_json` →
/// owner-scoped `SyncEntity::WorkflowRun` notify (all via `spawn_background_run`)
/// → an ITEM-9 `notification` inbox row + `SyncEntity::Notification` — and runs a
/// REAL detached `AgentCore` turn for the actual work (via the shared
/// `build_detached_agent_core` builder, the same one the proven workflow
/// `kind: agent` host uses). The run-row + notification + sync scaffolding is
/// unchanged from the backbone; only the executor body now drives a real turn.
async fn execute_subagent_run(
    pool: &PgPool,
    run_id: Uuid,
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    model_id: Uuid,
    handle: Arc<registry::RunHandle>,
    system: &str,
    task: &str,
) -> BackgroundOutcome {
    let outcome = match drive_subagent_turn(
        pool,
        run_id,
        user_id,
        conversation_id,
        model_id,
        handle,
        system,
        task,
    )
    .await
    {
        Ok(o) => o,
        Err(e) => BackgroundOutcome::Failed {
            error: format!("background sub-agent: {e}"),
        },
    };

    // ── ITEM-9: results-land-when-done. On COMPLETION post a durable inbox row so
    //    an away user is told, and it live-pushes via the installed
    //    `SyncEntity::Notification` emitter. (`spawn_background_run` separately
    //    emits `SyncEntity::WorkflowRun` on the terminal transition, incl. for a
    //    failed/cancelled run.) A notify failure must NOT fail the run — log and
    //    continue, exactly like the scheduler's first-producer path. ──
    if let BackgroundOutcome::Completed { final_output } = &outcome {
        let final_text = final_output
            .as_ref()
            .and_then(|v| v.get("final_text"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .unwrap_or("")
            .to_string();

        let summary = if final_text.is_empty() {
            "Background task finished.".to_string()
        } else {
            final_text.chars().take(500).collect::<String>()
        };
        post_completion_notification(pool, user_id, run_id, conversation_id, summary).await;

        // ── Push-to-resume (kill the poll loop): a conversation-bound sub-agent
        //    that produced a result re-engages the chat agent loop with that
        //    result. Detached (`tokio::spawn`) so it does NOT block the runner's
        //    terminal transition; owns every capture (DEC-6). Gated to
        //    conversation-bound + non-empty result (the subagent-only gate is
        //    structural — this driver is never the sandbox path). A resume failure
        //    must NEVER fail the already-completed run — log + continue (DEC-7). ──
        // Deploy-level kill switch (`background_mcp.resume_enabled`, default true):
        // an operator can turn auto-resume OFF entirely — the result still lives in
        // the run row + inbox; only the automatic re-engagement is suppressed.
        let resume_enabled = super::resume::resume_enabled_from_config();
        if super::resume::should_resume(resume_enabled, conversation_id, &final_text) {
            let cid = conversation_id.expect("should_resume guarantees Some");
            let resume = super::resume::ResumeRequest {
                pool: pool.clone(),
                user_id,
                conversation_id: cid,
                run_id,
                model_id,
                task: task.to_string(),
                final_text,
            };
            tokio::spawn(async move {
                if let Err(e) = super::resume::resume_conversation_with_result(resume).await {
                    tracing::warn!(
                        "background_mcp: push-to-resume failed for run {run_id} \
                         (conversation {cid}); result remains in the run row + inbox: {e:?}"
                    );
                }
            });
        }
    }

    outcome
}

/// ITEM-9/ITEM-13: post the durable "background task finished" inbox row on a
/// completed background run, shared by EVERY background kind (sub-agent / sandbox
/// exec). It live-pushes via the installed `SyncEntity::Notification` emitter;
/// `spawn_background_run` separately emits `SyncEntity::WorkflowRun` on the
/// terminal transition. A notify failure must NOT fail the run — log + continue
/// (exactly like the scheduler's first-producer path).
async fn post_completion_notification(
    pool: &PgPool,
    user_id: Uuid,
    run_id: Uuid,
    conversation_id: Option<Uuid>,
    summary: String,
) {
    let mut payload = serde_json::Map::new();
    payload.insert("workflow_run_id".into(), json!(run_id));
    if let Some(cid) = conversation_id {
        payload.insert("conversation_id".into(), json!(cid));
    }
    let notif = NewNotification::new(user_id, "background_run_result", "Background task finished")
        .body(summary)
        .payload(Value::Object(payload));
    if let Err(e) = create_and_emit(pool, notif).await {
        tracing::warn!(
            "background_mcp: failed to create completion notification for run {run_id}: {e:?}"
        );
    }
}

/// Copyable literal-JSON example for a `spec.flavor` refusal.
const SANDBOX_FLAVOR_EXAMPLE: &str =
    r#"{"kind":"sandbox_exec","spec":{"command":"python hello.py","flavor":"minimal"}}"#;

/// Resolve `spec.flavor`, ENFORCING the `["minimal","full"]` enum the tool schema
/// advertises.
///
/// Before this, ANY non-empty string was accepted here and flowed through
/// `execute_command_detached` → `ensure_rootfs_ready` → `install_version` →
/// `format!("ziee-sandbox-rootfs-{arch}-{flavor}.{ext}")` → a live GitHub
/// Releases request. This runs before the `workflow_runs` row is created, so a
/// bad flavor costs nothing and constructs no URL.
///
/// A thin adapter over the SHARED `code_sandbox::resolve_tool_flavor` — the
/// argument name and the example are the only things that differ from the chat
/// `execute_command` entry point, and they are the only things that live here.
/// (The two were briefly duplicated resolvers; sharing only the predicate is how
/// two copies of the same rule drift, and the default `"minimal"` had reached
/// three independent literals.)
fn resolve_spec_flavor(spec: &Value) -> Result<String, AppError> {
    crate::modules::code_sandbox::resolve_tool_flavor(
        spec.get("flavor"),
        "spec.flavor",
        SANDBOX_FLAVOR_EXAMPLE,
    )
}

/// `spawn_background{kind:'sandbox_exec'}` — launch a detached
/// [`JobKind::SandboxExec`] shell command in THIS conversation's code sandbox
/// (ITEM-11/12/13, Group C).
///
/// The sandbox workspace is per-conversation, so a conversation context is
/// REQUIRED (unlike a sub-agent, which only needs a model). Ownership is verified
/// up front (fail fast — no doomed run row for a foreign conversation) AND again
/// inside `execute_command_detached`'s `build_context` (defense-in-depth). No
/// model is needed — this just runs a command.
async fn spawn_sandbox_exec(
    pool: &PgPool,
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    kind: &'static KindContract,
    spec: Value,
) -> Result<Value, AppError> {
    let conversation_id = conversation_id.ok_or_else(|| {
        AppError::bad_request(
            "BACKGROUND_NO_CONVERSATION",
            "background sandbox exec requires a conversation context (the sandbox workspace is per-conversation)",
        )
    })?;
    let command = require_spec_field(kind, &spec)?;
    let flavor = resolve_spec_flavor(&spec)?;

    // Fail fast on a foreign / missing conversation (owner-scoped 404); the run
    // row is only created for a conversation the caller actually owns.
    crate::core::Repos
        .chat
        .core
        .get_conversation(conversation_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("conversation not found"))?;

    let request = CreateBackgroundRun {
        job_kind: kind.job_kind,
        conversation_id: Some(conversation_id),
        user_id,
        model_id: None,
        sandbox_flavor: Some(flavor.clone()),
        invocation_source: "conversation".into(),
        inputs_json: spec.clone(),
    };

    // Fire-and-forget: the driver runs OUTSIDE any per-conversation single-flight
    // lock, so the foreground chat stays interactive while the command runs.
    let run_id = runner::spawn_background_run(pool, request, move |task_pool, run_id, handle| async move {
        execute_sandbox_run(&task_pool, run_id, user_id, conversation_id, handle, command, flavor).await
    })
    .await?;

    Ok(json!({
        "run_id": run_id,
        "kind": kind.job_kind.as_str(),
        "status": "pending",
        "note": "Background sandbox command started. END your turn — its completion drops a notification in the inbox; read its output with collect_result on demand."
    }))
}

/// The SandboxExec background driver (ITEM-11/12/13).
///
/// Reuses the SAME durable run lifecycle scaffolding as the sub-agent driver —
/// the `workflow_runs` row → `running` + heartbeat → terminal `completed` +
/// `final_output_json` → owner-scoped `SyncEntity::WorkflowRun` notify (all via
/// `spawn_background_run`) → the shared `post_completion_notification` inbox row.
/// The ONLY kind-specific part is the body: it runs the command through the
/// UNCHANGED `code_sandbox` execute path (`execute_command_detached`), so every
/// hardening guard (`--clearenv`, seccomp, cgroup, PID-ns, prlimit caps, the
/// per-command wall-clock cap) is preserved verbatim.
///
/// An owner cancel (via `check_status`/conversation-delete) is raced against the
/// command: when it wins, dropping the exec future triggers the sandbox child's
/// `kill_on_drop(true)` SIGKILL — a real cancel of the running command. (The
/// cgroup-kill grandchild reap + idle reaper are the SDK ITEM-30/31 follow-up.)
async fn execute_sandbox_run(
    pool: &PgPool,
    run_id: Uuid,
    user_id: Uuid,
    conversation_id: Uuid,
    handle: Arc<registry::RunHandle>,
    command: String,
    flavor: String,
) -> BackgroundOutcome {
    let exec_fut = crate::modules::code_sandbox::handlers::execute_command_detached(
        conversation_id,
        user_id,
        &command,
        &flavor,
    );
    tokio::pin!(exec_fut);

    let outcome = tokio::select! {
        // Owner cancel landed first: drop the exec future → kill_on_drop reaps the
        // sandbox child. Report Cancelled (no final_output, no completion inbox).
        _ = handle.await_cancel() => BackgroundOutcome::Cancelled,
        r = &mut exec_fut => match r {
            Ok(exec) => BackgroundOutcome::Completed {
                final_output: Some(build_sandbox_final_output(&command, &flavor, &exec)),
            },
            // The SANDBOX itself failed (not-initialized / workspace / ownership) —
            // distinct from a command that ran but exited nonzero (that's a
            // Completed run whose final_output carries the exit_code).
            Err(e) => BackgroundOutcome::Failed {
                error: format!("background sandbox exec: {e}"),
            },
        },
    };

    if let BackgroundOutcome::Completed { final_output } = &outcome {
        let summary = final_output
            .as_ref()
            .map(sandbox_notification_summary)
            .unwrap_or_else(|| "Background command finished.".to_string());
        post_completion_notification(pool, user_id, run_id, Some(conversation_id), summary).await;
    }

    outcome
}

/// Project the `execute_command` result JSON (`{stdout, stderr, exit_code,
/// timed_out, duration_ms, *_truncated, …}`) into the stable, collectible
/// `final_output` envelope `collect_result` pages. Pure → unit-tested rootfs-free.
/// A nonzero `exit_code` is DATA (the run still `completed`); only a sandbox-level
/// error maps to a failed run.
fn build_sandbox_final_output(command: &str, flavor: &str, exec: &Value) -> Value {
    let timed_out = exec.get("timed_out").and_then(|v| v.as_bool()).unwrap_or(false);
    let status = if timed_out { "timed_out" } else { "completed" };
    json!({
        "executor": "code-sandbox",
        "kind": "sandbox_exec",
        "status": status,
        "command": command,
        "flavor": flavor,
        "exit_code": exec.get("exit_code").cloned().unwrap_or(Value::Null),
        "timed_out": timed_out,
        "stdout": exec.get("stdout").cloned().unwrap_or(Value::Null),
        "stderr": exec.get("stderr").cloned().unwrap_or(Value::Null),
        "duration_ms": exec.get("duration_ms").cloned().unwrap_or(Value::Null),
        "stdout_truncated": exec.get("stdout_truncated").and_then(|v| v.as_bool()).unwrap_or(false),
        "stderr_truncated": exec.get("stderr_truncated").and_then(|v| v.as_bool()).unwrap_or(false),
    })
}

/// Human-readable completion summary for the notification inbox row. Pure →
/// unit-tested rootfs-free.
fn sandbox_notification_summary(final_output: &Value) -> String {
    let head = |key: &str| {
        final_output
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.chars().take(200).collect::<String>())
    };
    if final_output.get("timed_out").and_then(|v| v.as_bool()).unwrap_or(false) {
        return "Background command timed out.".to_string();
    }
    match final_output.get("exit_code").and_then(|v| v.as_i64()) {
        Some(0) => head("stdout")
            .map(|o| format!("Command succeeded: {o}"))
            .unwrap_or_else(|| "Background command finished (exit 0).".to_string()),
        Some(code) => {
            let detail = head("stderr")
                .or_else(|| head("stdout"))
                .map(|d| format!(": {d}"))
                .unwrap_or_default();
            format!("Background command exited with code {code}{detail}")
        }
        None => "Background command finished.".to_string(),
    }
}

/// Build + run ONE detached `AgentCore` turn on the run's model, collecting the
/// final assistant text into a structured `final_output`. Errors (model resolve,
/// loop failure) bubble up so the caller maps them to `BackgroundOutcome::Failed`.
async fn drive_subagent_turn(
    pool: &PgPool,
    run_id: Uuid,
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    model_id: Uuid,
    handle: Arc<registry::RunHandle>,
    system: &str,
    task: &str,
) -> Result<BackgroundOutcome, AppError> {
    // Resolve the run's model → provider (under the owner's RBAC) → model client.
    let (provider, model_name, ..) = create_provider_from_model_id(model_id, user_id).await?;
    let model_client: Arc<dyn ModelClient> = Arc::new(ProviderModelClient::new(provider));

    // Admin agent policy → per-RUN budget (DEC-6: reuse default_max_steps +
    // per_run_token_cap for a background run). Sane defaults if the row is
    // unreadable. `settings` also feeds the shared builder's reviewer / sandbox /
    // fan-out limits below.
    let settings = crate::core::Repos.agent.get_admin_settings().await.ok();
    let (max_steps, per_run_cap) = settings
        .as_ref()
        .map(|s| (s.default_max_steps.max(1) as u32, s.per_run_token_cap.max(0) as u64))
        .unwrap_or((30, 1_000_000));
    let budget = Budget::new(max_steps, per_run_cap, per_run_cap);

    // A detached background sub-agent is UNATTENDED (DEC-117): a quiet sink + a
    // deny-and-continue gate (never parks `waiting`). Everything else (transcript,
    // tools, approval policy, reviewer, compaction, task list) is built by the
    // shared detached-core builder, identical to the workflow `kind: agent` host.
    let core = build_detached_agent_core(DetachedAgentCoreArgs {
        pool: pool.clone(),
        user_id,
        conversation_id,
        run_id,
        model_id,
        model_name: model_name.clone(),
        model_client,
        cancel: handle.clone(),
        sink: Arc::new(BackgroundEventSink),
        gate: Arc::new(UnattendedDenyGate),
        classifications: Arc::new(Mutex::new(HashMap::new())),
        settings,
        budget,
        // ITEM-25 / DEC-79: THIS is the run the `background/runs/{id}/notes` REST
        // steers — wire the durable note-queue reader so queued notes reach the
        // loop as `[steering]` messages on its next iteration.
        steer: Some(Arc::new(RunNoteSteerPort { pool: pool.clone() })),
    })
    .await;

    // Start fresh from the spec (no resume in this tranche): a `NewMessage(task)`
    // seed + the optional `system` framing. Empty tool scope — a minimal reasoning
    // turn; spec-driven `servers` is a follow-up. The unattended gate is the
    // backstop if the model ever requests an approval-needing tool.
    let system_blocks: Vec<ContentBlock> = if system.trim().is_empty() {
        Vec::new()
    } else {
        vec![ContentBlock::Text { text: system.to_string() }]
    };
    let req = AgentTurnRequest {
        run_id,
        user_id,
        seed: TurnSeed::NewMessage(ChatMessage::user(task.to_string())),
        system: system_blocks,
        tool_scope: ToolScope {
            servers: Vec::new(),
            // ITEM-2 / DEC-2: a detached background sub-agent run stays
            // `allow_delegate: false` unconditionally (NOT gated on the admin
            // `delegate_enabled`) — a spawned sub-agent must never spawn its own
            // sub-agents (the depth cap). Only the top-level chat/workflow hosts
            // honor `delegate_enabled`.
            allow_delegate: false,
        },
        start_iteration: 1,
        inputs: Value::Null,
    };

    // Bridge the owner-cancel handle into the crate's cooperative token so a
    // `check_status`-driven cancel (or a conversation delete) stops the turn.
    let cancel_token = CancelToken::new();
    let bridge = {
        let ct = cancel_token.clone();
        let h = handle.clone();
        tokio::spawn(async move {
            h.await_cancel().await;
            ct.cancel();
        })
    };
    let run_result = core.run(req, cancel_token).await;
    bridge.abort();

    let events = run_result?;

    // Owner-cancel → the loop ends `Halted` with no gate.
    let last_stop = events.iter().rev().find_map(|e| match e {
        AgentEvent::Stopped(r) => Some(*r),
        _ => None,
    });
    if last_stop == Some(StopReason::Halted) {
        return Ok(BackgroundOutcome::Cancelled);
    }

    // The final answer is the loop's last assistant text.
    let final_text = events
        .iter()
        .rev()
        .find_map(|e| match e {
            AgentEvent::Message(msg) if msg.role == Role::Assistant => {
                let text: String = msg
                    .content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("");
                if text.is_empty() { None } else { Some(text) }
            }
            _ => None,
        })
        .unwrap_or_default();

    let tokens: u64 = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::Usage(u) => Some(u.total_tokens),
            _ => None,
        })
        .sum();

    let final_output = json!({
        "executor": "agent-core",
        "status": "completed",
        "final_text": final_text,
        "tokens_used": tokens,
        "spec": { "system": system, "task": task },
    });

    Ok(BackgroundOutcome::Completed {
        final_output: Some(final_output),
    })
}

/// `check_status{run_id}` — cheap owner-scoped read of the run's state +
/// progress. A foreign / missing id — or a classic workflow run (background-only
/// surface) — → 404 (never leaks another user's run, never reads a workflow run
/// through the background surface).
async fn check_status(pool: &PgPool, user_id: Uuid, args: &Value) -> Result<Value, AppError> {
    let run_id = parse_run_id(args)?;
    let run = repository::find_background_run_for_owner(pool, run_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("background run not found"))?;

    let terminal = WorkflowRunStatus::from_db_str(&run.status).is_some_and(|s| s.is_terminal());
    Ok(json!({
        "run_id": run.id,
        "kind": run.job_kind,
        "status": run.status,
        "terminal": terminal,
        "current_step": run.current_step,
        "error_message": run.error_message,
        "progress": run.step_progress_json,
        "updated_at": run.updated_at,
    }))
}

/// `collect_result{run_id, offset?, max_chars?}` — idempotent, paged owner-scoped
/// read of `final_output_json`. Not-yet-terminal → returns the current status
/// (the model should retry). A foreign / missing id — or a classic workflow run
/// (background-only surface) — → 404.
async fn collect_result(pool: &PgPool, user_id: Uuid, args: &Value) -> Result<Value, AppError> {
    let run_id = parse_run_id(args)?;
    let run = repository::find_background_run_for_owner(pool, run_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("background run not found"))?;

    let status = WorkflowRunStatus::from_db_str(&run.status);
    let terminal = status.is_some_and(|s| s.is_terminal());
    if !terminal {
        return Ok(json!({
            "run_id": run.id,
            "status": run.status,
            "complete": false,
            "note": "Run is not finished yet — poll check_status or retry collect_result.",
        }));
    }

    // Terminal but no output (e.g. a failed run): report the terminal status +
    // error rather than an empty result.
    let Some(output) = run.final_output_json.clone() else {
        return Ok(json!({
            "run_id": run.id,
            "status": run.status,
            "complete": true,
            "final_output": Value::Null,
            "error_message": run.error_message,
        }));
    };

    // Page over the serialized output (mirrors tool_result_mcp's char paging).
    let serialized = serde_json::to_string(&output).unwrap_or_default();
    let chars: Vec<char> = serialized.chars().collect();
    let total = chars.len();
    let offset = args
        .get("offset")
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
        .min(total as u64) as usize;
    let max_chars = args
        .get("max_chars")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(COLLECT_DEFAULT_MAX_CHARS)
        .clamp(1, COLLECT_MAX_CHARS_CAP);
    let end = (offset + max_chars).min(total);
    let chunk: String = chars[offset..end].iter().collect();
    let next_offset = if end < total { Some(end) } else { None };

    Ok(json!({
        "run_id": run.id,
        "status": run.status,
        "complete": true,
        "final_output_chunk": chunk,
        "offset": offset,
        "next_offset": next_offset,
        "total_chars": total,
        "truncated": next_offset.is_some(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // TEST (DEC-33 / built-in-MCP §11): the WRITE tool is NOT approval-bypassed;
    // the two READ tools ARE. This is the per-tool half of the approval contract
    // consumed by the `is_background` arm in `mcp/chat_extension/mcp.rs`.
    #[test]
    fn spawn_needs_approval_reads_do_not() {
        assert!(
            background_call_needs_approval("spawn_background"),
            "spawn_background LAUNCHES a detached agent → must require approval"
        );
        assert!(!background_call_needs_approval("check_status"));
        assert!(!background_call_needs_approval("collect_result"));
        // Fail-safe: an unrecognized tool requires approval.
        assert!(background_call_needs_approval("something_else"));
    }

    // TEST (DEC-117 — unattended safe default): a detached background sub-agent
    // has NO human to answer a prompt, so the gate must DENY (deny-and-continue)
    // any approval-needing call rather than `Suspend` the run `waiting` forever.
    // This is the security-critical wiring: with `GateOutcome::Decided(Denied)`
    // the core loop feeds an error result back and the agent proceeds without the
    // tool (see `core.rs`'s `GateOutcome::Decided(_) => Act::Deny`), and never
    // emits `GateOpened`, so no orphan `waiting` row is left behind.
    #[tokio::test]
    async fn unattended_gate_denies_never_suspends() {
        use agent_core::ToolCall;

        let gate = UnattendedDenyGate;
        let ask = GateAsk {
            call: ToolCall {
                id: "tu_1".into(),
                server: Some("some_server".into()),
                name: "do_dangerous_thing".into(),
                input: json!({}),
            },
            reason: "tool call requires approval".into(),
        };
        let outcome = gate
            .request(Uuid::new_v4(), ask)
            .await
            .expect("unattended gate never errors");
        match outcome {
            GateOutcome::Decided(ReviewDecision::Denied) => {}
            other => panic!(
                "a background (unattended) gate must Deny (deny-and-continue), never \
                 Suspend/Approve; got {other:?}"
            ),
        }
    }

    // TEST: the trio is advertised with the required-arg shapes.
    #[test]
    fn tool_list_advertises_the_trio() {
        let list = tool_list();
        let tools = list["tools"].as_array().expect("tools array");
        let names: Vec<&str> = tools
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        assert!(names.contains(&"spawn_background"));
        assert!(names.contains(&"check_status"));
        assert!(names.contains(&"collect_result"));
        assert_eq!(names.len(), 3, "exactly the trio, no accidental extras");

        // spawn_background requires `spec`; the reads require `run_id`.
        let spawn = tools.iter().find(|t| t["name"] == "spawn_background").unwrap();
        assert_eq!(spawn["inputSchema"]["required"][0], "spec");
        for read in ["check_status", "collect_result"] {
            let t = tools.iter().find(|t| t["name"] == read).unwrap();
            assert_eq!(t["inputSchema"]["required"][0], "run_id");
        }
    }

    // TEST-1 (push-to-resume): the tool descriptions must NOT re-grow the polling
    // anti-pattern. `collect_result` must not instruct the model to poll
    // `check_status` until complete, and `spawn_background` must tell the model to
    // END its turn + that a subagent auto-re-engages the conversation. A regression
    // that reintroduces the poll loop silently defeats the whole feature.
    #[test]
    fn descriptions_drop_polling_and_teach_push_resume() {
        let list = tool_list();
        let tools = list["tools"].as_array().unwrap();
        let desc = |name: &str| -> String {
            tools
                .iter()
                .find(|t| t["name"] == name)
                .and_then(|t| t["description"].as_str())
                .unwrap_or("")
                .to_string()
        };

        let collect = desc("collect_result").to_lowercase();
        assert!(
            !collect.contains("poll `check_status`") && !collect.contains("poll check_status"),
            "collect_result must not instruct polling check_status: {collect}"
        );
        assert!(
            !collect.contains("until it is complete"),
            "collect_result must drop the 'poll until complete' loop: {collect}"
        );

        let spawn = desc("spawn_background").to_lowercase();
        assert!(
            spawn.contains("end your turn"),
            "spawn_background must tell the model to end its turn: {spawn}"
        );
        assert!(
            spawn.contains("re-engaged") || spawn.contains("auto-resume") || spawn.contains("automatically"),
            "spawn_background must describe automatic re-engagement on completion: {spawn}"
        );
        assert!(
            spawn.contains("do not poll") || spawn.contains("do not") && spawn.contains("poll"),
            "spawn_background must warn against polling: {spawn}"
        );
    }

    // TEST (Group C): the `kind` enum now advertises BOTH background kinds so the
    // model can route a command into the sandbox. A regression that dropped
    // `sandbox_exec` from the schema silently hides the whole feature.
    #[test]
    fn spawn_kind_enum_advertises_sandbox_exec() {
        let list = tool_list();
        let tools = list["tools"].as_array().unwrap();
        let spawn = tools.iter().find(|t| t["name"] == "spawn_background").unwrap();
        let kinds = spawn["inputSchema"]["properties"]["kind"]["enum"]
            .as_array()
            .expect("kind enum");
        let kinds: Vec<&str> = kinds.iter().filter_map(|k| k.as_str()).collect();
        assert!(kinds.contains(&"subagent"), "subagent kind still advertised");
        assert!(kinds.contains(&"sandbox_exec"), "sandbox_exec kind advertised");
    }

    // TEST (rootfs-free executor wiring — ITEM-11/13): `build_sandbox_final_output`
    // projects the `execute_command` result JSON into the stable, collectible
    // `final_output` envelope. This is the serialization the `collect_result` read
    // path pages, proven WITHOUT a live bwrap sandbox (mirrors how the subagent
    // executor's wiring is provable without a live model).
    #[test]
    fn build_sandbox_final_output_projects_exec_result() {
        // The shape `ziee_sandbox::tools::execute::execute_command` returns.
        let exec = json!({
            "stdout": "hi\n",
            "stderr": "",
            "exit_code": 0,
            "timed_out": false,
            "duration_ms": 12,
            "stdout_truncated": false,
            "stderr_truncated": false,
            "flavor": "minimal",
        });
        let out = build_sandbox_final_output("echo hi", "minimal", &exec);
        assert_eq!(out["executor"], "code-sandbox");
        assert_eq!(out["kind"], "sandbox_exec");
        assert_eq!(out["status"], "completed");
        assert_eq!(out["command"], "echo hi");
        assert_eq!(out["flavor"], "minimal");
        assert_eq!(out["exit_code"], json!(0));
        assert_eq!(out["stdout"], "hi\n");
        assert_eq!(out["timed_out"], json!(false));
    }

    // TEST: a NONZERO exit code is DATA, not a run failure — the command RAN, so
    // the run still `completed`; the exit_code is carried in the envelope for the
    // model to read. (A sandbox-level error is what maps to a Failed run — that
    // path is the `Err(e)` arm in `execute_sandbox_run`, unreachable here.)
    #[test]
    fn nonzero_exit_is_completed_with_exit_code_preserved() {
        let exec = json!({
            "stdout": "", "stderr": "boom", "exit_code": 2,
            "timed_out": false, "duration_ms": 5,
            "stdout_truncated": false, "stderr_truncated": false,
        });
        let out = build_sandbox_final_output("false", "minimal", &exec);
        assert_eq!(out["status"], "completed", "the command ran → completed run");
        assert_eq!(out["exit_code"], json!(2));
        assert_eq!(out["stderr"], "boom");
    }

    // TEST: a timed-out command is reported DISTINCTLY (DEC-74) — status
    // `timed_out` + `timed_out:true` in the envelope, and the notification says so.
    #[test]
    fn timed_out_command_is_reported_distinctly() {
        let exec = json!({
            "stdout": "partial", "stderr": "", "exit_code": Value::Null,
            "timed_out": true, "duration_ms": 600000,
            "stdout_truncated": true, "stderr_truncated": false,
        });
        let out = build_sandbox_final_output("sleep 999", "minimal", &exec);
        assert_eq!(out["status"], "timed_out");
        assert_eq!(out["timed_out"], json!(true));
        assert_eq!(out["stdout_truncated"], json!(true));
        assert_eq!(
            sandbox_notification_summary(&out),
            "Background command timed out."
        );
    }

    // TEST: the notification summary derives a legible one-liner per exit class.
    #[test]
    fn sandbox_notification_summary_by_exit_class() {
        let ok = build_sandbox_final_output(
            "echo done",
            "minimal",
            &json!({ "stdout": "done\n", "stderr": "", "exit_code": 0, "timed_out": false }),
        );
        assert!(
            sandbox_notification_summary(&ok).starts_with("Command succeeded:"),
            "success summary carries a stdout head: {}",
            sandbox_notification_summary(&ok)
        );

        let failed = build_sandbox_final_output(
            "false",
            "minimal",
            &json!({ "stdout": "", "stderr": "nope", "exit_code": 1, "timed_out": false }),
        );
        let s = sandbox_notification_summary(&failed);
        assert!(s.contains("exited with code 1"), "failure summary names the code: {s}");
        assert!(s.contains("nope"), "failure summary carries the stderr head: {s}");
    }
}

#[cfg(test)]
mod stringified_arg_tests {
    use super::*;
    use crate::common::tool_args::conformance::{assert_arg_conformance, ArgSite};
    use crate::common::tool_args::ArgShape;
    use serde_json::json;

    /// A double-encoded `spec` used to produce the LIE "spec.task must be a
    /// non-empty string" — `task` was supplied, just one level too deep.
    /// (TEST-29)
    #[test]
    fn background_spec_decodes_instead_of_blaming_a_missing_task() {
        let spec = decode_spec_arg(&json!({ "spec": r#"{"task":"Summarise it"}"# }))
            .expect("a stringified spec must decode");
        assert_eq!(spec["task"], json!("Summarise it"));

        let err = decode_spec_arg(&json!({ "spec": "not json {" })).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("spec") && msg.contains("JSON object"), "got: {msg}");
        assert!(msg.contains(BACKGROUND_SPEC_EXAMPLE), "must show a spec to copy: {msg}");
    }

    /// The shared conformance battery, applied to `spawn_background.spec`.
    /// (TEST-41)
    #[test]
    fn background_spec_passes_the_shared_argument_conformance_battery() {
        assert_arg_conformance(ArgSite {
            site: "spawn_background.spec",
            arg: "spec",
            shape: ArgShape::Object,
            canonical: json!({ "task": "Summarise it" }),
            example: BACKGROUND_SPEC_EXAMPLE,
            absent_yields: None,
            extract: |args: serde_json::Value| match decode_spec_arg(&args) {
                Ok(v) => Ok(Some(v)),
                // `spec` is REQUIRED here, so "absent" is its own error. Map it
                // back to the battery's absent contract rather than weakening
                // the battery for every other site.
                Err(e) if format!("{e}").contains("was not supplied") => Ok(None),
                Err(e) => Err(format!("{e}")),
            },
        });
    }
}

#[cfg(test)]
mod argument_contract_tests {
    use super::*;
    use serde_json::json;

    // =====================================================================
    // The `spawn_background` ARGUMENT CONTRACT (tool-argument-contracts)
    // =====================================================================

    /// Drive `parse_spawn_args` and report either the resolved kind name + spec,
    /// or the model-facing refusal text.
    fn parse(args: serde_json::Value) -> Result<(&'static str, Value), String> {
        parse_spawn_args(&args)
            .map(|a| (a.kind.name, a.spec))
            .map_err(|e| e.to_string())
    }

    /// Every literal-JSON example this module can hand a model.
    fn all_examples() -> Vec<&'static str> {
        let mut v: Vec<&'static str> = KIND_CONTRACTS.iter().map(|k| k.example).collect();
        v.push(SANDBOX_FLAVOR_EXAMPLE);
        v.push(BACKGROUND_SPEC_EXAMPLE);
        v
    }

    /// INV-1's three elements, asserted the same way
    /// `common::tool_args::conformance::assert_actionable` asserts them for a
    /// SHAPE refusal: the message must name the argument, say what is expected,
    /// and carry a literal-JSON example the model can copy.
    ///
    /// Asserting only "it errored" is the blind spot in a different costume — an
    /// error the model cannot act on is still a failure.
    fn assert_actionable(label: &str, msg: &str, arg: &str) {
        assert!(
            msg.contains(arg),
            "[{label}] the refusal must NAME the argument `{arg}`: {msg}"
        );
        assert!(
            msg.contains("must be one of")
                || msg.contains("is required")
                || msg.contains("does not accept")
                || msg.contains("Supply it once"),
            "[{label}] the refusal must say what is EXPECTED: {msg}"
        );
        // The example must be one of the REAL examples this module hands out,
        // and it must PARSE. Matching the literal "Example: {" (as an earlier
        // revision did) would pass for an empty, malformed, or wrong-kind
        // example — which is exactly the `label` defect one level down, and it
        // is why the shared `tool_args::conformance::assert_actionable` matches
        // the site's actual example string rather than a marker.
        let carried = msg
            .split_once("Example: ")
            .map(|(_, tail)| tail.trim().to_string())
            .unwrap_or_else(|| {
                panic!("[{label}] the refusal must carry a literal-JSON EXAMPLE to copy: {msg}")
            });
        assert!(
            all_examples().contains(&carried.as_str()),
            "[{label}] the refusal carried an example this module does not define \
             ({carried:?}); it must hand the model one of {:?}",
            all_examples()
        );
        serde_json::from_str::<Value>(&carried)
            .unwrap_or_else(|e| panic!("[{label}] the carried example must be valid JSON: {e}"));
    }

    // TEST-1: a `kind` nested inside `spec` RESOLVES the job kind instead of
    // being dropped in favour of the `subagent` default — the reported defect.
    // The happy-path counterparts (top-level kind, and no kind at all) are in
    // the same test, so this can never pass because resolution broke entirely.
    #[test]
    fn nested_spec_kind_resolves_the_job_kind() {
        // The reported payload: `kind` supplied ONLY inside `spec`.
        let (kind, _) = parse(json!({
            "spec": { "kind": "sandbox_exec", "command": "python hello.py" }
        }))
        .expect("a nested `kind` must be honoured, not dropped");
        assert_eq!(kind, "sandbox_exec", "the nested `kind` resolves the job kind");

        // HAPPY-PATH COUNTERPARTS — the pre-existing behaviour is unchanged.
        let (kind, _) = parse(json!({ "kind": "subagent", "spec": { "task": "x" } })).unwrap();
        assert_eq!(kind, "subagent", "a top-level `kind` still wins");
        let (kind, _) =
            parse(json!({ "kind": "sandbox_exec", "spec": { "command": "ls" } })).unwrap();
        assert_eq!(kind, "sandbox_exec", "a top-level sandbox kind still works");
        let (kind, _) = parse(json!({ "spec": { "task": "x" } })).unwrap();
        assert_eq!(kind, DEFAULT_KIND, "an ABSENT `kind` still falls to the default");
    }

    // TEST-2: two `kind`s that DISAGREE are refused naming both, never resolved
    // by preference. An AGREEING pair is not a contradiction and is accepted.
    #[test]
    fn conflicting_kinds_are_refused_and_agreeing_ones_accepted() {
        let msg = parse(json!({
            "kind": "subagent",
            "spec": { "kind": "sandbox_exec", "command": "ls" }
        }))
        .expect_err("a contradiction must not be silently resolved");
        assert!(msg.contains("subagent"), "names the top-level value: {msg}");
        assert!(msg.contains("sandbox_exec"), "names the nested value: {msg}");
        assert_actionable("kind-conflict", &msg, "kind");

        // HAPPY-PATH COUNTERPART: the same value in both places is fine.
        let (kind, spec) = parse(json!({
            "kind": "sandbox_exec",
            "spec": { "kind": "sandbox_exec", "command": "ls" }
        }))
        .expect("an AGREEING pair is not a contradiction");
        assert_eq!(kind, "sandbox_exec");
        assert!(spec.get("kind").is_none(), "the consumed `kind` is removed");
    }

    // TEST-3: the consumed `kind` is stripped from the spec that gets persisted
    // as `inputs_json`, and nothing else is disturbed.
    #[test]
    fn consumed_kind_is_stripped_and_other_keys_survive_verbatim() {
        let (_, spec) = parse(json!({
            "spec": { "kind": "sandbox_exec", "command": "ls -la", "flavor": "full" }
        }))
        .unwrap();
        assert!(spec.get("kind").is_none(), "`kind` is consumed, not persisted: {spec}");
        assert_eq!(spec["command"], json!("ls -la"), "every other key survives");
        assert_eq!(spec["flavor"], json!("full"), "…byte-identically");

        // A spec that never carried `kind` is returned unchanged.
        let original = json!({ "task": "x", "system": "be terse" });
        let (_, spec) = parse(json!({ "kind": "subagent", "spec": original.clone() })).unwrap();
        assert_eq!(spec, original, "a spec without `kind` is untouched");
    }

    // TEST-4: an unknown `kind` VALUE is refused with the valid kinds + an
    // example — from either location. The old message was the bare text
    // "unknown background kind '<x>'": no list, no example.
    #[test]
    fn unknown_kind_value_is_refused_actionably_from_either_location() {
        for args in [
            json!({ "kind": "zee-workflow", "spec": { "task": "x" } }),
            json!({ "spec": { "kind": "zee-workflow", "task": "x" } }),
        ] {
            let msg = parse(args.clone()).expect_err("an unknown kind must be refused");
            assert!(msg.contains("zee-workflow"), "echoes what was received: {msg}");
            for k in KIND_CONTRACTS {
                assert!(msg.contains(k.name), "lists the valid kind `{}`: {msg}", k.name);
            }
            assert_actionable("unknown-kind", &msg, "kind");
        }

        // A non-string `kind` is refused too, never defaulted (INV-2).
        let msg = parse(json!({ "kind": 42, "spec": { "task": "x" } })).unwrap_err();
        assert!(msg.contains("a number"), "names what arrived: {msg}");
        assert_actionable("non-string-kind", &msg, "kind");
    }

    // TEST-5: `spec` is held to the keys the schema advertises — and the schema
    // now SAYS so, which is the half that makes the enforcement honest.
    #[test]
    fn unadvertised_spec_keys_are_refused_and_advertised_ones_accepted() {
        let msg = parse(json!({ "kind": "subagent", "spec": { "task": "x", "priority": "high" } }))
            .expect_err("an unadvertised key must be refused, not ignored");
        assert!(msg.contains("priority"), "names the offending key: {msg}");
        assert!(
            msg.contains("task") && msg.contains("system"),
            "lists the keys THIS kind accepts: {msg}"
        );
        assert_actionable("unknown-spec-key", &msg, "spec");

        // The other kind's OPTIONAL field is refused too, not silently ignored:
        // `flavor` on a sub-agent spec is a field the spawner never reads, so
        // accepting it would tell the caller nothing — the silent-ignore this
        // whole contract removes. (An earlier revision accepted the union of
        // both kinds' keys and a test certified that silence.)
        for (label, args) in [
            (
                "flavor-on-subagent",
                json!({ "kind": "subagent", "spec": { "task": "x", "flavor": "full" } }),
            ),
            (
                "system-on-sandbox",
                json!({ "kind": "sandbox_exec", "spec": { "command": "ls", "system": "s" } }),
            ),
        ] {
            let msg = parse(args).expect_err("a cross-kind OPTIONAL field must be refused");
            assert_actionable(label, &msg, "spec");
        }

        // …but the other kind's REQUIRED field is deliberately let through here,
        // because it is the signature of a misplaced `kind` and earns the precise
        // diagnosis in TEST-7 rather than a blunt "unknown key".
        parse(json!({ "kind": "subagent", "spec": { "command": "ls" } }))
            .expect("the other kind's REQUIRED field reaches the misplaced-kind diagnosis");

        // HAPPY-PATH COUNTERPART: every key a kind advertises is accepted.
        parse(json!({ "spec": { "kind": "subagent", "task": "x", "system": "s" } }))
            .expect("every `subagent` spec key must be accepted");
        parse(json!({
            "spec": { "kind": "sandbox_exec", "command": "ls", "flavor": "minimal" }
        }))
        .expect("every `sandbox_exec` spec key must be accepted");

        // The ADVERTISEMENT matches the enforcement: `spec` declares
        // `additionalProperties: false` and declares `kind` among its
        // properties, so a model reading the schema is told the same rule the
        // server applies.
        let list = tool_list();
        let tools = list["tools"].as_array().unwrap();
        let spawn = tools.iter().find(|t| t["name"] == "spawn_background").unwrap();
        let spec_schema = &spawn["inputSchema"]["properties"]["spec"];
        assert_eq!(
            spec_schema["additionalProperties"],
            json!(false),
            "the schema must advertise the closed key set the server enforces"
        );
        let props = spec_schema["properties"].as_object().expect("spec properties");
        // Derived from KIND_CONTRACTS, not a hardcoded list: adding a kind to the
        // table without advertising its fields is the advertisement-vs-enforcement
        // gap this whole change closes, so it must fail HERE.
        let mut table_keys: Vec<&str> = vec!["kind"];
        for k in KIND_CONTRACTS {
            table_keys.extend(k.own_fields());
        }
        for key in &table_keys {
            assert!(props.contains_key(*key), "`spec` must advertise `{key}`");
        }
        for key in props.keys() {
            assert!(
                table_keys.contains(&key.as_str()),
                "the schema advertises `{key}`, which no kind in KIND_CONTRACTS \
                 accepts — an advertisement the server does not honour is the \
                 defect this contract exists to remove"
            );
        }
        // Both `kind` enums (top-level AND the one nested in `spec`) must equal
        // the table, so a third kind cannot be enforced-but-never-advertised.
        let table_kinds: Vec<&str> = KIND_CONTRACTS.iter().map(|k| k.name).collect();
        for path in [&spawn["inputSchema"]["properties"]["kind"], &spec_schema["properties"]["kind"]]
        {
            let advertised: Vec<&str> = path["enum"]
                .as_array()
                .expect("a kind enum")
                .iter()
                .filter_map(|v| v.as_str())
                .collect();
            assert_eq!(
                advertised, table_kinds,
                "every advertised `kind` enum must equal KIND_CONTRACTS"
            );
        }
        // The module's own copyable example must survive its own rule.
        let example: Value =
            serde_json::from_str(BACKGROUND_SPEC_EXAMPLE).expect("the example is valid JSON");
        parse(json!({ "spec": example })).expect("the example the model is told to copy must WORK");
    }

    // TEST-6 [acceptance] [invariant: INV-1]: EVERY refusal this contract can
    // emit names the argument, says what is expected, and carries a copyable
    // literal-JSON example. `BACKGROUND_TASK_REQUIRED`'s old text
    // ("spec.task must be a non-empty string") met none of the three.
    #[test]
    fn every_spawn_refusal_is_actionable() {
        // Argument-parsing refusals.
        let cases: &[(&str, serde_json::Value, &str)] = &[
            ("unknown-kind", json!({ "kind": "nope", "spec": { "task": "x" } }), "kind"),
            (
                "kind-conflict",
                json!({ "kind": "subagent", "spec": { "kind": "sandbox_exec", "command": "ls" } }),
                "kind",
            ),
            ("non-string-kind", json!({ "kind": true, "spec": { "task": "x" } }), "kind"),
            (
                "unknown-spec-key",
                json!({ "kind": "subagent", "spec": { "task": "x", "nope": 1 } }),
                "spec",
            ),
            ("absent-spec", json!({ "kind": "subagent" }), "spec"),
            ("non-json-spec", json!({ "spec": "not json {" }), "spec"),
        ];
        for (label, args, arg) in cases {
            match parse(args.clone()) {
                Err(msg) => assert_actionable(label, &msg, arg),
                Ok((kind, spec)) => panic!(
                    "[{label}] expected a refusal, but parsing SUCCEEDED as \
                     kind=`{kind}` spec={spec}"
                ),
            }
        }

        // The per-kind MISSING-FIELD refusals — the two whose old text
        // ("spec.task must be a non-empty string" / "spec.command must be a
        // non-empty string") is the reported symptom. They are raised after
        // parsing, so they are driven through `require_spec_field` directly.
        for (label, kind_name, spec) in [
            ("missing-task", "subagent", json!({})),
            ("missing-command", "sandbox_exec", json!({})),
            ("missing-task-with-command", "subagent", json!({ "command": "ls" })),
            ("missing-command-with-task", "sandbox_exec", json!({ "task": "x" })),
        ] {
            let contract = find_kind(kind_name).expect("a table kind");
            let msg = require_spec_field(contract, &spec)
                .expect_err("a missing required field must be refused")
                .to_string();
            assert_actionable(label, &msg, &format!("spec.{}", contract.required_field));
        }

        // The flavor refusals, from both model-facing entry points.
        assert_actionable(
            "bad-spec-flavor",
            &resolve_spec_flavor(&json!({ "flavor": "nope" })).unwrap_err().to_string(),
            "spec.flavor",
        );
    }

    // TEST-7: a missing required field names the REAL mistake when the other
    // kind's field is what was actually supplied.
    #[test]
    fn missing_required_field_names_the_misplaced_kind() {
        // The silent-wrong-thing payload, at the field-reading layer: a
        // sandbox_exec spec carrying `task`.
        let (kind, spec) =
            parse(json!({ "spec": { "kind": "sandbox_exec", "task": "Say hello." } })).unwrap();
        assert_eq!(kind, "sandbox_exec", "the supplied kind is honoured");
        let contract = find_kind(kind).unwrap();
        let msg = require_spec_field(contract, &spec).unwrap_err().to_string();
        assert!(msg.contains("spec.command"), "demands the field THIS kind needs: {msg}");
        assert!(msg.contains("spec.task"), "names the field that WAS supplied: {msg}");
        assert!(msg.contains("subagent"), "…and which kind it belongs to: {msg}");
        assert_actionable("cross-kind-task", &msg, "spec.command");

        // Symmetric case: a subagent spec carrying `command`.
        let (kind, spec) = parse(json!({ "kind": "subagent", "spec": { "command": "ls" } })).unwrap();
        let contract = find_kind(kind).unwrap();
        let msg = require_spec_field(contract, &spec).unwrap_err().to_string();
        assert!(msg.contains("spec.task"), "demands `task`: {msg}");
        assert!(msg.contains("spec.command"), "names the misplaced field: {msg}");
        assert!(msg.contains("sandbox_exec"), "…and its kind: {msg}");
        assert_actionable("cross-kind-command", &msg, "spec.task");

        // No cross-kind field → the plain actionable message, no false hint.
        let (kind, spec) = parse(json!({ "kind": "subagent", "spec": { "system": "s" } })).unwrap();
        let contract = find_kind(kind).unwrap();
        let msg = require_spec_field(contract, &spec).unwrap_err().to_string();
        assert!(msg.contains("spec.task"), "still demands `task`: {msg}");
        assert!(
            !msg.contains("belongs to"),
            "must NOT invent a misplaced-kind hint when there is none: {msg}"
        );
        assert_actionable("plain-missing-task", &msg, "spec.task");

        // A cross-kind field that is PRESENT but not usable — explicit null,
        // wrong type, empty string — must NOT trigger the hint either. "Supplied"
        // has to mean the same thing here as it does to the reader, or the
        // refusal confidently steers the model toward a kind whose required
        // field is just as unusable: a hint pointing at another dead end.
        for (label, bad_command) in [
            ("null-command", json!(null)),
            ("number-command", json!(7)),
            ("empty-command", json!("   ")),
        ] {
            let (kind, spec) =
                parse(json!({ "kind": "subagent", "spec": { "command": bad_command } })).unwrap();
            let contract = find_kind(kind).unwrap();
            let msg = require_spec_field(contract, &spec).unwrap_err().to_string();
            assert!(
                !msg.contains("belongs to"),
                "[{label}] `spec.command` was not usably supplied, so it must not be \
                 reported as the real intent: {msg}"
            );
            assert_actionable(label, &msg, "spec.task");
        }

        // HAPPY-PATH COUNTERPART: a well-formed spec still reads its field.
        let (kind, spec) = parse(json!({ "kind": "subagent", "spec": { "task": " x " } })).unwrap();
        assert_eq!(
            require_spec_field(find_kind(kind).unwrap(), &spec).unwrap(),
            "x",
            "a supplied task is read (and trimmed) exactly as before"
        );
    }

    // TEST-8 [acceptance] [invariant: INV-3]: the advertised `flavor` enum is
    // enforced on the background path, before any run row or URL exists.
    #[test]
    fn spec_flavor_is_held_to_the_advertised_enum() {
        let msg = resolve_spec_flavor(&json!({ "flavor": "zee-workflow" }))
            .expect_err("an invented flavor must be refused")
            .to_string();
        assert!(msg.contains("spec.flavor"), "names the argument: {msg}");
        for name in crate::modules::code_sandbox::known_flavor_names() {
            assert!(msg.contains(name), "lists the advertised flavor `{name}`: {msg}");
        }
        assert_actionable("bad-flavor", &msg, "spec.flavor");

        // A supplied-but-empty flavor is refused too, never defaulted (INV-2).
        assert!(resolve_spec_flavor(&json!({ "flavor": "   " })).is_err());
        assert!(resolve_spec_flavor(&json!({ "flavor": 7 })).is_err());

        // HAPPY-PATH COUNTERPART: every advertised flavor is accepted, and an
        // absent/null flavor still falls to the unchanged default.
        for name in crate::modules::code_sandbox::known_flavor_names() {
            assert_eq!(
                resolve_spec_flavor(&json!({ "flavor": name })).unwrap(),
                name,
                "the advertised flavor `{name}` must still be accepted"
            );
        }
        assert_eq!(resolve_spec_flavor(&json!({})).unwrap(), crate::modules::code_sandbox::DEFAULT_TOOL_FLAVOR);
        assert_eq!(
            resolve_spec_flavor(&json!({ "flavor": null })).unwrap(),
            crate::modules::code_sandbox::DEFAULT_TOOL_FLAVOR
        );

        // The schema advertises exactly the flavors the server accepts.
        let list = tool_list();
        let tools = list["tools"].as_array().unwrap();
        let spawn = tools.iter().find(|t| t["name"] == "spawn_background").unwrap();
        let advertised: Vec<&str> = spawn["inputSchema"]["properties"]["spec"]["properties"]
            ["flavor"]["enum"]
            .as_array()
            .expect("flavor enum")
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(
            advertised,
            crate::modules::code_sandbox::known_flavor_names(),
            "the advertised enum and the enforced allow-list must be the same list"
        );
    }

    // TEST-18: EVERY literal-JSON example this module hands a model must survive
    // the module's OWN rules, end to end.
    //
    // This is the `label` defect generalised. `BACKGROUND_SPEC_EXAMPLE` used to
    // advertise a `"label"` key that no schema declares and no code reads; TEST-5
    // pins that one example, but the three the model actually sees in a
    // kind/conflict/unknown-key/missing-field/flavor refusal were unpinned. An
    // example the server would itself refuse is a refusal that cannot be obeyed.
    #[test]
    fn every_example_this_module_hands_out_is_itself_accepted() {
        for kind in KIND_CONTRACTS {
            let args: Value = serde_json::from_str(kind.example)
                .unwrap_or_else(|e| panic!("`{}` example must be valid JSON: {e}", kind.name));
            let (resolved, spec) = parse(args.clone()).unwrap_or_else(|e| {
                panic!("the `{}` example must PARSE, got refusal: {e}", kind.name)
            });
            assert_eq!(
                resolved, kind.name,
                "the `{}` example must resolve to its own kind",
                kind.name
            );
            require_spec_field(kind, &spec).unwrap_or_else(|e| {
                panic!("the `{}` example must satisfy its required field: {e}", kind.name)
            });
        }

        // The flavor example must additionally carry a flavor the server accepts.
        let args: Value =
            serde_json::from_str(SANDBOX_FLAVOR_EXAMPLE).expect("flavor example is valid JSON");
        let (resolved, spec) = parse(args).expect("the flavor example must PARSE");
        assert_eq!(resolved, "sandbox_exec");
        require_spec_field(find_kind(resolved).unwrap(), &spec).expect("…and carry its command");
        assert_eq!(
            resolve_spec_flavor(&spec).expect("…and a flavor the server accepts"),
            "minimal"
        );
    }

    // TEST-19: the default flavor — the value taken on the MOST-TRAVELLED path,
    // where the model supplies nothing — must itself be in the advertised
    // catalog. Nothing else checks it: the absent/null arm returns the constant
    // without consulting the allow-list, so a catalog rename would silently make
    // every no-flavor call construct an unknown-flavor download URL.
    #[test]
    fn default_flavor_is_in_the_catalog() {
        assert!(
            crate::modules::code_sandbox::is_known_flavor(
                crate::modules::code_sandbox::DEFAULT_TOOL_FLAVOR
            ),
            "DEFAULT_TOOL_FLAVOR must be one of {:?}",
            crate::modules::code_sandbox::known_flavor_names()
        );
    }

    // TEST-20: input classes the parser handles that nothing exercised — each of
    // these could have been deleted from the implementation with no test red.
    #[test]
    fn parser_handles_the_awkward_input_classes() {
        // (a) A NESTED non-string `kind`. The whole `location` argument of
        // `read_kind` exists for this message and was previously unasserted.
        let msg = parse(json!({ "spec": { "kind": 42, "task": "x" } }))
            .expect_err("a nested non-string kind must be refused");
        assert!(
            msg.contains("inside `spec`"),
            "the refusal must say WHERE the bad `kind` was: {msg}"
        );
        assert!(msg.contains("a number"), "…and what arrived: {msg}");

        // (b) An empty / whitespace-only `kind` is an UNKNOWN kind, never a
        // "conflict" with a real sibling value and never silently defaulted.
        for spec_kind in ["", "   "] {
            let msg = parse(json!({ "kind": "subagent", "spec": { "kind": spec_kind, "task": "x" } }))
                .expect_err("an empty kind must be refused");
            assert!(
                !msg.contains("supplied twice"),
                "an empty string is not a competing value — it must not be reported as a \
                 contradiction: {msg}"
            );
            assert_actionable("empty-kind", &msg, "kind");
        }

        // (c) Whitespace around a REAL kind is trimmed, not rejected.
        let (kind, _) = parse(json!({ "kind": "  sandbox_exec  ", "spec": { "command": "ls" } }))
            .expect("a padded kind must be trimmed");
        assert_eq!(kind, "sandbox_exec");

        // (d) A JSON-ENCODED `spec` that ALSO carries a nested `kind` — the exact
        // combination `decode_spec_arg` was moved ahead of the kind read for. It
        // must decode AND resolve, not fall to the default.
        let (kind, spec) = parse(json!({
            "spec": r#"{"kind":"sandbox_exec","command":"python hello.py"}"#
        }))
        .expect("a stringified spec carrying a nested kind must decode and resolve");
        assert_eq!(kind, "sandbox_exec", "the nested kind survives decoding");
        assert_eq!(spec["command"], json!("python hello.py"));

        // (e) More unknown keys than the refusal names: the message must SAY it
        // was cut, or a model that fixes the named keys loops on the same error.
        let mut many = serde_json::Map::new();
        many.insert("task".into(), json!("x"));
        for i in 0..(MAX_REPORTED_UNKNOWN_KEYS + 3) {
            many.insert(format!("bogus_{i}"), json!(1));
        }
        let msg = parse(json!({ "kind": "subagent", "spec": Value::Object(many) }))
            .expect_err("unknown keys must be refused");
        assert!(msg.contains("and 3 more"), "the truncated list must say so: {msg}");
        assert_actionable("many-unknown-keys", &msg, "spec");
    }

    // TEST-21: the dispatch arm and the run row's `job_kind` are the SAME field,
    // so they cannot disagree. Each spawner used to hardcode its own `JobKind`
    // while receiving an unrelated contract for its refusals — passing the wrong
    // one compiled cleanly and produced a `subagent` run demanding `spec.command`.
    #[test]
    fn each_contract_names_its_own_job_kind() {
        for k in KIND_CONTRACTS {
            assert_eq!(
                k.job_kind.as_str(),
                k.name,
                "a contract's advertised `kind` and its run-row `job_kind` must be the \
                 same value — the dispatch selects on one and the row is written from \
                 the other"
            );
        }
    }
}
