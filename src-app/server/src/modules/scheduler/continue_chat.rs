//! "Continue in chat" — seed a NEW conversation with a scheduled run's REAL result
//! as a synthesized ASSISTANT turn (DEC-23), so the user can keep chatting about a
//! background result. Covers:
//!   * single-run follow-up (ITEM-42) — a prompt run's real last assistant text, or
//!     a workflow run's output digest + its persisted artifacts as file blocks;
//!   * series follow-up (ITEM-43 / J5) — the last N runs' previews + deltas folded
//!     into one assistant summary.
//!
//! A short USER framing turn precedes the assistant turn so the seeded history is
//! provider-valid (starts with a user role) while the RESULT lives in the assistant
//! turn (per DEC-23, not embedded in a user message).

use schemars::JsonSchema;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::chat::core::models::content::MessageContentData;
use crate::modules::file::chat_extension::types::FileContent;

use super::dispatch::summarize_workflow_output;
use super::models::ScheduledTaskRun;

/// Response of the continue endpoints: the id of the freshly-seeded conversation
/// the client should navigate to.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ContinueResult {
    pub conversation_id: Uuid,
}

/// Cap on the seeded result text so a large run output doesn't bloat the seed.
const RESULT_TEXT_MAX: usize = 8000;
/// Cap on the whole series-summary assistant text (bounds 100 runs × preview).
const SERIES_TEXT_MAX: usize = 12000;
/// Server-side clamp on the series follow-up run count (DEC-22 "all-loaded").
const SERIES_LIMIT_MAX: i64 = 100;

/// The pure plan for a seeded conversation: a user framing turn + the assistant
/// turn carrying the real result, plus artifact file ids to attach to the
/// assistant turn (ITEM-42).
struct SeedPlan {
    title: String,
    user_text: String,
    assistant_text: String,
    file_ids: Vec<Uuid>,
}

/// Pure builder for a SINGLE-run seed (DEC-23). The assistant turn carries the
/// resolved result (or a truthful fallback when none was captured). Testable
/// without I/O (TEST-45).
fn build_run_seed(
    task_name: &str,
    run_status: &str,
    result_text: &str,
    file_ids: Vec<Uuid>,
) -> SeedPlan {
    let trimmed = result_text.trim();
    let assistant_text = if trimmed.is_empty() {
        format!(
            "The scheduled run of \"{task_name}\" finished (status: {run_status}), but produced no result text."
        )
    } else {
        trimmed.chars().take(RESULT_TEXT_MAX).collect()
    };
    SeedPlan {
        title: format!("Continue: {task_name}"),
        user_text: format!("What did the latest scheduled run of \"{task_name}\" produce?"),
        assistant_text,
        file_ids,
    }
}

/// Pure builder for a SERIES seed (DEC-22 / ITEM-43): folds the given runs
/// (already newest-first from the repo) into one assistant summary, including each
/// run's delta. Testable without I/O (TEST-47).
fn build_series_seed(task_name: &str, runs: &[ScheduledTaskRun]) -> SeedPlan {
    let mut lines = Vec::new();
    for r in runs {
        let when = r.fired_at.format("%Y-%m-%d %H:%M UTC");
        let new_count = r
            .change_summary_json
            .as_ref()
            .and_then(|v| v.get("new_count"))
            .and_then(|n| n.as_i64())
            .unwrap_or(0);
        let preview = r.result_preview.as_deref().unwrap_or("(no result captured)");
        let delta = if new_count > 0 {
            format!(" — {new_count} new")
        } else {
            String::new()
        };
        lines.push(format!("- {when} [{}]{delta}: {preview}", r.status));
    }
    let assistant_text = if lines.is_empty() {
        format!("\"{task_name}\" has no recorded runs yet.")
    } else {
        let body = format!(
            "Here are the last {} runs of \"{task_name}\" (newest first):\n\n{}",
            lines.len(),
            lines.join("\n")
        );
        body.chars().take(SERIES_TEXT_MAX).collect()
    };
    SeedPlan {
        title: format!("Series: {task_name}"),
        user_text: format!("Summarize and compare the recent runs of \"{task_name}\"."),
        assistant_text,
        file_ids: Vec::new(),
    }
}

