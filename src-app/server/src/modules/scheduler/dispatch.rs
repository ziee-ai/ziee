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
use crate::modules::notification::models::NewNotification;
use ziee_notification::create_and_emit;
use crate::modules::workflow::runner::{SpawnRunOpts, spawn_run};

use super::change::{self, Signature};
use super::failure::{FailureClass, classify};
use super::models::ScheduledTask;

/// Max characters of result text embedded in a notification body.
const NOTIF_BODY_CAP: usize = 800;
/// Round 2 / DEC-20 (fixed UX constant): max chars of the per-run `result_preview`
/// digest surfaced in the runs timeline (J1). Named so it can be promoted to a
/// configurable setting later without a rewrite.
const PREVIEW_CHARS: usize = 280;
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
    /// Tools skipped this firing because they weren't permitted unattended
    /// (ITEM-17 / DEC-17.5). Empty for workflow runs + un-gated paths.
    pub skipped_tools: Vec<super::models::SkippedTool>,
    /// Round 2 (ITEM-40): a short digest of the result for the runs timeline;
    /// `None` for a failed firing (no result).
    pub result_preview: Option<String>,
    /// Round 2 (ITEM-40): `{ changed, new_count, new_items }` from change-detection;
    /// `None` for a failed firing.
    pub change_summary: Option<serde_json::Value>,
    /// ITEM-24 / DEC-63: the fired turn's result artifact text — the ONLY input
    /// (besides the condition) the goal-seeking evaluator sees. `None` for a
    /// failed / empty firing (⇒ the evaluator returns not_done without a model
    /// call). Transient (not persisted); consumed in-process by the write-back.
    pub result_text: Option<String>,
}

/// A successful target run, before change-detection/notification.
struct RawResult {
    text: String,
    workflow_run_id: Option<Uuid>,
    conversation_id: Option<Uuid>,
    /// Tools skipped during a headless prompt run (ITEM-17); empty for workflow.
    skipped_tools: Vec<super::models::SkippedTool>,
}

/// ITEM-14/DEC-17: build the unattended run's `mcp_config.mcp_servers` attach set
/// from the task's allow-list grants — one entry per distinct server; a
/// whole-server grant (`tool_name: None`) yields `tools: []` (= all tools), and
/// per-tool grants for a server are grouped into its `tools`. An EMPTY grant list
/// yields an EMPTY set ⇒ no third-party servers attach (the read-only safe floor;
/// built-ins auto-attach separately). Pure + unit-tested (TEST-26/27/34).
pub(super) fn build_unattended_mcp_servers(
    grants: &[super::models::AllowedTool],
) -> Vec<serde_json::Value> {
    let mut by_server: HashMap<Uuid, (bool, Vec<String>)> = HashMap::new();
    for g in grants {
        let entry = by_server.entry(g.server_id).or_insert((false, Vec::new()));
        match &g.tool_name {
            Some(t) => entry.1.push(t.clone()),
            None => entry.0 = true, // whole-server grant
        }
    }
    by_server
        .into_iter()
        .map(|(sid, (whole, tools))| {
            serde_json::json!({ "server_id": sid, "tools": if whole { Vec::<String>::new() } else { tools } })
        })
        .collect()
}

/// ITEM-17/DEC-17.5: the notification line for tools skipped this firing (or
/// `None` when none were). Proper singular/plural, no emoji. Pure + unit-tested.
pub(super) fn skipped_tools_note(skipped: &[super::models::SkippedTool]) -> Option<String> {
    if skipped.is_empty() {
        return None;
    }
    let names: Vec<&str> = skipped.iter().map(|s| s.tool_name.as_str()).collect();
    let n = skipped.len();
    let noun = if n == 1 { "tool was" } else { "tools were" };
    Some(format!(
        "\n\nNote: {n} {noun} skipped (not permitted unattended): {}. Pre-authorize on the task to allow.",
        names.join(", ")
    ))
}

