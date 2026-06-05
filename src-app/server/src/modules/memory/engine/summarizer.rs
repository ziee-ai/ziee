//! Conversation summarizer — when a branch exceeds N messages the
//! summarizer condenses the oldest messages into a single text block
//! stored in `conversation_summaries`. `apply_summary_to_history`
//! (called from `MemoryExtension::before_llm_call`) replaces those
//! summarized messages in the LLM request with the summary block,
//! freeing real prompt-side budget.
//!
//! Refresh strategy:
//!   - First call for a branch: FULL re-summarize of all messages
//!     older than `keep_recent`. Cost grows linearly with branch
//!     length.
//!   - Subsequent calls: INCREMENTAL — feed the LLM the previous
//!     summary + only the new turns since `summarized_up_to_id`. Cost
//!     stays bounded regardless of branch length (a 1000-turn chat
//!     pays the same per-refresh cost as a 60-turn chat).
//!   - Fallback to FULL when:
//!     • previous row has NULL `summarized_up_to_id` (legacy data),
//!     • the anchor message id is no longer in the current history
//!       (branch fork, deletion), or
//!     • `keep_recent` was raised between refreshes so the prior
//!       summary covers messages we now want to keep verbatim.
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

/// Default prompt for the FULL-summarize path. Used when
/// `memory_admin_settings.full_summary_prompt` is NULL. Exposed as
/// `pub` so the admin UI can render it as the placeholder.
pub const DEFAULT_FULL_SUMMARY_PROMPT: &str = r#"Summarize the following conversation into a concise narrative (3-6 sentences) capturing the essential context: who the user is, what they're trying to accomplish, key facts established, and any unresolved threads. Output only the summary text; no preamble.

Conversation:
{transcript}"#;

/// Default prompt for the INCREMENTAL-refresh path. Used when
/// `memory_admin_settings.incremental_summary_prompt` is NULL.
pub const DEFAULT_INCREMENTAL_SUMMARY_PROMPT: &str = r#"You are maintaining a running summary of an ongoing conversation between a user and an assistant.

An EXISTING summary is below. Additional conversation turns have happened since. Produce an UPDATED summary (3-6 sentences) that:
- Preserves the essential context from the existing summary.
- Incorporates relevant new facts, goals, or unresolved threads from the new turns.
- Drops details from the existing summary that are no longer relevant given the new state.
- Keeps the same form (concise narrative, no preamble, no bullet lists).

Output only the updated summary text; no preamble, no commentary.

Existing summary:
{previous_summary}

New conversation turns since the existing summary:
{new_transcript}"#;

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

/// Minimal projection of a message used by the summarizer's pure
/// decision logic. Decouples `decide_summarize_action` from
/// `chat::core::repository::MessageWithContent` so unit tests can
/// construct fakes without dragging the full chat history graph in.
#[derive(Debug, Clone)]
pub struct SummarizableMessage {
    pub id: Uuid,
    pub role: String,
    /// Concatenated text content. Empty if the message had only
    /// non-text content (tool calls, file attachments) — such messages
    /// still count toward the trigger and the `message_count` field,
    /// but contribute nothing to the transcript fed to the LLM.
    pub text: String,
}

/// The outcome of `decide_summarize_action`. Carries everything the
/// caller needs to either skip, full-summarize, or incremental-refresh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SummarizeAction {
    Noop,
    Full {
        transcript: String,
        summarized_up_to_id: Uuid,
        message_count: i32,
    },
    Incremental {
        previous_summary: String,
        new_transcript: String,
        summarized_up_to_id: Uuid,
        message_count: i32,
    },
}