/// Resolve a single run's REAL result text + owner-scoped artifact file ids
/// (ITEM-42). Prompt run → the last assistant text from its source conversation;
/// workflow run → the run-output digest + its persisted artifacts (re-scoped to
/// the caller, since `list_ids_by_workflow_run` is run-id-scoped, not user-scoped).
async fn resolve_run_result(
    pool: &PgPool,
    user_id: Uuid,
    run: &ScheduledTaskRun,
) -> (String, Vec<Uuid>) {
    if let Some(src_conv) = run.conversation_id {
        if let Ok(Some(conv)) = Repos.chat.core.get_conversation(src_conv, user_id).await {
            if let Some(bid) = conv.active_branch_id {
                if let Ok(history) = Repos.chat.core.get_conversation_history(bid).await {
                    if let Some(last) =
                        history.iter().rev().find(|m| m.message.role == "assistant")
                    {
                        let text = last
                            .contents
                            .iter()
                            .filter(|c| c.content_type == "text")
                            .filter_map(|c| c.content.get("text").and_then(|t| t.as_str()))
                            .collect::<Vec<_>>()
                            .join("\n");
                        return (text, Vec::new());
                    }
                }
            }
        }
    }

    if let Some(wr) = run.workflow_run_id {
        let mut text = String::new();
        if let Ok(Some(wrun)) = crate::modules::workflow::repository::find_run(pool, wr).await {
            text = summarize_workflow_output(wrun.final_output_json.as_ref());
        }
        let mut owned = Vec::new();
        if let Ok(ids) = Repos.file_workflow_runs.list_file_ids(wr).await {
            for fid in ids {
                if let Ok(Some(f)) = Repos.file.get_by_id(fid).await {
                    if f.user_id == user_id {
                        owned.push(fid);
                    }
                }
            }
        }
        return (text, owned);
    }

    (String::new(), Vec::new())
}

/// Create a conversation from a SeedPlan (user framing turn + assistant result turn
/// + artifact file blocks) and return its id. Shared by both follow-up paths.
async fn seed_conversation(
    user_id: Uuid,
    model_id: Option<Uuid>,
    plan: SeedPlan,
) -> Result<Uuid, AppError> {
    let conv = Repos
        .chat
        .core
        .create_conversation(user_id, model_id, Some(plan.title))
        .await?;
    let branch_id = conv
        .active_branch_id
        .ok_or_else(|| AppError::internal_error("new conversation has no branch"))?;

    // User framing turn (keeps the seeded history provider-valid).
    let umsg = Repos
        .chat
        .core
        .create_message(branch_id, "user", model_id)
        .await?;
    Repos
        .chat
        .core
        .append_content(umsg.id, "text", MessageContentData::Text { text: plan.user_text })
        .await?;

    // Assistant turn carrying the REAL result (DEC-23).
    let amsg = Repos
        .chat
        .core
        .create_message(branch_id, "assistant", model_id)
        .await?;
    Repos
        .chat
        .core
        .append_content(
            amsg.id,
            "text",
            MessageContentData::Text { text: plan.assistant_text },
        )
        .await?;

    // Attach artifacts as file blocks on the assistant turn (owner already checked).
    for fid in plan.file_ids {
        if let Ok(Some(f)) = Repos.file.get_by_id(fid).await {
            let block = FileContent::FileAttachment {
                file_id: f.id,
                filename: f.filename,
                mime_type: f.mime_type,
                file_size: f.file_size,
                version_id: Some(f.current_version_id),
                version: Some(f.version),
            }
            .to_message_content();
            let ct = block.content_type();
            Repos.chat.core.append_content(amsg.id, &ct, block).await?;
        }
    }

    Ok(conv.id)
}

