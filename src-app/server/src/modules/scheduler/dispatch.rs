//! Target dispatch — runs a task's target to completion, applies
//! change-detection, and (unless suppressed) writes an inbox notification.
//!
//! Two kinds, both awaited to a terminal result so change-detection has the
//! full output:
//!   * workflow — `runner::spawn_run(invocation_source="scheduled")` then poll
//!     `find_run` to terminal (mirrors `workflow_mcp::await_terminal`).
//!   * prompt — drive the real chat pipeline via `StreamingService`, append to
//!     the task's BOUND conversation, and poll the in-process generation slot
//!     (`chat::stream::registry::is_generating`) to detect turn completion,
//!     then read the assistant text.
//!
//! `dispatch` never returns `Err` — every failure is captured into the outcome
//! so the tick always records a run + (for failures) a notification.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;
use crate::core::config::Config;
use crate::modules::chat::core::extension::SendMessageRequest;
use crate::modules::chat::core::services::StreamingService;
use crate::modules::chat::extension_registration::auto_register_extensions;
use crate::modules::notification::events::create_and_emit;
use crate::modules::notification::models::NewNotification;
use crate::modules::workflow::runner::{SpawnRunOpts, spawn_run};

use super::change::{self, Signature};
use super::failure::{FailureClass, classify};
use super::models::ScheduledTask;

/// Max characters of result text embedded in a notification body.
const NOTIF_BODY_CAP: usize = 800;
/// How long to wait for a target to reach a terminal state.
const TERMINAL_WAIT: Duration = Duration::from_secs(15 * 60);
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// What the tick records after a firing.
pub struct DispatchOutcome {
    pub success: bool,
    pub status: &'static str, // "completed" | "no_change" | "failed"
    pub error_class: Option<String>,
    pub error_message: Option<String>,
    pub notification_id: Option<Uuid>,
    pub workflow_run_id: Option<Uuid>,
    pub conversation_id: Option<Uuid>,
    pub fingerprint: Option<String>,
    pub signature: Option<serde_json::Value>,
}

/// A successful target run, before change-detection/notification.
struct RawResult {
    text: String,
    workflow_run_id: Option<Uuid>,
    conversation_id: Option<Uuid>,
}

/// Run a task's target, apply change-detection, and notify. Total function.
pub async fn dispatch(
    pool: &PgPool,
    config: &Arc<Config>,
    task: &ScheduledTask,
    _trigger: &str,
) -> DispatchOutcome {
    let raw = match task.target_kind.as_str() {
        "workflow" => dispatch_workflow(pool, task).await,
        "prompt" => dispatch_prompt(pool, config, task).await,
        other => Err(AppError::internal_error(format!(
            "scheduler: unknown target_kind {other}"
        ))),
    };

    match raw {
        Ok(raw) => finalize_success(pool, task, raw).await,
        Err(e) => finalize_failure(pool, task, e).await,
    }
}

// ── workflow target ────────────────────────────────────────────────────