/// Pure decision logic. Given a chronologically-ordered slice of
/// user/assistant messages, the configured trigger / keep_recent
/// thresholds, and the previously-persisted summary (if any), decide
/// what (if anything) to do.
///
/// All branches are unit-tested in `tests` below. The function is
/// pure (no I/O, no clock, no DB) so the tests are fast and stable.
pub fn decide_summarize_action(
    msgs: &[SummarizableMessage],
    trigger: usize,
    keep_recent: usize,
    existing: Option<&ConversationSummary>,
) -> SummarizeAction {
    if msgs.len() <= trigger {
        return SummarizeAction::Noop;
    }
    let cutoff = msgs.len().saturating_sub(keep_recent);
    if cutoff == 0 {
        return SummarizeAction::Noop;
    }
    let to_summarize = &msgs[..cutoff];
    let Some(last) = to_summarize.last() else {
        return SummarizeAction::Noop;
    };
    let summarized_up_to_id = last.id;
    let message_count = to_summarize.len() as i32;

    // Try the incremental path. Three conditions must hold:
    //   1. We have a previous summary,
    //   2. with a non-NULL anchor message id,
    //   3. and the previous summary doesn't already cover messages we
    //      now want to keep verbatim (i.e., keep_recent wasn't raised).
    // Plus the anchor must still be present in the current history.
    if let Some(prev) = existing {
        if let Some(prev_anchor_id) = prev.summarized_up_to_id {
            let prev_count = prev.message_count as usize;
            if prev_count <= to_summarize.len() {
                if let Some(prev_idx) =
                    to_summarize.iter().position(|m| m.id == prev_anchor_id)
                {
                    let new_msgs = &to_summarize[prev_idx + 1..];
                    if new_msgs.is_empty() {
                        // Summary already covers everything we'd
                        // currently summarize. No new content arrived
                        // between the prior refresh and now (or the
                        // only new content is in keep_recent).
                        return SummarizeAction::Noop;
                    }
                    let new_transcript = build_transcript(new_msgs);
                    if new_transcript.is_empty() {
                        // All new messages are non-text (tool-only).
                        // Skip — there's nothing for the LLM to fold
                        // into the summary.
                        return SummarizeAction::Noop;
                    }
                    return SummarizeAction::Incremental {
                        previous_summary: prev.summary_text.clone(),
                        new_transcript,
                        summarized_up_to_id,
                        message_count,
                    };
                }
                // Anchor not in current history — branch fork, deletion,
                // or a `summarized_up_to_id` from a different branch.
                // Fall through to Full path.
                tracing::info!(
                    "memory.summarizer: previous anchor {prev_anchor_id} not in branch history; falling back to full re-summarize"
                );
            } else {
                // Previous summary covered MORE messages than we now
                // want to summarize (admin raised keep_recent). The
                // summary's content is "ahead" of the new cutoff —
                // safest path is a full re-summarize from scratch.
                tracing::info!(
                    "memory.summarizer: prev.message_count={prev_count} > current to_summarize.len()={}; falling back to full re-summarize",
                    to_summarize.len()
                );
            }
        }
        // else: NULL anchor (legacy data pre-incremental refactor) —
        // fall through to Full so the next row gets a proper anchor.
    }

    // Full path.
    let transcript = build_transcript(to_summarize);
    if transcript.is_empty() {
        return SummarizeAction::Noop;
    }
    SummarizeAction::Full {
        transcript,
        summarized_up_to_id,
        message_count,
    }
}

