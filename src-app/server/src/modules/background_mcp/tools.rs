//! Tool descriptors + dispatch for the built-in background_mcp server.
//!
//! The uniform background-run surface (ITEM-17) on the `workflow_runs`-backed
//! backbone: `spawn_background` (a WRITE — launches a detached run, routed
//! through approval) + `check_status` / `collect_result` (owner-scoped READS,
//! approval-bypassed). Ownership is enforced at every read via
//! `repository::find_run_for_owner` (a cross-user `run_id` → 404, never leaks).

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

/// Static tool descriptors emitted by `tools/list`.
pub fn tool_list() -> Value {
    json!({
        "tools": [
            {
                "name": "spawn_background",
                "description": "Launch background work DETACHED from this conversation — you keep chatting while it runs, then collect its result later with `collect_result`. Two kinds: 'subagent' runs a detached agent turn on a self-contained task (research a question, draft a section, analyze data); 'sandbox_exec' runs a shell command in this conversation's isolated code sandbox as a background job (a long build, a data crunch, a test suite) so it doesn't block the chat. Use for a bounded unit of work whose answer you don't need inline right now. Returns an opaque `run_id`. Do NOT use for trivial things you can answer directly. This LAUNCHES work, so it requires approval before it starts.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "kind": {
                            "type": "string",
                            "enum": ["subagent", "sandbox_exec"],
                            "default": "subagent",
                            "description": "The background job kind. 'subagent' runs a detached agent turn on the spec; 'sandbox_exec' runs a shell command in this conversation's code sandbox as a background job."
                        },
                        "spec": {
                            "type": "object",
                            "description": "What the background job should do. The required fields depend on `kind`: 'subagent' requires `task`; 'sandbox_exec' requires `command`.",
                            "properties": {
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
                "description": "Check the state and progress of a background run you spawned, by its `run_id`. Cheap and non-blocking: use it to see whether a background task is still running, has completed, failed, or is waiting for input — WITHOUT fetching the full result (use `collect_result` for that). Only your own runs are visible; an unknown or foreign id returns not found.",
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
                "description": "Read the final result of a background run by its `run_id`. Idempotent — safe to call repeatedly — and paged for large outputs via `offset`/`max_chars`. If the run has not finished yet, this returns its current status instead of a result, so poll `check_status` (or retry) until it is complete. Only your own runs are visible; an unknown or foreign id returns not found.",
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
    let kind_str = args.get("kind").and_then(|v| v.as_str()).unwrap_or("subagent");
    let spec = args
        .get("spec")
        .cloned()
        .ok_or_else(|| AppError::bad_request("BACKGROUND_SPEC_REQUIRED", "spec is required"))?;

    match kind_str {
        "subagent" => spawn_subagent(pool, user_id, conversation_id, spec).await,
        "sandbox_exec" => spawn_sandbox_exec(pool, user_id, conversation_id, spec).await,
        other => Err(AppError::bad_request(
            "BACKGROUND_KIND_UNKNOWN",
            format!("unknown background kind '{other}'"),
        )),
    }
}

/// `spawn_background{kind:'subagent'}` — launch a detached [`JobKind::SubAgent`]
/// agent-core turn on `spec.{task,system}`.
async fn spawn_subagent(
    pool: &PgPool,
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    spec: Value,
) -> Result<Value, AppError> {
    let job_kind = JobKind::SubAgent;
    let task = spec
        .get("task")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::bad_request("BACKGROUND_TASK_REQUIRED", "spec.task must be a non-empty string"))?
        .to_string();
    let system = spec
        .get("system")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Resolve the model the detached sub-agent runs on: the originating
    // conversation's model (re-checked under the owner's RBAC at turn time by
    // `create_provider_from_model_id`). A conversation with no model set — or a
    // spawn with no conversation context — has nothing to run on, so reject
    // clearly instead of launching a doomed run. Recorded on the run row so the
    // choice is durable + auditable.
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
        "note": "Background run started. Poll check_status, then collect_result when it is complete."
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
        let summary = final_output
            .as_ref()
            .and_then(|v| v.get("final_text"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.chars().take(500).collect::<String>())
            .unwrap_or_else(|| "Background task finished.".to_string());
        post_completion_notification(pool, user_id, run_id, conversation_id, summary).await;
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

/// Default rootfs flavor for a background sandbox command (matches the foreground
/// `execute_command` tool's `default_flavor`).
const DEFAULT_SANDBOX_FLAVOR: &str = "minimal";

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
    spec: Value,
) -> Result<Value, AppError> {
    let conversation_id = conversation_id.ok_or_else(|| {
        AppError::bad_request(
            "BACKGROUND_NO_CONVERSATION",
            "background sandbox exec requires a conversation context (the sandbox workspace is per-conversation)",
        )
    })?;
    let command = spec
        .get("command")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            AppError::bad_request(
                "BACKGROUND_COMMAND_REQUIRED",
                "spec.command must be a non-empty string",
            )
        })?
        .to_string();
    let flavor = spec
        .get("flavor")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_SANDBOX_FLAVOR)
        .to_string();

    // Fail fast on a foreign / missing conversation (owner-scoped 404); the run
    // row is only created for a conversation the caller actually owns.
    crate::core::Repos
        .chat
        .core
        .get_conversation(conversation_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("conversation not found"))?;

    let request = CreateBackgroundRun {
        job_kind: JobKind::SandboxExec,
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
        "kind": JobKind::SandboxExec.as_str(),
        "status": "pending",
        "note": "Background sandbox command started. Poll check_status, then collect_result when it is complete."
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
/// progress. A foreign / missing id → 404 (never leaks another user's run).
async fn check_status(pool: &PgPool, user_id: Uuid, args: &Value) -> Result<Value, AppError> {
    let run_id = parse_run_id(args)?;
    let run = repository::find_run_for_owner(pool, run_id, user_id)
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
/// (the model should retry). A foreign / missing id → 404.
async fn collect_result(pool: &PgPool, user_id: Uuid, args: &Value) -> Result<Value, AppError> {
    let run_id = parse_run_id(args)?;
    let run = repository::find_run_for_owner(pool, run_id, user_id)
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