/// Round 2 (ITEM-40): a char-safe, single-paragraph preview of the result text for
/// the runs timeline. `None` for an empty result. Pure + unit-tested (TEST-41).
pub(super) fn build_result_preview(text: &str) -> Option<String> {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return None;
    }
    Some(collapsed.chars().take(PREVIEW_CHARS).collect())
}

/// Round 2 (ITEM-40): reduce a change-detection outcome to the run's persisted
/// `change_summary_json` — `{ changed, new_count, new_items }` (new_items capped so a
/// huge delta can't bloat the row). Pure + unit-tested (TEST-41).
pub(super) fn build_change_summary(outcome: &super::change::ChangeOutcome) -> serde_json::Value {
    const NEW_ITEMS_CAP: usize = 50;
    let items: Vec<&String> = outcome.new_items.iter().take(NEW_ITEMS_CAP).collect();
    serde_json::json!({
        "changed": outcome.changed,
        "new_count": outcome.new_items.len(),
        "new_items": items,
    })
}

/// ITEM-21 / DEC-42/44/45: compute the self-paced WRITE-BACK outcome for a fired
/// turn — clamp the model's proposed delay via `schedule::next_self_paced_fire`
/// (honoring the min-interval floor, the max-horizon ceiling, and the absolute
/// per-task expiry), or, when NO proposal was produced (the model-facing
/// `schedule_next` tool is a later tranche), self-stop. Pure + unit-testable; the
/// caller (`tick::fire_task`) applies the result via `repository::arm_self_paced`.
pub(super) fn self_paced_writeback(
    proposal: Option<&super::schedule::SelfPacedProposal>,
    min_interval_seconds: i64,
    max_horizon_days: i64,
    created_at: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> super::schedule::SelfPacedOutcome {
    match proposal {
        Some(p) => super::schedule::next_self_paced_fire(
            p,
            min_interval_seconds,
            max_horizon_days,
            created_at,
            now,
        ),
        // No proposal (tool not yet wired) ⇒ a fired self-paced turn self-completes
        // rather than looping forever.
        None => super::schedule::SelfPacedOutcome::Disable,
    }
}

/// ITEM-9/DEC-8: transient-failure tolerance is provided by the consecutive-
/// failure CAP (`max_consecutive_failures`, admin-configurable), NOT by an
/// in-run retry. An earlier design re-ran the whole `dispatch` on a transient
/// classification, but both targets are NON-IDEMPOTENT — re-running `dispatch_
/// workflow` re-`spawn_run`s the entire workflow (duplicate/overlapping
/// execution) and re-running `dispatch_prompt` re-sends the message + re-executes
/// its tools (double side effects). So the firing runs the target EXACTLY ONCE;
/// a transient failure is recorded and counts toward the cap like any failure
/// (the tick only auto-pauses at the cap, so a single blip never pauses).
/// See DRIFT-1 / FIX_ROUND-1 (blind-audit HIGH: in-run retry re-execution).
///
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
    // Re-check access at fire time (owner scope / group assignment): a workflow
    // the user lost access to — or never had — must not run. `find_by_id` is
    // id-scoped only, so this is the real authorization gate.
    if !crate::modules::workflow::repository::user_can_access(pool, task.user_id, workflow_id)
        .await?
    {
        return Err(AppError::not_found("Workflow"));
    }

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
                    skipped_tools: Vec::new(),
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

    // Re-check assistant access at fire time (defense-in-depth; also gated at create).
    if let Some(aid) = task.assistant_id {
        if Repos.assistant.get_for_user(aid, task.user_id).await?.is_none() {
            return Err(AppError::not_found("Assistant"));
        }
    }
    // Re-check MODEL access at fire time too (blind-audit fix: create validated it
    // but the firing did not — a user removed from the provider's group must not
    // keep invoking that model via the schedule). 403 → Permission → terminal pause.
    {
        let model = Repos
            .llm_model
            .get_by_id(model_id)
            .await?
            .ok_or_else(|| AppError::not_found("Model"))?;
        if !Repos
            .user_group_llm_provider
            .user_has_access_to_provider(task.user_id, model.provider_id)
            .await?
        {
            return Err(AppError::forbidden(
                "SCHEDULER_MODEL_FORBIDDEN",
                "you do not have access to this model",
            ));
        }
    }

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

    // ITEM-14/DEC-17: constrain the third-party MCP servers to the task's
    // allow-list so a non-allow-listed (esp. Always-mode side-effecting) server
    // never attaches / pre-executes in an unattended run. `mcp_config = Some`
    // with an explicit server list means "ONLY these third-party servers"
    // (empty ⇒ none); built-in read-only servers still auto-attach regardless.
    let grants = super::models::parse_allowed_tools(&task.allowed_unattended_tools);
    let mcp_servers = build_unattended_mcp_servers(&grants);

    // Build the send request via JSON (extension fields default). Enable MCP so
    // agentic tasks ("check PubMed…") can use the built-in tools; mark the run
    // UNATTENDED so the MCP approval decision denies (not pauses) non-allow-listed
    // approval-required tools (ITEM-13).
    let mut req_json = serde_json::json!({
        "content": task.prompt.clone().unwrap_or_default(),
        "model_id": model_id,
        "branch_id": branch_id,
        "enable_mcp": true,
        "unattended": true,
        "unattended_allowed_tools": task.allowed_unattended_tools.clone(),
        "mcp_config": { "mcp_servers": mcp_servers },
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
    let mwc = Repos
        .chat
        .core
        .get_message_with_content(assistant_message_id)
        .await?;

    let (text, skipped_tools) = match mwc {
        Some(mwc) => {
            let text = mwc
                .contents
                .iter()
                .filter(|c| c.content_type == "text")
                .filter_map(|c| c.content.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n");
            // ITEM-17: the unattended-approval gate (mcp.rs) emits a denial
            // tool_result marked `structured_content.unattended_denied` for each
            // tool it skipped. Collect them for the run's skipped-tools report.
            let skipped: Vec<super::models::SkippedTool> = mwc
                .contents
                .iter()
                .filter(|c| c.content_type == "tool_result")
                .filter_map(|c| {
                    let sc = c.content.get("structured_content")?;
                    if sc.get("unattended_denied").and_then(|v| v.as_bool()) == Some(true) {
                        Some(super::models::SkippedTool {
                            tool_name: sc
                                .get("tool_name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("tool")
                                .to_string(),
                            reason: "requires approval; not permitted unattended".to_string(),
                        })
                    } else {
                        None
                    }
                })
                .collect();
            (text, skipped)
        }
        None => (String::new(), Vec::new()),
    };

    Ok(RawResult {
        text,
        workflow_run_id: None,
        conversation_id: Some(conversation_id),
        skipped_tools,
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

    // Round 2 (ITEM-40): persist a preview + change summary on the run for the timeline.
    let result_preview = build_result_preview(&raw.text);
    let change_summary = Some(build_change_summary(&outcome));
    // ITEM-24 / DEC-63: carry the full result artifact for the goal-seeking
    // evaluator (transient; capped when the evaluator builds its prompt). None
    // for an empty result → the evaluator returns not_done without a model call.
    let result_text = (!raw.text.trim().is_empty()).then(|| raw.text.clone());

    // on_change + nothing changed → record success, send NO notification —
    // UNLESS tools were skipped this firing. A skipped tool means the result is
    // degraded/incomplete, so it must not be silently swallowed as "no change"
    // (blind-audit fix: the ITEM-17 honesty guard was bypassed on this path).
    if task.notify_on == "on_change" && !outcome.changed && raw.skipped_tools.is_empty() {
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
            skipped_tools: raw.skipped_tools.clone(),
            result_preview,
            change_summary,
            result_text,
        };
    }

    // Build the notification (delta leads the body when we have identifiable items).
    let mut body = String::new();
    if !outcome.new_items.is_empty() {
        body.push_str(&format!("{} new since last run.\n\n", outcome.new_items.len()));
    }
    body.push_str(truncate(&raw.text, NOTIF_BODY_CAP));
    // ITEM-17: be honest — a truncated result must not read as a clean success.
    if let Some(note) = skipped_tools_note(&raw.skipped_tools) {
        body.push_str(&note);
    }
    let title = format!("Scheduled task '{}' ran", task.name);

    let interrupt = task.notify_mode == "always";
    // R2 payload convention: kind-specific ids ride the `payload jsonb` column
    // (the SDK notification schema is domain-agnostic). For a scheduler-produced
    // notification the well-known keys are `scheduled_task_id` (always) and,
    // when the run produced them, `workflow_run_id` / `conversation_id`. The FE
    // renderer reads these via `n.payload?.<key>`; only applicable keys are set.
    let mut payload = serde_json::Map::new();
    payload.insert("scheduled_task_id".into(), serde_json::json!(task.id));
    if let Some(wr) = raw.workflow_run_id {
        payload.insert("workflow_run_id".into(), serde_json::json!(wr));
    }
    if let Some(cid) = raw.conversation_id {
        payload.insert("conversation_id".into(), serde_json::json!(cid));
    }
    let mut n = NewNotification::new(task.user_id, "scheduled_task_result", title)
        .body(body)
        .payload(serde_json::Value::Object(payload));
    if !interrupt {
        n = n.silent();
    }
    // ITEM-11: log (don't silently swallow) a notification-creation failure —
    // a broken task that also fails to notify was previously undiagnosable.
    let notification_id = match create_and_emit(pool, n).await {
        Ok(row) => Some(row.id),
        Err(e) => {
            tracing::warn!("scheduler: failed to create task notification for '{}': {e:?}", task.name);
            None
        }
    };

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
        skipped_tools: raw.skipped_tools.clone(),
        result_preview,
        change_summary,
        result_text,
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
    // R2 payload convention (see finalize_success): the well-known
    // `scheduled_task_id` key rides the `payload jsonb` column.
    let n = NewNotification::new(
        task.user_id,
        "scheduled_task_failed",
        format!("Scheduled task '{}' failed", task.name),
    )
    .body(truncate(&msg, NOTIF_BODY_CAP))
    .payload(serde_json::json!({ "scheduled_task_id": task.id }));
    // ITEM-11: log (don't silently swallow) a notification-creation failure —
    // a broken task that also fails to notify was previously undiagnosable.
    let notification_id = match create_and_emit(pool, n).await {
        Ok(row) => Some(row.id),
        Err(e) => {
            tracing::warn!("scheduler: failed to create task notification for '{}': {e:?}", task.name);
            None
        }
    };

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
        skipped_tools: Vec::new(),
        result_preview: None,
        change_summary: None,
        result_text: None,
    }
}

fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::super::change::ChangeOutcome;
    use super::super::models::{AllowedTool, SkippedTool};
    use super::{build_change_summary, build_result_preview, build_unattended_mcp_servers, skipped_tools_note};
    use uuid::Uuid;

    // TEST-41 (ITEM-40): the preview truncates char-safely + collapses whitespace,
    // and the change-summary maps a ChangeOutcome → {changed,new_count,new_items}.
    #[test]
    fn result_preview_and_change_summary_are_built() {
        assert_eq!(build_result_preview("   "), None, "empty/whitespace → None");
        let p = build_result_preview("Found 3 papers.\n\n  See  10.1000/x.").unwrap();
        assert_eq!(p, "Found 3 papers. See 10.1000/x.", "whitespace collapsed");
        let long: String = "x".repeat(500);
        assert_eq!(build_result_preview(&long).unwrap().chars().count(), 280, "capped at PREVIEW_CHARS");

        let unchanged = build_change_summary(&ChangeOutcome { changed: false, new_items: vec![] });
        assert_eq!(unchanged["changed"], serde_json::json!(false));
        assert_eq!(unchanged["new_count"], serde_json::json!(0));
        let changed = build_change_summary(&ChangeOutcome {
            changed: true,
            new_items: vec!["doi:10.1/a".into(), "doi:10.2/b".into()],
        });
        assert_eq!(changed["changed"], serde_json::json!(true));
        assert_eq!(changed["new_count"], serde_json::json!(2));
        assert_eq!(changed["new_items"].as_array().unwrap().len(), 2);
    }

    // TEST-27/TEST-26: the unattended mcp_config attach set = allow-listed servers.
    #[test]
    fn build_unattended_mcp_servers_constrains_to_allow_list() {
        // Empty allow-list → empty set (read-only safe floor: no third-party attaches).
        assert!(build_unattended_mcp_servers(&[]).is_empty());

        let s1 = Uuid::new_v4();
        let s2 = Uuid::new_v4();
        // Whole-server grant → tools:[] (= all tools); per-tool grants grouped.
        let grants = vec![
            AllowedTool { server_id: s1, tool_name: None },
            AllowedTool { server_id: s2, tool_name: Some("search".into()) },
            AllowedTool { server_id: s2, tool_name: Some("fetch".into()) },
        ];
        let out = build_unattended_mcp_servers(&grants);
        assert_eq!(out.len(), 2, "one entry per distinct server");
        let e1 = out.iter().find(|e| e["server_id"] == serde_json::json!(s1)).unwrap();
        assert_eq!(e1["tools"], serde_json::json!([]), "whole-server grant → all tools");
        let e2 = out.iter().find(|e| e["server_id"] == serde_json::json!(s2)).unwrap();
        let tools = e2["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2, "per-tool grants grouped under the server");
        assert!(tools.contains(&serde_json::json!("search")) && tools.contains(&serde_json::json!("fetch")));
        // A non-allow-listed server is NEVER in the attach set.
        let s3 = Uuid::new_v4();
        assert!(!out.iter().any(|e| e["server_id"] == serde_json::json!(s3)));
    }

    // TEST-31: the skipped-tools notification line (round-trip into the body).
    #[test]
    fn skipped_tools_note_is_honest_and_pluralized() {
        assert_eq!(skipped_tools_note(&[]), None, "no skips → no note");
        let one = vec![SkippedTool { tool_name: "post_message".into(), reason: "x".into() }];
        let n1 = skipped_tools_note(&one).unwrap();
        assert!(n1.contains("1 tool was skipped"), "singular: {n1}");
        assert!(n1.contains("post_message"));
        assert!(!n1.contains('⚠'), "no emoji");
        let two = vec![
            SkippedTool { tool_name: "a".into(), reason: "x".into() },
            SkippedTool { tool_name: "b".into(), reason: "x".into() },
        ];
        let n2 = skipped_tools_note(&two).unwrap();
        assert!(n2.contains("2 tools were skipped"), "plural: {n2}");
        assert!(n2.contains("a, b"));
    }

    // TEST-34: the DisabledServer predicate the scheduled-workflow disabled-servers
    // gate (ITEM-18b) relies on — a whole-server disable blocks all its tools; a
    // tool-scoped disable blocks only that tool.
    #[test]
    fn disabled_server_predicate_blocks_correctly() {
        use crate::modules::mcp::chat_extension::approval::models::DisabledServer;
        let sid = Uuid::new_v4();
        let whole = DisabledServer { server_id: sid, tools: vec![] };
        assert!(whole.is_server_disabled(), "empty tools = whole server disabled");
        assert!(whole.is_tool_disabled("anything"));
        let scoped = DisabledServer { server_id: sid, tools: vec!["send".into()] };
        assert!(!scoped.is_server_disabled(), "tool-scoped ≠ whole-server");
        assert!(scoped.is_tool_disabled("send"));
        assert!(!scoped.is_tool_disabled("read"));
    }
}
