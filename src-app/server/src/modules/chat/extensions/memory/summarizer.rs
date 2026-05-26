//! Conversation summarizer — when a branch exceeds N messages the
//! summarizer condenses the oldest messages into a single text block
//! stored in `conversation_summaries`. `apply_summary_to_history`
//! (called from `MemoryExtension::before_llm_call`) replaces those
//! summarized messages in the LLM request with the summary block,
//! freeing real prompt-side budget.
//!
//! Trigger lives in `MemoryExtension::after_llm_call` (fire-and-forget
//! spawn). Threshold + keep-recent come from `memory_admin_settings`
//! (admin-tunable, no restart needed).

use ai_providers::{ChatMessage, ChatRequest, ContentBlock, Provider, Role};
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;

/// Fallback summarizer thresholds — used only if the admin settings
/// row can't be read on a given call (transient DB blip). Match the
/// column DEFAULTs in migration 52. The runtime values come from
/// `memory_admin_settings.summarize_after_n_messages` /
/// `.summarizer_keep_recent` and can be tuned per-deployment from the
/// admin UI without a redeploy.
const FALLBACK_SUMMARIZE_AFTER_N_MESSAGES: usize = 50;
const FALLBACK_KEEP_RECENT: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConversationSummary {
    pub branch_id: Uuid,
    pub summary_text: String,
    pub summarized_up_to_id: Option<Uuid>,
    pub message_count: i32,
    pub model_used: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Fetch the persisted summary (if any) for this branch.
pub async fn fetch_summary(
    pool: &PgPool,
    branch_id: Uuid,
) -> Result<Option<ConversationSummary>, AppError> {
    let row = sqlx::query_as!(
        ConversationSummary,
        r#"
        SELECT
            branch_id,
            summary_text,
            summarized_up_to_id,
            message_count,
            model_used,
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        FROM conversation_summaries
        WHERE branch_id = $1
        "#,
        branch_id
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

/// Replace the summarized prefix of `chat_request.messages` with the
/// persisted summary block. Idempotent — if no summary exists for this
/// branch, the request is left untouched.
///
/// Pruning algorithm: chat_request.messages is built as
///   [System*, User|Assistant*]
/// where the leading System block is the assistant's instructions
/// (and any other extension-injected system context that has ALREADY
/// run before us — the retriever's memory-block comes AFTER summary
/// per the call order in `MemoryExtension::before_llm_call`).
///
/// We:
///   1. Count the leading System prefix length → `system_prefix_len`.
///   2. Drop the next `summary.message_count` messages — these are the
///      user/assistant turns the summary condenses. Clamp to the
///      available range so a shorter-than-expected history doesn't
///      panic.
///   3. Insert the summary as a single System message at
///      `system_prefix_len` (where the dropped block used to start).
///
/// Net effect: the LLM sees `[System*, SummaryBlock, RecentTurns]`
/// instead of `[System*, AllOldTurns, RecentTurns]`. Context budget
/// gets freed proportionally to `summary.message_count`.
pub async fn apply_summary_to_history(
    branch_id: Uuid,
    chat_request: &mut ChatRequest,
) -> Result<(), AppError> {
    let pool = Repos.memory.pool_clone();
    let summary = match fetch_summary(&pool, branch_id).await? {
        Some(s) => s,
        None => return Ok(()),
    };

    let system_prefix_len = chat_request
        .messages
        .iter()
        .take_while(|m| matches!(m.role, Role::System))
        .count();

    // Clamp to the actual length so we never drain past the end. In
    // normal operation the history has at least keep_recent verbatim
    // messages remaining; the clamp guards pathological cases (race
    // between summary write and history truncation, history rebuilt
    // smaller than at summarization time, etc.).
    let raw_drop_until = system_prefix_len.saturating_add(summary.message_count as usize);
    let drop_until = raw_drop_until.min(chat_request.messages.len());

    if drop_until > system_prefix_len {
        chat_request
            .messages
            .drain(system_prefix_len..drop_until);
    }

    let block = format!(
        "## Earlier conversation summary ({} messages condensed):\n\n{}",
        summary.message_count, summary.summary_text
    );
    chat_request.messages.insert(
        system_prefix_len,
        ChatMessage {
            role: Role::System,
            content: vec![ContentBlock::Text { text: block }],
        },
    );
    Ok(())
}

/// Generate / refresh the persisted summary for this branch. Runs the
/// configured summarization model against all messages older than the
/// last KEEP_RECENT in the branch. Idempotent at the row level — does
/// an upsert keyed on `branch_id`.
pub async fn refresh_summary(
    branch_id: Uuid,
    summarization_model_id: Uuid,
) -> Result<(), AppError> {
    let history = Repos.chat.core.get_conversation_history(branch_id).await?;
    let conv_msgs: Vec<_> = history
        .iter()
        .filter(|m| m.message.role == "user" || m.message.role == "assistant")
        .collect();

    // Read thresholds fresh — admin tuning takes effect on the next
    // summarization without a restart.
    let (trigger, keep_recent) = match Repos.memory.get_admin_settings().await {
        Ok(s) => (s.summarize_after_n_messages as usize, s.summarizer_keep_recent as usize),
        Err(e) => {
            tracing::warn!(
                "memory.summarizer: get_admin_settings failed ({e}); using fallback thresholds"
            );
            (FALLBACK_SUMMARIZE_AFTER_N_MESSAGES, FALLBACK_KEEP_RECENT)
        }
    };

    if conv_msgs.len() <= trigger {
        return Ok(()); // Nothing to summarize.
    }

    let cutoff = conv_msgs.len().saturating_sub(keep_recent);
    let to_summarize: Vec<_> = conv_msgs.iter().take(cutoff).collect();
    if to_summarize.is_empty() {
        return Ok(());
    }
    let summarized_up_to_id = to_summarize.last().map(|m| m.message.id);

    // Build a concatenated transcript. Only text content is collected
    // (tool calls / file attachments are skipped; the summary is a
    // textual narrative).
    let mut transcript = String::new();
    for m in &to_summarize {
        let mut text = String::new();
        for c in &m.contents {
            let Ok(data) = c.parse_content() else { continue };
            let Ok(value) = serde_json::to_value(&data) else { continue };
            if value.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(t) = value.get("text").and_then(|t| t.as_str()) {
                    text.push_str(t);
                }
            }
        }
        if !text.is_empty() {
            transcript.push_str(&format!("{}: {}\n", m.message.role, text));
        }
    }
    if transcript.is_empty() {
        return Ok(());
    }

    let prompt = format!(
        r#"Summarize the following conversation into a concise narrative (3-6 sentences) capturing the essential context: who the user is, what they're trying to accomplish, key facts established, and any unresolved threads. Output only the summary text; no preamble.

Conversation:
{}"#,
        transcript
    );

    let model = Repos
        .llm_model
        .get_by_id(summarization_model_id)
        .await
        .map_err(AppError::database_error)?
        .ok_or_else(|| AppError::not_found("LlmModel"))?;
    let provider = Repos
        .llm_provider
        .get_by_id(model.provider_id)
        .await
        .map_err(AppError::database_error)?
        .ok_or_else(|| AppError::internal_error("Summarization provider not found"))?;

    let api_key = provider.api_key.as_deref().unwrap_or("");
    let base_url = provider.base_url.as_deref().ok_or_else(|| {
        AppError::internal_error(format!("Provider '{}' has no base_url", provider.name))
    })?;
    let ai_provider = Provider::new(&provider.provider_type, api_key, base_url)
        .map_err(|e| AppError::internal_error(format!("create summary provider: {e}")))?;

    let req = ChatRequest {
        model: model.name.clone(),
        messages: vec![ChatMessage {
            role: Role::User,
            content: vec![ContentBlock::Text { text: prompt }],
        }],
        temperature: Some(0.3),
        max_tokens: Some(800),
        ..Default::default()
    };

    let mut stream = ai_provider
        .chat_stream(req)
        .await
        .map_err(|e| AppError::internal_error(format!("summary stream: {e}")))?;
    let mut summary_text = String::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::internal_error(format!("stream chunk: {e}")))?;
        for delta in &chunk.content {
            if let ai_providers::ContentBlockDelta::TextDelta { delta, .. } = delta {
                summary_text.push_str(delta);
            }
        }
    }
    let summary_text = summary_text.trim().to_string();
    if summary_text.is_empty() {
        tracing::warn!(
            "memory.summarizer: empty summary returned for branch {} — skipping write",
            branch_id
        );
        return Ok(());
    }

    let pool = Repos.memory.pool_clone();
    let message_count_i32 = to_summarize.len() as i32;
    sqlx::query!(
        r#"
        INSERT INTO conversation_summaries
            (branch_id, summary_text, summarized_up_to_id, message_count, model_used)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (branch_id) DO UPDATE
        SET summary_text = EXCLUDED.summary_text,
            summarized_up_to_id = EXCLUDED.summarized_up_to_id,
            message_count = EXCLUDED.message_count,
            model_used = EXCLUDED.model_used,
            updated_at = NOW()
        "#,
        branch_id,
        summary_text,
        summarized_up_to_id,
        message_count_i32,
        model.name
    )
    .execute(&pool)
    .await
    .map_err(AppError::database_error)?;

    tracing::info!(
        "memory.summarizer: refreshed summary for branch {} ({} messages, model={})",
        branch_id,
        to_summarize.len(),
        model.name
    );
    Ok(())
}