/// Single-run follow-up (ITEM-42). Owner-scoped: the caller resolves the run under
/// `user_id`; this re-scopes the parent task too.
pub async fn continue_run_in_chat(
    pool: &PgPool,
    user_id: Uuid,
    run: &ScheduledTaskRun,
) -> Result<Uuid, AppError> {
    let task = super::repository::get_for_user(pool, user_id, run.scheduled_task_id)
        .await?
        .ok_or_else(|| AppError::not_found("Scheduled task"))?;
    let (text, file_ids) = resolve_run_result(pool, user_id, run).await;
    let plan = build_run_seed(&task.name, &run.status, &text, file_ids);
    seed_conversation(user_id, task.model_id, plan).await
}

/// Series follow-up (ITEM-43 / J5): seed a conversation with the last N runs of a
/// task. Owner-scoped (foreign task → 404 via `get_for_user`).
pub async fn continue_series_in_chat(
    pool: &PgPool,
    user_id: Uuid,
    task_id: Uuid,
    limit: i64,
) -> Result<Uuid, AppError> {
    let task = super::repository::get_for_user(pool, user_id, task_id)
        .await?
        .ok_or_else(|| AppError::not_found("Scheduled task"))?;
    let limit = limit.clamp(1, SERIES_LIMIT_MAX);
    let (runs, _total) =
        super::repository::list_runs_for_task(pool, user_id, task_id, 1, limit).await?;
    let plan = build_series_seed(&task.name, &runs);
    seed_conversation(user_id, task.model_id, plan).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn run_with(status: &str, preview: Option<&str>, new_count: i64) -> ScheduledTaskRun {
        ScheduledTaskRun {
            id: Uuid::new_v4(),
            scheduled_task_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            trigger: "schedule".into(),
            status: status.into(),
            error_class: None,
            error_message: None,
            notification_id: None,
            workflow_run_id: None,
            conversation_id: None,
            skipped_tools: serde_json::json!([]),
            result_preview: preview.map(|s| s.to_string()),
            change_summary_json: Some(serde_json::json!({ "changed": new_count > 0, "new_count": new_count })),
            fired_at: Utc::now(),
            finished_at: None,
        }
    }

    // TEST-45 (ITEM-42): the single-run seed builder emits an assistant turn
    // carrying the resolved result (+ a truthful fallback when empty), and carries
    // the artifact ids forward.
    #[test]
    fn build_run_seed_carries_real_result_and_artifacts() {
        let fid = Uuid::new_v4();
        let plan = build_run_seed("Morning brief", "completed", "3 new papers on X.", vec![fid]);
        assert_eq!(plan.assistant_text, "3 new papers on X.", "assistant turn = real result");
        assert_eq!(plan.file_ids, vec![fid], "artifact ids carried to the assistant turn");
        assert!(plan.title.contains("Morning brief"));

        let empty = build_run_seed("W", "completed", "   ", vec![]);
        assert!(
            empty.assistant_text.contains("no result text"),
            "empty result → truthful fallback, not a blank turn: {}",
            empty.assistant_text
        );
    }

    // TEST-47 (ITEM-43): the series seed folds each run's preview + delta into one
    // assistant summary, in the order given (repo yields newest-first).
    #[test]
    fn build_series_seed_summarizes_runs_with_deltas() {
        let runs = vec![
            run_with("completed", Some("2 papers on base editing"), 2),
            run_with("no_change", Some("no change"), 0),
        ];
        let plan = build_series_seed("Lit watch", &runs);
        assert!(plan.assistant_text.contains("last 2 runs"), "count: {}", plan.assistant_text);
        assert!(plan.assistant_text.contains("2 papers on base editing"));
        assert!(plan.assistant_text.contains("— 2 new"), "delta shown for a changed run");
        assert!(plan.assistant_text.contains("[no_change]"), "each run's status shown");
        // First run listed before the second (newest-first order preserved).
        let i_new = plan.assistant_text.find("base editing").unwrap();
        let i_nochange = plan.assistant_text.find("[no_change]").unwrap();
        assert!(i_new < i_nochange, "runs rendered in the given order");

        let empty = build_series_seed("Idle", &[]);
        assert!(empty.assistant_text.contains("no recorded runs"));
    }
}
