//! Tool descriptors + dispatch for the built-in background_mcp server.
//!
//! The uniform background-run surface (ITEM-17) on the `workflow_runs`-backed
//! backbone: `spawn_background` (a WRITE — launches a detached run, routed
//! through approval) + `check_status` / `collect_result` (owner-scoped READS,
//! approval-bypassed). Ownership is enforced at every read via
//! `repository::find_run_for_owner` (a cross-user `run_id` → 404, never leaks).

use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::notification::models::NewNotification;
use crate::modules::workflow::models::{CreateBackgroundRun, JobKind, WorkflowRunStatus};
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
                "description": "Launch a background sub-agent that works on a self-contained task DETACHED from this conversation — you keep chatting while it runs, then collect its result later with `collect_result`. Use for a bounded unit of work that may take a while (research a question, draft a section, analyze data) and whose answer you don't need inline right now. Returns an opaque `run_id`. Do NOT use for trivial things you can answer directly. This LAUNCHES work, so it requires approval before it starts.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "kind": {
                            "type": "string",
                            "enum": ["subagent"],
                            "default": "subagent",
                            "description": "The background job kind. 'subagent' runs a detached agent turn on the spec."
                        },
                        "spec": {
                            "type": "object",
                            "description": "What the background sub-agent should do.",
                            "properties": {
                                "system": {
                                    "type": "string",
                                    "description": "Optional system framing / role for the sub-agent."
                                },
                                "task": {
                                    "type": "string",
                                    "description": "The concrete task the sub-agent must complete and report back on."
                                }
                            },
                            "required": ["task"]
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
/// notify); the kind-specific work runs in the `driver` closure below.
async fn spawn_background(
    pool: &PgPool,
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    args: &Value,
) -> Result<Value, AppError> {
    let kind_str = args.get("kind").and_then(|v| v.as_str()).unwrap_or("subagent");
    let job_kind = match kind_str {
        "subagent" => JobKind::SubAgent,
        // The sandbox-exec background driver is cross-repo (sdk `ziee-sandbox`)
        // and lands in a later tranche; reject it clearly rather than pretend.
        "sandbox_exec" => {
            return Err(AppError::bad_request(
                "BACKGROUND_KIND_UNSUPPORTED",
                "background kind 'sandbox_exec' is not available yet",
            ));
        }
        other => {
            return Err(AppError::bad_request(
                "BACKGROUND_KIND_UNKNOWN",
                format!("unknown background kind '{other}'"),
            ));
        }
    };

    let spec = args
        .get("spec")
        .cloned()
        .ok_or_else(|| AppError::bad_request("BACKGROUND_SPEC_REQUIRED", "spec is required"))?;
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

    let request = CreateBackgroundRun {
        job_kind,
        conversation_id,
        user_id,
        model_id: None,
        sandbox_flavor: None,
        // An LLM tool call from a conversation (mirrors workflow_mcp's
        // `wf_<slug>` convention for a chat-model-driven run).
        invocation_source: "conversation".into(),
        inputs_json: spec.clone(),
    };

    // Capture the spec into the detached driver (ITEM-7 / ITEM-9). The driver
    // runs OUTSIDE any per-conversation single-flight lock — this is
    // fire-and-forget, so the foreground chat stays interactive.
    let run_id = runner::spawn_background_run(pool, request, move |task_pool, run_id, _handle| async move {
        execute_subagent_run(&task_pool, run_id, user_id, conversation_id, &system, &task).await
    })
    .await?;

    Ok(json!({
        "run_id": run_id,
        "kind": job_kind.as_str(),
        "status": "pending",
        "note": "Background run started. Poll check_status, then collect_result when it is complete."
    }))
}

/// The SubAgent background driver (ITEM-7 / ITEM-9).
///
/// **NOTE — MINIMAL EXECUTOR (flagged; NOT faked silently).** This tranche wires
/// the FULL durable run lifecycle end-to-end — a `workflow_runs` row → `running`
/// + heartbeat → terminal `completed` + `final_output_json` → owner-scoped
/// `SyncEntity::WorkflowRun` notify (all via `spawn_background_run`) → an ITEM-9
/// `notification` inbox row + `SyncEntity::Notification`. The ONE piece that is a
/// placeholder is the actual LLM turn: building a detached `AgentCore` (its 6
/// injected ports + reviewer + model resolution) is large and can't be
/// integration-tested in this pass, so instead of a real agent turn this records
/// the received spec and returns an HONEST structured summary marked
/// `executor:"minimal-placeholder"`. A follow-up replaces the body below with a
/// real `AgentCore` turn built from the port impls in
/// `workflow/agent_dispatch.rs` (the proven `kind: agent` host) — the run-row +
/// notification + sync scaffolding here stays unchanged.
async fn execute_subagent_run(
    pool: &PgPool,
    run_id: Uuid,
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    system: &str,
    task: &str,
) -> BackgroundOutcome {
    // ── Placeholder work (see the NOTE above — the real AgentCore turn is a
    //    follow-up). Produce an honest, self-describing summary. ──
    let summary = format!(
        "Background sub-agent received the task and recorded it. (Minimal executor: \
         the detached agent turn is not yet wired — a follow-up runs a real AgentCore \
         turn here.) Task: {task}"
    );
    let final_output = json!({
        "executor": "minimal-placeholder",
        "status": "recorded",
        "summary": summary,
        "spec": { "system": system, "task": task },
    });

    // ── ITEM-9: results-land-when-done. Post a durable inbox row so an away user
    //    is told, and it live-pushes via the installed `SyncEntity::Notification`
    //    emitter. (`spawn_background_run` separately emits `SyncEntity::WorkflowRun`
    //    on the terminal transition.) A notify failure must NOT fail the run —
    //    log and continue, exactly like the scheduler's first-producer path. ──
    let mut payload = serde_json::Map::new();
    payload.insert("workflow_run_id".into(), json!(run_id));
    if let Some(cid) = conversation_id {
        payload.insert("conversation_id".into(), json!(cid));
    }
    let notif = NewNotification::new(user_id, "background_run_result", "Background task finished")
        .body(summary.clone())
        .payload(Value::Object(payload));
    if let Err(e) = create_and_emit(pool, notif).await {
        tracing::warn!("background_mcp: failed to create completion notification for run {run_id}: {e:?}");
    }

    BackgroundOutcome::Completed {
        final_output: Some(final_output),
    }
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
}