async fn dispatch_workflow(pool: &PgPool, task: &ScheduledTask) -> Result<RawResult, AppError> {
    let workflow_id = task
        .workflow_id
        .ok_or_else(|| AppError::not_found("Workflow"))?;
    let workflow = crate::modules::workflow::repository::find_by_id(pool, workflow_id)
        .await?
        .ok_or_else(|| AppError::not_found("Workflow"))?;

    let run_id = spawn_run(
        pool,
        &workflow,
        task.user_id,
        None,
        task.inputs_json.clone(),
        HashMap::new(),
        SpawnRunOpts {
            model_id: task.model_id,
            invocation_source: "scheduled",
            persist_artifacts: true,
            force_log_capture: false,
        },
    )
    .await?;

    // Poll to terminal (mirrors workflow_mcp::await_terminal).
    let deadline = tokio::time::Instant::now() + TERMINAL_WAIT;
    loop {
        let run = crate::modules::workflow::repository::find_run(pool, run_id)
            .await?
            .ok_or_else(|| AppError::not_found("WorkflowRun"))?;
        match run.status.as_str() {
            "completed" => {
                return Ok(RawResult {
                    text: summarize_workflow_output(run.final_output_json.as_ref()),
                    workflow_run_id: Some(run_id),
                    conversation_id: None,
                });
            }
            "failed" | "cancelled" => {
                let msg = run
                    .error_message
                    .unwrap_or_else(|| format!("workflow run {}", run.status));
                return Err(AppError::internal_error(msg));
            }
            _ => {}
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(AppError::internal_error("workflow run timed out"));
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Build a text digest from a workflow run's `final_output_json`
/// (`{ output_name: { value_preview, .. } }`) for change-detection + the body.
pub fn summarize_workflow_output(final_output: Option<&serde_json::Value>) -> String {
    let Some(obj) = final_output.and_then(|v| v.as_object()) else {
        return String::new();
    };
    let mut lines = Vec::new();
    for (k, v) in obj {
        let preview = v
            .get("value_preview")
            .and_then(|p| p.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| v.to_string());
        lines.push(format!("{k}: {preview}"));
    }
    lines.join("\n")
}

// ── prompt target ──────────────────────────────────────────────────────

async fn dispatch_prompt(
    pool: &PgPool,
    config: &Arc<Config>,
    task: &ScheduledTask,
) -> Result<RawResult, AppError> {
    let model_id = task
        .model_id
        .ok_or_else(|| AppError::not_found("Model"))?;

    // Resolve the bound conversation (reuse or create), and its active branch.
    let (conversation_id, branch_id) = match task.bound_conversation_id {
        Some(cid) => {
            let conv = Repos
                .chat
                .core
                .get_conversation(cid, task.user_id)
                .await?
                .ok_or_else(|| AppError::not_found("Conversation"))?;
            let bid = conv
                .active_branch_id
                .ok_or_else(|| AppError::internal_error("bound conversation has no branch"))?;
            (cid, bid)
        }
        None => {
            let conv = Repos
                .chat
                .core
                .create_conversation(task.user_id, Some(model_id), Some(task.name.clone()))
                .await?;
            let bid = conv
                .active_branch_id
                .ok_or_else(|| AppError::internal_error("new conversation has no branch"))?;
            super::repository::set_bound_conversation(pool, task.id, conv.id).await?;
            (conv.id, bid)
        }
    };

    // Build the send request via JSON (extension fields default). Enable MCP so
    // agentic tasks ("check PubMed…") can use the built-in tools.
    let mut req_json = serde_json::json!({
        "content": task.prompt.clone().unwrap_or_default(),
        "model_id": model_id,
        "branch_id": branch_id,
        "enable_mcp": true,
    });
    if let Some(aid) = task.assistant_id {
        req_json["assistant_id"] = serde_json::json!(aid);
    }
    let request: SendMessageRequest = serde_json::from_value(req_json)
        .map_err(|e| AppError::internal_error(format!("scheduler: build request: {e}")))?;

    let registry = Arc::new(auto_register_extensions(pool.clone(), config.clone()));
    let service = StreamingService::new(pool.clone()).with_extensions(registry);
    let (_user_msg, assistant_message_id) = service
        .start_generation(branch_id, conversation_id, task.user_id, None, request)
        .await?;

    // Wait for the detached turn to finish: the generation slot flips off when
    // the terminal frame publishes. (start_generation claims the slot before
    // returning, so there is no start race.)
    let deadline = tokio::time::Instant::now() + TERMINAL_WAIT;
    while crate::modules::chat::stream::registry::is_generating(conversation_id) {
        if tokio::time::Instant::now() >= deadline {
            break; // best-effort: read whatever completed
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }

    // Read the assistant text (join `text` content blocks by sequence order).
    let text = Repos
        .chat
        .core
        .get_message_with_content(assistant_message_id)
        .await?
        .map(|mwc| {
            mwc.contents
                .iter()
                .filter(|c| c.content_type == "text")
                .filter_map(|c| c.content.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();

    Ok(RawResult {
        text,
        workflow_run_id: None,
        conversation_id: Some(conversation_id),
    })
}

// ── shared finalize (change-detection + notification) ──────────────────

async fn finalize_success(
    pool: &PgPool,
    task: &ScheduledTask,
    raw: RawResult,
) -> DispatchOutcome {
    let sig = change::compute_signature(&raw.text);
    let prev: Option<Signature> = task
        .last_result_signature_json
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let outcome = change::diff(prev.as_ref(), &sig);

    let sig_json = serde_json::to_value(&sig).ok();
    let fingerprint = Some(sig.fingerprint.clone());

    // on_change + nothing changed → record success, send NO notification.
    if task.notify_on == "on_change" && !outcome.changed {
        return DispatchOutcome {
            success: true,
            status: "no_change",
            error_class: None,
            error_message: None,
            notification_id: None,
            workflow_run_id: raw.workflow_run_id,
            conversation_id: raw.conversation_id,
            fingerprint,
            signature: sig_json,
        };
    }

    // Build the notification (delta leads the body when we have identifiable items).
    let mut body = String::new();
    if !outcome.new_items.is_empty() {
        body.push_str(&format!("{} new since last run.\n\n", outcome.new_items.len()));
    }
    body.push_str(truncate(&raw.text, NOTIF_BODY_CAP));
    let title = format!("Scheduled task '{}' ran", task.name);

    let interrupt = task.notify_mode == "always";
    let mut n = NewNotification::new(task.user_id, "scheduled_task_result", title)
        .body(body)
        .task(task.id);
    if !interrupt {
        n = n.silent();
    }
    if let Some(wr) = raw.workflow_run_id {
        n = n.workflow_run(wr);
    }
    if let Some(cid) = raw.conversation_id {
        n = n.conversation(cid);
    }
    let notification_id = create_and_emit(pool, n).await.ok().map(|row| row.id);

    DispatchOutcome {
        success: true,
        status: "completed",
        error_class: None,
        error_message: None,
        notification_id,
        workflow_run_id: raw.workflow_run_id,
        conversation_id: raw.conversation_id,
        fingerprint,
        signature: sig_json,
    }
}

async fn finalize_failure(
    pool: &PgPool,
    task: &ScheduledTask,
    err: AppError,
) -> DispatchOutcome {
    let status = axum::http::StatusCode::from_u16(err.status_code())
        .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    let class: FailureClass = classify(status, false);
    let msg = err.to_string();

    // A failure always interrupts (regardless of notify_mode) — the user needs
    // to know their task stopped working.
    let n = NewNotification::new(
        task.user_id,
        "scheduled_task_failed",
        format!("Scheduled task '{}' failed", task.name),
    )
    .body(truncate(&msg, NOTIF_BODY_CAP))
    .task(task.id);
    let notification_id = create_and_emit(pool, n).await.ok().map(|row| row.id);

    DispatchOutcome {
        success: false,
        status: "failed",
        error_class: Some(class.as_str().to_string()),
        error_message: Some(msg),
        notification_id,
        workflow_run_id: None,
        conversation_id: None,
        fingerprint: None,
        signature: None,
    }
}

fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}
