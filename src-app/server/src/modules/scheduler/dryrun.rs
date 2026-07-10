//! Dry-run / test-fire (DEC-24, ITEM-34) — execute a task's TARGET once and
//! return the result inline, with the scheduler's side effects suppressed: no
//! notification, no `scheduled_task_runs` row, no schedule mutation, and (for
//! the prompt kind) a THROWAWAY conversation that is deleted afterward so the
//! bound conversation is never touched. This backs the drawer's "Test" button
//! ("does this do what I meant?" before committing to a cadence).
//!
//! Scope note: the workflow kind executes a real `workflow_runs` row (viewable /
//! user-deletable on its own) — a fully side-effect-free workflow dry-run would
//! need the workflow test harness's heavy setup; the scheduler-side surface
//! (notifications / task history / schedule) is untouched, which is what the
//! "no side effects" contract is about here.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;
use crate::core::config::Config;
use crate::modules::chat::core::extension::SendMessageRequest;
use crate::modules::chat::core::services::StreamingService;
use crate::modules::chat::extension_registration::auto_register_extensions;
use crate::modules::workflow::runner::{SpawnRunOpts, spawn_run};

const TERMINAL_WAIT: Duration = Duration::from_secs(10 * 60);
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// The target config to test (an unsaved drawer config or a saved task's fields).
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct TestFireRequest {
    pub target_kind: String, // 'workflow' | 'prompt'
    pub workflow_id: Option<Uuid>,
    #[serde(default = "empty_object")]
    pub inputs_json: serde_json::Value,
    pub assistant_id: Option<Uuid>,
    pub prompt: Option<String>,
    pub model_id: Uuid,
}

/// The inline result of a test-fire.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TestFireResult {
    pub ok: bool,
    pub text: String,
    pub error: Option<String>,
}

fn empty_object() -> serde_json::Value {
    serde_json::json!({})
}

/// Run the target once, side-effect-free, as `user_id`. Never returns Err — a
/// failure is captured into `TestFireResult { ok:false, error }`.
pub async fn test_fire(
    pool: &PgPool,
    config: &Arc<Config>,
    user_id: Uuid,
    req: &TestFireRequest,
) -> TestFireResult {
    let outcome = match req.target_kind.as_str() {
        "workflow" => test_workflow(pool, user_id, req).await,
        "prompt" => test_prompt(pool, config, user_id, req).await,
        other => Err(AppError::bad_request(
            "SCHEDULER_BAD_TARGET_KIND",
            format!("unknown target_kind {other}"),
        )),
    };
    match outcome {
        Ok(text) => TestFireResult {
            ok: true,
            text,
            error: None,
        },
        Err(e) => TestFireResult {
            ok: false,
            text: String::new(),
            error: Some(e.to_string()),
        },
    }
}