/// Build a `role: text\n` transcript from a slice of messages. Skips
/// messages whose `text` is empty (non-text-only content). Pure.
pub fn build_transcript(msgs: &[SummarizableMessage]) -> String {
    let mut s = String::new();
    for m in msgs {
        if !m.text.is_empty() {
            s.push_str(&m.role);
            s.push_str(": ");
            s.push_str(&m.text);
            s.push('\n');
        }
    }
    s
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
pub async fn apply_summary_to_history(
    branch_id: Uuid,
    chat_request: &mut ChatRequest,
) -> Result<(), AppError> {
    let pool = Repos.memory.pool_clone();
    let summary = match fetch_summary(&pool, branch_id).await? {
        Some(s) => s,
        None => return Ok(()),
    };
    apply_summary_block(&summary, chat_request);
    Ok(())
}

/// Pure mutation: drop the summarized prefix and insert the summary
/// block. Split out from `apply_summary_to_history` so unit tests can
/// drive it directly with a fake `ConversationSummary`.
///
/// Pruning algorithm: chat_request.messages is built as
///   `[System*, User|Assistant*]`
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
pub fn apply_summary_block(summary: &ConversationSummary, chat_request: &mut ChatRequest) {
    let system_prefix_len = chat_request
        .messages
        .iter()
        .take_while(|m| matches!(m.role, Role::System))
        .count();

    let raw_drop_until = system_prefix_len.saturating_add(summary.message_count as usize);
    let drop_until = raw_drop_until.min(chat_request.messages.len());

    if drop_until > system_prefix_len {
        chat_request.messages.drain(system_prefix_len..drop_until);
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
}

/// Generate / refresh the persisted summary for this branch. Picks
/// FULL or INCREMENTAL path based on `decide_summarize_action`.
/// Idempotent at the row level via `ON CONFLICT (branch_id) DO UPDATE`.
pub async fn refresh_summary(
    branch_id: Uuid,
    summarization_model_id: Uuid,
) -> Result<(), AppError> {
    let history = Repos.chat.core.get_conversation_history(branch_id).await?;
    let msgs: Vec<SummarizableMessage> = history
        .iter()
        .filter(|m| m.message.role == "user" || m.message.role == "assistant")
        .map(message_to_summarizable)
        .collect();

    // Read thresholds + prompt overrides fresh — admin tuning takes
    // effect on the next summarization without a restart. Defaults
    // back to the compiled-in constants when the row is NULL or read
    // fails.
    let (trigger, keep_recent, full_prompt, incremental_prompt) =
        match Repos.memory.get_admin_settings().await {
            Ok(s) => (
                s.summarize_after_n_messages as usize,
                s.summarizer_keep_recent as usize,
                s.full_summary_prompt
                    .unwrap_or_else(|| DEFAULT_FULL_SUMMARY_PROMPT.to_string()),
                s.incremental_summary_prompt
                    .unwrap_or_else(|| DEFAULT_INCREMENTAL_SUMMARY_PROMPT.to_string()),
            ),
            Err(e) => {
                tracing::warn!(
                    "memory.summarizer: get_admin_settings failed ({e}); using compiled-in defaults"
                );
                (
                    FALLBACK_SUMMARIZE_AFTER_N_MESSAGES,
                    FALLBACK_KEEP_RECENT,
                    DEFAULT_FULL_SUMMARY_PROMPT.to_string(),
                    DEFAULT_INCREMENTAL_SUMMARY_PROMPT.to_string(),
                )
            }
        };

    let pool = Repos.memory.pool_clone();
    let existing = fetch_summary(&pool, branch_id).await?;

    let action = decide_summarize_action(&msgs, trigger, keep_recent, existing.as_ref());
    if matches!(action, SummarizeAction::Noop) {
        return Ok(());
    }

    // Load + capability-check the model BEFORE building the prompt — an
    // embedding model can't generate (served `--embeddings`, no logits),
    // so skip early rather than do prompt work for a doomed call. Mirrors
    // the early guard in `extractor::run`. See `engine::capability`.
    let model = Repos
        .llm_model
        .get_by_id(summarization_model_id)
        .await
        .map_err(AppError::database_error)?
        .ok_or_else(|| AppError::not_found("LlmModel"))?;
    if let Some(reason) =
        super::capability::generation_unsupported_reason(&model.name, &model.capabilities)
    {
        tracing::warn!("memory.summarizer: {reason} — skipping summarization");
        return Ok(());
    }

    let (prompt, summarized_up_to_id, message_count, mode) = match action {
        // Already handled above; kept for match exhaustiveness.
        SummarizeAction::Noop => return Ok(()),
        SummarizeAction::Full {
            transcript,
            summarized_up_to_id,
            message_count,
        } => (
            full_prompt.replace("{transcript}", &transcript),
            summarized_up_to_id,
            message_count,
            "full",
        ),
        SummarizeAction::Incremental {
            previous_summary,
            new_transcript,
            summarized_up_to_id,
            message_count,
        } => (
            incremental_prompt
                .replace("{previous_summary}", &previous_summary)
                .replace("{new_transcript}", &new_transcript),
            summarized_up_to_id,
            message_count,
            "incremental",
        ),
    };

    let summary_text = call_summarization_llm(&model, prompt).await?;
    if summary_text.is_empty() {
        tracing::warn!(
            "memory.summarizer: empty {mode} summary returned for branch {branch_id} — skipping write"
        );
        return Ok(());
    }

    upsert_summary(
        &pool,
        branch_id,
        &summary_text,
        Some(summarized_up_to_id),
        message_count,
        &model.name,
    )
    .await?;

    tracing::info!(
        "memory.summarizer: {mode} refresh for branch {branch_id} ({message_count} total summarized, model={})",
        model.name
    );
    Ok(())
}

fn message_to_summarizable(
    m: &crate::modules::chat::core::types::MessageWithContent,
) -> SummarizableMessage {
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
    SummarizableMessage {
        id: m.message.id,
        role: m.message.role.clone(),
        text,
    }
}

async fn call_summarization_llm(
    model: &crate::modules::llm_model::models::LlmModel,
    prompt: String,
) -> Result<String, AppError> {
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
    Ok(summary_text.trim().to_string())
}

async fn upsert_summary(
    pool: &PgPool,
    branch_id: Uuid,
    summary_text: &str,
    summarized_up_to_id: Option<Uuid>,
    message_count: i32,
    model_name: &str,
) -> Result<(), AppError> {
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
        message_count,
        model_name
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

// ============================================================================
// Unit tests — pure logic only. No DB, no LLM, no clock.
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn msg(id: Uuid, role: &str, text: &str) -> SummarizableMessage {
        SummarizableMessage {
            id,
            role: role.to_string(),
            text: text.to_string(),
        }
    }

    fn fake_summary(
        anchor: Option<Uuid>,
        message_count: i32,
        text: &str,
    ) -> ConversationSummary {
        ConversationSummary {
            branch_id: Uuid::nil(),
            summary_text: text.to_string(),
            summarized_up_to_id: anchor,
            message_count,
            model_used: Some("test-model".to_string()),
            created_at: Utc.timestamp_opt(0, 0).unwrap(),
            updated_at: Utc.timestamp_opt(0, 0).unwrap(),
        }
    }

    // ---- decide_summarize_action ----

    #[test]
    fn decide_below_trigger_is_noop() {
        let msgs: Vec<_> = (0..5)
            .map(|i| msg(Uuid::new_v4(), "user", &format!("m{i}")))
            .collect();
        assert_eq!(
            decide_summarize_action(&msgs, 10, 3, None),
            SummarizeAction::Noop
        );
    }

    #[test]
    fn decide_at_trigger_is_noop() {
        // Boundary: `msgs.len() == trigger` is NOT enough to fire.
        let msgs: Vec<_> = (0..10)
            .map(|i| msg(Uuid::new_v4(), "user", &format!("m{i}")))
            .collect();
        assert_eq!(
            decide_summarize_action(&msgs, 10, 3, None),
            SummarizeAction::Noop
        );
    }

    #[test]
    fn decide_above_trigger_no_existing_returns_full() {
        let msgs: Vec<_> = (0..12)
            .map(|i| {
                msg(
                    Uuid::new_v4(),
                    if i % 2 == 0 { "user" } else { "assistant" },
                    &format!("m{i}"),
                )
            })
            .collect();
        let action = decide_summarize_action(&msgs, 10, 3, None);
        match action {
            SummarizeAction::Full {
                transcript,
                summarized_up_to_id,
                message_count,
            } => {
                // cutoff = 12 - 3 = 9 → summarize msgs[0..9]
                assert_eq!(message_count, 9);
                assert_eq!(summarized_up_to_id, msgs[8].id);
                // Transcript contains all 9 messages, one line each.
                assert_eq!(transcript.lines().count(), 9);
                assert!(transcript.contains("m0"));
                assert!(transcript.contains("m8"));
                assert!(!transcript.contains("m9")); // kept verbatim
            }
            other => panic!("expected Full, got {other:?}"),
        }
    }

    #[test]
    fn decide_incremental_with_valid_anchor() {
        let msgs: Vec<_> = (0..15)
            .map(|i| msg(Uuid::new_v4(), "user", &format!("m{i}")))
            .collect();
        // Previous summary anchored at m5 (covered 6 msgs: 0..=5).
        let prev = fake_summary(Some(msgs[5].id), 6, "old summary");
        let action = decide_summarize_action(&msgs, 10, 3, Some(&prev));
        match action {
            SummarizeAction::Incremental {
                previous_summary,
                new_transcript,
                summarized_up_to_id,
                message_count,
            } => {
                // cutoff = 15 - 3 = 12 → summarize msgs[0..12]
                // new = msgs[6..12] (6 messages)
                assert_eq!(message_count, 12);
                assert_eq!(summarized_up_to_id, msgs[11].id);
                assert_eq!(previous_summary, "old summary");
                let lines: Vec<_> = new_transcript.lines().collect();
                assert_eq!(lines.len(), 6);
                assert!(lines[0].contains("m6"));
                assert!(lines[5].contains("m11"));
                assert!(!new_transcript.contains("m5"));
                assert!(!new_transcript.contains("m12"));
            }
            other => panic!("expected Incremental, got {other:?}"),
        }
    }

    #[test]
    fn decide_anchor_at_cutoff_minus_one_is_noop() {
        // Anchor is the LAST message that would be summarized → no
        // new content to fold into the summary → Noop.
        let msgs: Vec<_> = (0..12)
            .map(|i| msg(Uuid::new_v4(), "user", &format!("m{i}")))
            .collect();
        // cutoff = 9, so anchor = msgs[8] means new_msgs is empty.
        let prev = fake_summary(Some(msgs[8].id), 9, "old summary");
        assert_eq!(
            decide_summarize_action(&msgs, 10, 3, Some(&prev)),
            SummarizeAction::Noop
        );
    }

    #[test]
    fn decide_anchor_not_in_history_falls_back_to_full() {
        let msgs: Vec<_> = (0..12)
            .map(|i| msg(Uuid::new_v4(), "user", &format!("m{i}")))
            .collect();
        // Anchor refers to a message that doesn't exist in this
        // branch's history (branch fork, deletion).
        let stale_anchor = Uuid::new_v4();
        let prev = fake_summary(Some(stale_anchor), 5, "old summary");
        let action = decide_summarize_action(&msgs, 10, 3, Some(&prev));
        assert!(matches!(action, SummarizeAction::Full { .. }));
    }

    #[test]
    fn decide_null_anchor_falls_back_to_full() {
        let msgs: Vec<_> = (0..12)
            .map(|i| msg(Uuid::new_v4(), "user", &format!("m{i}")))
            .collect();
        // Legacy row with no anchor → Full path so the new row gets one.
        let prev = fake_summary(None, 5, "old summary");
        let action = decide_summarize_action(&msgs, 10, 3, Some(&prev));
        assert!(matches!(action, SummarizeAction::Full { .. }));
    }

    #[test]
    fn decide_keep_recent_expanded_falls_back_to_full() {
        // Previously keep_recent was 3 → summary covered 9 of 12 msgs.
        // Now admin raised keep_recent to 6 → we'd want to summarize
        // only 9 - 3 = 6 msgs, but prev summary covers 9. Fall back.
        let msgs: Vec<_> = (0..12)
            .map(|i| msg(Uuid::new_v4(), "user", &format!("m{i}")))
            .collect();
        let prev = fake_summary(Some(msgs[8].id), 9, "old summary");
        let action = decide_summarize_action(&msgs, 10, 6, Some(&prev));
        assert!(matches!(action, SummarizeAction::Full { .. }));
    }

    #[test]
    fn decide_incremental_with_non_text_new_msgs_is_noop() {
        let msgs: Vec<_> = (0..12)
            .map(|i| {
                // Last 3 messages (after anchor) have empty text →
                // nothing for the LLM to fold in.
                let text = if i < 9 { format!("m{i}") } else { String::new() };
                msg(Uuid::new_v4(), "user", &text)
            })
            .collect();
        // Anchor = msgs[7] (covered first 8 msgs). cutoff=9.
        // new_msgs = msgs[8..9] = 1 msg, but its text is "" (i=8 has
        // text="m8", let me recompute) — actually i=8 has text="m8",
        // so new transcript is not empty. Let me adjust: anchor at
        // msgs[8] so new_msgs = msgs[9..9] is empty? No, cutoff is 9
        // so to_summarize=msgs[..9], new=msgs[9..9] is empty.
        //
        // Re-aim: anchor at msgs[7] → new = msgs[8..9] = [msg8].
        // msg8 has text "m8", so new transcript is non-empty. To get
        // empty new_transcript I need msg8 to have empty text. Done
        // above (i<9 vs i>=9: msg8 falls in "<9", so text="m8" not "").
        // Adjust the cutoff so the new range has only the empty-text
        // messages.
        let prev = fake_summary(Some(msgs[8].id), 9, "old summary");
        // to_summarize = msgs[0..9], new = msgs[9..9] = empty → Noop
        // (the "anchor at cutoff-1" case, covered above). Pick a
        // different anchor to hit the all-non-text-new case.
        //
        // Actually a cleaner setup: use a single empty-text msg between
        // anchor and cutoff.
        let mut msgs2 = msgs.clone();
        msgs2[8].text = String::new(); // make msg8 empty-text
        let prev2 = fake_summary(Some(msgs2[7].id), 8, "old summary");
        // to_summarize = msgs2[0..9], new = msgs2[8..9] = [empty-text
        // msg]. new_transcript is empty → Noop.
        assert_eq!(
            decide_summarize_action(&msgs2, 10, 3, Some(&prev2)),
            SummarizeAction::Noop
        );
        let _ = prev; // silence unused
    }

    // ---- build_transcript ----

    #[test]
    fn transcript_text_only() {
        let m = vec![
            msg(Uuid::new_v4(), "user", "hello"),
            msg(Uuid::new_v4(), "assistant", "hi there"),
        ];
        assert_eq!(build_transcript(&m), "user: hello\nassistant: hi there\n");
    }

    #[test]
    fn transcript_skips_empty_text() {
        let m = vec![
            msg(Uuid::new_v4(), "user", "hello"),
            msg(Uuid::new_v4(), "assistant", ""), // tool-only turn
            msg(Uuid::new_v4(), "user", "world"),
        ];
        assert_eq!(build_transcript(&m), "user: hello\nuser: world\n");
    }

    #[test]
    fn transcript_empty_when_no_text() {
        let m = vec![
            msg(Uuid::new_v4(), "user", ""),
            msg(Uuid::new_v4(), "assistant", ""),
        ];
        assert_eq!(build_transcript(&m), "");
    }

    // ---- apply_summary_block ----

    fn user_msg(text: &str) -> ChatMessage {
        ChatMessage {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
        }
    }

    fn asst_msg(text: &str) -> ChatMessage {
        ChatMessage {
            role: Role::Assistant,
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
        }
    }

    fn sys_msg(text: &str) -> ChatMessage {
        ChatMessage {
            role: Role::System,
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
        }
    }

    fn request_text(req: &ChatRequest, idx: usize) -> &str {
        match &req.messages[idx].content[0] {
            ContentBlock::Text { text } => text.as_str(),
            _ => panic!("expected text content"),
        }
    }

    #[test]
    fn apply_block_drains_and_inserts_after_system_prefix() {
        let mut req = ChatRequest {
            model: "x".into(),
            messages: vec![
                sys_msg("primary instructions"),
                user_msg("m0"),
                asst_msg("m1"),
                user_msg("m2"),
                asst_msg("m3"),
                user_msg("m4"), // kept (keep_recent style)
                asst_msg("m5"),
            ],
            ..Default::default()
        };
        let s = fake_summary(Some(Uuid::new_v4()), 4, "condensed");
        apply_summary_block(&s, &mut req);

        // Expected order: [System primary, System summary, m4, m5]
        assert_eq!(req.messages.len(), 4);
        assert!(matches!(req.messages[0].role, Role::System));
        assert_eq!(request_text(&req, 0), "primary instructions");
        assert!(matches!(req.messages[1].role, Role::System));
        assert!(request_text(&req, 1).contains("condensed"));
        assert!(request_text(&req, 1).contains("4 messages condensed"));
        assert_eq!(request_text(&req, 2), "m4");
        assert_eq!(request_text(&req, 3), "m5");
    }

    #[test]
    fn apply_block_no_system_prefix() {
        let mut req = ChatRequest {
            model: "x".into(),
            messages: vec![user_msg("m0"), asst_msg("m1"), user_msg("m2")],
            ..Default::default()
        };
        let s = fake_summary(Some(Uuid::new_v4()), 2, "condensed");
        apply_summary_block(&s, &mut req);
        // Expected: [System summary, m2]
        assert_eq!(req.messages.len(), 2);
        assert!(matches!(req.messages[0].role, Role::System));
        assert_eq!(request_text(&req, 1), "m2");
    }

    #[test]
    fn apply_block_clamps_overflow_drain() {
        let mut req = ChatRequest {
            model: "x".into(),
            messages: vec![user_msg("only-one-msg")],
            ..Default::default()
        };
        // Summary claims it covers 100 messages but request has only 1.
        let s = fake_summary(Some(Uuid::new_v4()), 100, "condensed");
        apply_summary_block(&s, &mut req);
        // Expected: drain clamped to len(1), then insert summary at 0.
        // Result: [System summary] — the one user msg got drained.
        assert_eq!(req.messages.len(), 1);
        assert!(matches!(req.messages[0].role, Role::System));
        assert!(request_text(&req, 0).contains("condensed"));
    }

    #[test]
    fn apply_block_message_count_zero_just_inserts() {
        let mut req = ChatRequest {
            model: "x".into(),
            messages: vec![sys_msg("primary"), user_msg("m0"), user_msg("m1")],
            ..Default::default()
        };
        let s = fake_summary(None, 0, "condensed");
        apply_summary_block(&s, &mut req);
        // Expected: drain 0 messages, insert summary after system prefix.
        // Result: [System primary, System summary, m0, m1]
        assert_eq!(req.messages.len(), 4);
        assert!(matches!(req.messages[0].role, Role::System));
        assert!(matches!(req.messages[1].role, Role::System));
        assert!(request_text(&req, 1).contains("condensed"));
        assert_eq!(request_text(&req, 2), "m0");
        assert_eq!(request_text(&req, 3), "m1");
    }
}
