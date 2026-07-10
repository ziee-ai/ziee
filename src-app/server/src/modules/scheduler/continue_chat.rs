//! "Continue in chat" (ITEM-32) — open a NEW conversation seeded with a
//! scheduled-task run's output/context so the user can keep chatting about a
//! background result. A `prompt`-target run already owns a conversation
//! (ITEM-30's bound conversation); this is the general affordance that also
//! covers `workflow`-target runs, whose result is not itself a conversation.

use schemars::JsonSchema;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::chat::core::models::content::MessageContentData;

use super::models::ScheduledTaskRun;

/// Response of the continue-in-chat endpoint: the id of the freshly-seeded
/// conversation the client should navigate to.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ContinueResult {
    pub conversation_id: Uuid,
}

/// Snippet cap so a large run output doesn't bloat the seed message.
const SNIPPET_MAX: usize = 2000;

/// Create a fresh conversation seeded with the run's context (status + a
/// truncated snippet of its output when available) and return the new
/// conversation id. Owner-scoped: the caller resolves the run under `user_id`.
pub async fn continue_run_in_chat(
    pool: &PgPool,
    user_id: Uuid,
    run: &ScheduledTaskRun,
) -> Result<Uuid, AppError> {
    // Parent task → its name + model (defense-in-depth: re-scoped to the user).
    let task = super::repository::get_for_user(pool, user_id, run.scheduled_task_id)
        .await?
        .ok_or_else(|| AppError::not_found("Scheduled task"))?;

    // Best-effort: pull the last assistant text from the run's source
    // conversation (prompt-kind runs) to genuinely seed with the result.
    let mut snippet = String::new();
    if let Some(src_conv) = run.conversation_id {
        if let Ok(Some(conv)) = Repos.chat.core.get_conversation(src_conv, user_id).await {
            if let Some(bid) = conv.active_branch_id {
                if let Ok(history) = Repos.chat.core.get_conversation_history(bid).await {
                    if let Some(last) = history
                        .iter()
                        .rev()
                        .find(|m| m.message.role == "assistant")
                    {
                        let text = last
                            .contents
                            .iter()
                            .filter(|c| c.content_type == "text")
                            .filter_map(|c| c.content.get("text").and_then(|t| t.as_str()))
                            .collect::<Vec<_>>()
                            .join("\n");
                        if !text.is_empty() {
                            let truncated: String = text.chars().take(SNIPPET_MAX).collect();
                            snippet = format!("\n\nPrevious result:\n{truncated}");
                        }
                    }
                }
            }
        }
    }

    // Create the new conversation + seed a user message. Robust to a missing
    // source conversation — the status-only seed still opens a usable chat.
    let conv = Repos
        .chat
        .core
        .create_conversation(
            user_id,
            task.model_id,
            Some(format!("Continue: {}", task.name)),
        )
        .await?;
    let branch_id = conv
        .active_branch_id
        .ok_or_else(|| AppError::internal_error("new conversation has no branch"))?;

    let seed = format!(
        "Continuing from a scheduled run of \"{}\" (status: {}). Let's discuss the result.{}",
        task.name, run.status, snippet
    );
    let msg = Repos
        .chat
        .core
        .create_message(branch_id, "user", task.model_id)
        .await?;
    Repos
        .chat
        .core
        .append_content(msg.id, "text", MessageContentData::Text { text: seed })
        .await?;

    Ok(conv.id)
}