async fn test_workflow(
    pool: &PgPool,
    user_id: Uuid,
    req: &TestFireRequest,
) -> Result<String, AppError> {
    let workflow_id = req
        .workflow_id
        .ok_or_else(|| AppError::bad_request("SCHEDULER_BAD_TARGET", "workflow_id required"))?;
    let workflow = crate::modules::workflow::repository::find_by_id(pool, workflow_id)
        .await?
        .ok_or_else(|| AppError::not_found("Workflow"))?;
    // Access re-check (see dispatch::dispatch_workflow) — test-fire must not run
    // a workflow the user can't access.
    if !crate::modules::workflow::repository::user_can_access(pool, user_id, workflow_id).await? {
        return Err(AppError::not_found("Workflow"));
    }
    let run_id = spawn_run(
        pool,
        &workflow,
        user_id,
        None,
        req.inputs_json.clone(),
        HashMap::new(),
        SpawnRunOpts {
            model_id: Some(req.model_id),
            invocation_source: "scheduled",
            persist_artifacts: false,
            force_log_capture: false,
        },
    )
    .await?;

    let deadline = tokio::time::Instant::now() + TERMINAL_WAIT;
    loop {
        let run = crate::modules::workflow::repository::find_run(pool, run_id)
            .await?
            .ok_or_else(|| AppError::not_found("WorkflowRun"))?;
        match run.status.as_str() {
            "completed" => {
                return Ok(super::dispatch::summarize_workflow_output(
                    run.final_output_json.as_ref(),
                ));
            }
            "failed" | "cancelled" => {
                return Err(AppError::internal_error(
                    run.error_message
                        .unwrap_or_else(|| format!("workflow run {}", run.status)),
                ));
            }
            _ => {}
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(AppError::internal_error("workflow run timed out"));
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

async fn test_prompt(
    pool: &PgPool,
    config: &Arc<Config>,
    user_id: Uuid,
    req: &TestFireRequest,
) -> Result<String, AppError> {
    let prompt = req
        .prompt
        .clone()
        .filter(|p| !p.trim().is_empty())
        .ok_or_else(|| AppError::bad_request("SCHEDULER_BAD_TARGET", "prompt required"))?;

    // A test-fire config is unsaved (never hit create's gate), so validate the
    // assistant belongs to the user here too.
    if let Some(aid) = req.assistant_id {
        if Repos.assistant.get_for_user(aid, user_id).await?.is_none() {
            return Err(AppError::not_found("Assistant"));
        }
    }

    // Throwaway conversation — deleted in every exit path below.
    let conv = Repos
        .chat
        .core
        .create_conversation(user_id, Some(req.model_id), Some("Scheduled task test".into()))
        .await?;
    let conversation_id = conv.id;
    let branch_id = conv.active_branch_id;

    let result = match branch_id {
        Some(bid) => run_throwaway_turn(pool, config, user_id, req, &prompt, conversation_id, bid).await,
        None => Err(AppError::internal_error("throwaway conversation has no branch")),
    };

    // Best-effort cleanup regardless of outcome.
    let _ = Repos
        .chat
        .core
        .delete_conversation(conversation_id, user_id)
        .await;

    result
}

#[allow(clippy::too_many_arguments)]
async fn run_throwaway_turn(
    pool: &PgPool,
    config: &Arc<Config>,
    user_id: Uuid,
    req: &TestFireRequest,
    prompt: &str,
    conversation_id: Uuid,
    branch_id: Uuid,
) -> Result<String, AppError> {
    // Test-fire runs headless too (no user to approve), so it uses the SAME
    // unattended policy as a real scheduled run: approval-required, non-allow-
    // listed tools are denied and no Always-mode side effect pre-executes during
    // a mere "Test" (blind-audit fidelity fix — Test previously attached ALL
    // accessible servers). The allow-list rides the request when the UI sends it;
    // absent ⇒ the read-only safe floor.
    let mut req_json = serde_json::json!({
        "content": prompt,
        "model_id": req.model_id,
        "branch_id": branch_id,
        "enable_mcp": true,
        "unattended": true,
        // No allow-list on the test body → the read-only safe floor: only
        // built-in read-only servers attach (empty mcp_servers ⇒ no third-party,
        // so no Always-mode pre-execution during a Test).
        "unattended_allowed_tools": [],
        "mcp_config": { "mcp_servers": [] },
    });
    if let Some(aid) = req.assistant_id {
        req_json["assistant_id"] = serde_json::json!(aid);
    }
    let request: SendMessageRequest = serde_json::from_value(req_json)
        .map_err(|e| AppError::internal_error(format!("build request: {e}")))?;

    let registry = Arc::new(auto_register_extensions(pool.clone(), config.clone()));
    let service = StreamingService::new(pool.clone()).with_extensions(registry);
    let (_u, assistant_message_id) = service
        .start_generation(branch_id, conversation_id, user_id, None, request)
        .await?;

    let deadline = tokio::time::Instant::now() + TERMINAL_WAIT;
    while crate::modules::chat::stream::registry::is_generating(conversation_id) {
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }

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
    Ok(text)
}
