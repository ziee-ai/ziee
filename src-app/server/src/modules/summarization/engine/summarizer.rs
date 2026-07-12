//! Conversation summarizer — when a branch exceeds N messages the
//! summarizer condenses the oldest messages into a single text block
//! stored in `conversation_summaries`. `apply_summary_to_history`
//! (called from `SummarizationExtension::before_llm_call`) replaces
//! those summarized messages in the LLM request with the summary
//! block, freeing real prompt-side budget.
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
//! Trigger lives in `SummarizationExtension::after_llm_call`
//! (fire-and-forget spawn). Threshold + keep-recent come from
//! `summarization_admin_settings` (admin-tunable, no restart needed).
//!
//! Concurrent-refresh race: two simultaneous turns on the same branch
//! could each spawn their own `refresh_summary`. Last-write-wins is
//! INTENTIONAL — this is an approximate rolling summary; the
//! authoritative content is always the message history itself.
//! A `>=message_count` UPSERT guard would block a legitimate
//! `keep_recent`-raise shrink, so we accept the cosmetic race.
//!
//! Unit tests (`#[cfg(test)] mod tests` at the bottom) cover the
//! pure decision logic + transcript assembly + summary-block apply.
//! Count: 19 tests (decide × 13, build/apply × 6).

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
/// column DEFAULTs in migration 91. The runtime values come from
/// `summarization_admin_settings.summarize_after_tokens` /
/// `.summarizer_keep_recent_tokens` and can be tuned per-deployment from the
/// admin UI without a redeploy.
const FALLBACK_SUMMARIZE_AFTER_TOKENS: usize = 12000;
const FALLBACK_KEEP_RECENT_TOKENS: usize = 3000;

/// Default prompt for the FULL-summarize path. Used when
/// `summarization_admin_settings.full_summary_prompt` is NULL. Exposed as
/// `pub` so the admin UI can render it as the placeholder.
pub const DEFAULT_FULL_SUMMARY_PROMPT: &str = r#"Summarize the following conversation into a concise narrative (3-6 sentences) capturing the essential context: who the user is, what they're trying to accomplish, key facts established, and any unresolved threads. Output only the summary text; no preamble.

Conversation:
{transcript}"#;

/// Default prompt for the INCREMENTAL-refresh path. Used when
/// `summarization_admin_settings.incremental_summary_prompt` is NULL.
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

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, schemars::JsonSchema)]
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
/// Apply the fraction-of-window override to the `(trigger, keep_recent)` token
/// pair before deciding whether to summarize.
///
/// - `trigger` is clamped to the SMALLER of the admin cap and the override
///   (`0.75 × the model's context window`, when known).
/// - `keep_recent` is then re-clamped strictly below the (possibly lowered)
///   trigger. Without this, a small-context override below `keep_recent` would
///   leave `keep_recent >= trigger`, which silently disables summarization (the
///   keep-recent loop never breaks → cutoff walks to 0 → Noop).
///
/// Pure (no I/O) so it's unit-tested in `tests` below.
pub(crate) fn apply_window_override(
    trigger: usize,
    keep_recent: usize,
    override_: Option<usize>,
) -> (usize, usize) {
    let trigger = match override_ {
        Some(o) => trigger.min(o),
        None => trigger,
    };
    let keep_recent = keep_recent.min(trigger.saturating_sub(1));
    (trigger, keep_recent)
}

/// All branches are unit-tested in `tests` below. The function is
/// pure (no I/O, no clock, no DB) so the tests are fast and stable.
pub fn decide_summarize_action(
    msgs: &[SummarizableMessage],
    trigger_tokens: usize,
    keep_recent_tokens: usize,
    existing: Option<&ConversationSummary>,
) -> SummarizeAction {
    // Token-aware (chars/4): summarize once the branch's text exceeds
    // `trigger_tokens`, keeping the newest message-boundary suffix that fits in
    // `keep_recent_tokens` verbatim. The cutoff stays on a message boundary, so
    // the message-id anchor + incremental refresh are unchanged — only the
    // trigger/cutoff arithmetic moved from message counts to estimated tokens.
    let total_tokens: usize = msgs
        .iter()
        .map(|m| crate::common::tokens::estimate_tokens(&m.text))
        .sum();
    if total_tokens <= trigger_tokens {
        return SummarizeAction::Noop;
    }
    let mut acc = 0usize;
    let mut cutoff = msgs.len();
    for i in (0..msgs.len()).rev() {
        let t = crate::common::tokens::estimate_tokens(&msgs[i].text);
        // Always keep the newest message; then keep older ones while under budget.
        if cutoff < msgs.len() && acc + t > keep_recent_tokens {
            break;
        }
        acc += t;
        cutoff = i;
    }
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
                    "summarization: previous anchor {prev_anchor_id} not in branch history; falling back to full re-summarize"
                );
            } else {
                // Previous summary covered MORE messages than we now
                // want to summarize (admin raised keep_recent). The
                // summary's content is "ahead" of the new cutoff —
                // safest path is a full re-summarize from scratch.
                tracing::info!(
                    "summarization: prev.message_count={prev_count} > current to_summarize.len()={}; falling back to full re-summarize",
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

/// Single-pass substitution of the incremental-summary prompt's two
/// placeholders. Sequential `String::replace` calls are unsafe here:
/// if a previously-LLM-generated `previous_summary` ever contained the
/// literal text `{new_transcript}` (an admin's persona prompt could
/// coax this out of a malicious user's branch), the second call would
/// re-substitute and leak transcript content into the "earlier context"
/// slot. Single-pass scanning never revisits substituted text, so
/// neither placeholder can match content the other inserted.
/// Render the FULL-summary prompt: substitute `{transcript}` in the (possibly
/// admin-customized) template. The template is whatever
/// `summarization_admin_settings.full_summary_prompt` holds, falling back to
/// `DEFAULT_FULL_SUMMARY_PROMPT`, so this is the point where a CUSTOM full
/// prompt becomes the text sent to the LLM.
fn render_full_prompt(template: &str, transcript: &str) -> String {
    template.replace("{transcript}", transcript)
}

fn render_incremental_prompt(
    template: &str,
    previous_summary: &str,
    new_transcript: &str,
) -> String {
    const PREV: &str = "{previous_summary}";
    const NEW: &str = "{new_transcript}";
    let mut out = String::with_capacity(
        template.len() + previous_summary.len() + new_transcript.len(),
    );
    let mut rest = template;
    while !rest.is_empty() {
        let next_prev = rest.find(PREV);
        let next_new = rest.find(NEW);
        let (cut, replacement, placeholder_len) = match (next_prev, next_new) {
            (None, None) => {
                out.push_str(rest);
                break;
            }
            (Some(p), None) => (p, previous_summary, PREV.len()),
            (None, Some(n)) => (n, new_transcript, NEW.len()),
            (Some(p), Some(n)) if p <= n => (p, previous_summary, PREV.len()),
            (Some(_), Some(n)) => (n, new_transcript, NEW.len()),
        };
        out.push_str(&rest[..cut]);
        out.push_str(replacement);
        rest = &rest[cut + placeholder_len..];
    }
    out
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
    let pool = Repos.summarization.pool_clone();
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
/// run before us). Extensions execute in ascending-order order, so
/// summarization (order 24) runs before memory (order 25); memory's
/// later prepend therefore lands ABOVE summarization's block in the
/// final array.
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
    let mut drop_until = raw_drop_until.min(chat_request.messages.len());

    // `message_count` counts DB messages, but the outbound array was already split
    // by `group_assistant_blocks` — one assistant DB message becomes an
    // `[Assistant { tool_use }, Tool { tool_result }]` pair per tool round-trip. A
    // raw count-based cut can therefore land BETWEEN an assistant tool_use turn and
    // its Tool result turn, leaving the retained history starting on an orphan
    // `tool_result` (every provider 400s on a tool_result with no preceding
    // tool_use). Snap the cut FORWARD past any leading Tool message(s): a
    // retained-leading `Role::Tool` is always an orphan whose tool_use is in the
    // dropped prefix (grouping always emits the Tool turn immediately after its
    // Assistant tool_use turn), so dropping it too — the summary text condenses it
    // anyway — is the correct, provider-agnostic fix.
    // Only snap when we are actually dropping a prefix (`drop_until >
    // system_prefix_len`): a `message_count` of 0 means "insert the summary, drop
    // nothing", so a (hypothetical) leading Tool must not be dropped — there is no
    // dropped tool_use for it to be orphaned by.
    while drop_until > system_prefix_len
        && drop_until < chat_request.messages.len()
        && matches!(chat_request.messages[drop_until].role, Role::Tool)
    {
        drop_until += 1;
    }

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
    // Fraction-of-window override (0.75× the chat model's context window). When
    // set, the effective trigger is `min(admin.summarize_after_tokens, override)`
    // so a small-context model summarizes before it overflows.
    trigger_override: Option<usize>,
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
        match Repos.summarization.get_admin_settings().await {
            Ok(s) => (
                s.summarize_after_tokens as usize,
                s.summarizer_keep_recent_tokens as usize,
                s.full_summary_prompt
                    .unwrap_or_else(|| DEFAULT_FULL_SUMMARY_PROMPT.to_string()),
                s.incremental_summary_prompt
                    .unwrap_or_else(|| DEFAULT_INCREMENTAL_SUMMARY_PROMPT.to_string()),
            ),
            Err(e) => {
                tracing::warn!(
                    "summarization: get_admin_settings failed ({e}); using compiled-in defaults"
                );
                (
                    FALLBACK_SUMMARIZE_AFTER_TOKENS,
                    FALLBACK_KEEP_RECENT_TOKENS,
                    DEFAULT_FULL_SUMMARY_PROMPT.to_string(),
                    DEFAULT_INCREMENTAL_SUMMARY_PROMPT.to_string(),
                )
            }
        };

    // Apply the fraction-of-window override: summarize at the SMALLER of the
    // admin cap and 0.75× the model's context window, re-clamping keep_recent
    // so a small-context override can't silently disable summarization.
    let (trigger, keep_recent) = apply_window_override(trigger, keep_recent, trigger_override);

    let pool = Repos.summarization.pool_clone();
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
    // The capability guard lives in `memory::engine::capability` (added
    // by khoi's pre-extraction commit, before summarization moved out
    // of memory). Reach across modules until a follow-up promotes it to
    // a shared location.
    if let Some(reason) = crate::modules::memory::engine::capability::generation_unsupported_reason(
        &model.name,
        &model.capabilities,
    ) {
        tracing::warn!("summarization: {reason} — skipping summarization");
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
            render_full_prompt(&full_prompt, &transcript),
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
            render_incremental_prompt(
                &incremental_prompt,
                &previous_summary,
                &new_transcript,
            ),
            summarized_up_to_id,
            message_count,
            "incremental",
        ),
    };

    let summary_text = call_summarization_llm(&model, prompt).await?;
    if !summary_is_writable(&summary_text) {
        tracing::warn!(
            "summarization: empty {mode} summary returned for branch {branch_id} — skipping write"
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
        "summarization: {mode} refresh for branch {branch_id} ({message_count} total summarized, model={})",
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

/// Call the configured LLM to generate a summary.
///
/// **Known limitation — system-key only.** This path reads
/// `provider.api_key` directly and does NOT route through
/// `chat::core::ai_provider::resolve_api_key_for_user`. Chat-time
/// requests honour a user's personal `user_llm_provider_api_keys`
/// override; summarization does not. On a deployment where the
/// `default_summarization_model_id` lives on a provider whose system
/// `api_key` is NULL (per-user-keys-only deployments), summarization
/// will silently 401 against the provider and the user will see no
/// summary marker ever appear. The fail-soft `tracing::warn` in
/// `after_llm_call` is the only signal. Plumbing `user_id` into
/// `refresh_summary` so it can call `resolve_api_key_for_user` is the
/// follow-up; tracked alongside memory's `embedding_worker` which has
/// the same constraint.
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

    // Retry the stream INIT a few times with backoff — summarization is a
    // best-effort background task, so a transient provider blip shouldn't lose
    // the summary outright.
    let mut stream = {
        const MAX_ATTEMPTS: u32 = 3;
        let mut attempt = 1u32;
        loop {
            match ai_provider.chat_stream(req.clone()).await {
                Ok(s) => break s,
                Err(e) if attempt < MAX_ATTEMPTS => {
                    tracing::warn!(
                        "summarization: stream init failed (attempt {attempt}/{MAX_ATTEMPTS}): {e}; retrying"
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(
                        250 * attempt as u64,
                    ))
                    .await;
                    attempt += 1;
                }
                Err(e) => {
                    return Err(AppError::internal_error(format!("summary stream: {e}")));
                }
            }
        }
    };
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

/// Guard against persisting a degenerate (empty / whitespace-only) summary. An
/// LLM that returns no text (provider hiccup, refusal, all-whitespace) must not
/// overwrite a good prior summary with a blank one — `refresh_summary` skips the
/// write when this returns false.
fn summary_is_writable(summary_text: &str) -> bool {
    !summary_text.trim().is_empty()
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


    #[test]
    fn decide_is_token_based_not_message_count() {
        // 3 messages but two are LARGE (~250 tokens each). With a token trigger
        // of 100 this MUST summarize; the old message-COUNT logic (3 <= 100)
        // would no-op. Proves the trigger is token-based.
        let big = "x".repeat(1000); // ~250 est tokens
        let msgs = vec![
            msg(Uuid::new_v4(), "user", &big),
            msg(Uuid::new_v4(), "assistant", &big),
            msg(Uuid::new_v4(), "user", "ok"),
        ];
        assert!(
            !matches!(
                decide_summarize_action(&msgs, 100, 100, None),
                SummarizeAction::Noop
            ),
            "large-token messages must trigger even at a small message count"
        );
    }


    #[test]
    fn decide_keep_recent_is_a_token_budget() {
        // keep_recent_tokens large enough to hold everything verbatim → nothing
        // old enough to summarize → Noop, even though total exceeds the trigger.
        let big = "x".repeat(1000);
        let msgs = vec![
            msg(Uuid::new_v4(), "user", &big),
            msg(Uuid::new_v4(), "assistant", &big),
        ];
        assert!(matches!(
            decide_summarize_action(&msgs, 100, 100_000, None),
            SummarizeAction::Noop
        ));
    }


    #[test]
    fn decide_summarizes_after_keep_recent_clamped_below_trigger() {
        // Regression for B-correctness-02: a small-context override drops the
        // trigger below the default keep_recent. `refresh_summary` re-clamps
        // keep_recent to `trigger - 1`; with that clamp applied here the
        // branch MUST NOT Noop even though `total < the unclamped keep_recent`.
        //
        // Two ~250-token messages (total ~500). Override trigger = 200 (e.g.
        // 0.75 × a tiny context window), default keep_recent = 3000. Without
        // the clamp, keep_recent (3000) > total (500) → the keep-recent loop
        // never breaks, cutoff walks to 0, and the function Noops. With the
        // clamp keep_recent = trigger - 1 = 199, so the oldest message is
        // summarized.
        let big = "x".repeat(1000); // ~250 est tokens
        let msgs = vec![
            msg(Uuid::new_v4(), "user", &big),
            msg(Uuid::new_v4(), "assistant", &big),
        ];
        let trigger = 200usize;
        let keep_recent_clamped = trigger.saturating_sub(1); // mirrors refresh_summary
        assert!(
            !matches!(
                decide_summarize_action(&msgs, trigger, keep_recent_clamped, None),
                SummarizeAction::Noop
            ),
            "clamped keep_recent below the override trigger must still summarize"
        );
        // Sanity: the UNclamped large keep_recent would (wrongly) Noop —
        // proving the clamp is load-bearing.
        assert!(
            matches!(
                decide_summarize_action(&msgs, trigger, 3000, None),
                SummarizeAction::Noop
            ),
            "without the clamp, keep_recent > total disables summarization"
        );
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


    fn asst_tool_use_msg(id: &str, name: &str) -> ChatMessage {
        ChatMessage {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: id.to_string(),
                name: name.to_string(),
                input: serde_json::json!({}),
            }],
        }
    }

    fn tool_result_msg(id: &str) -> ChatMessage {
        ChatMessage {
            role: Role::Tool,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: id.to_string(),
                name: None,
                content: vec![ContentBlock::Text {
                    text: "r".to_string(),
                }],
                is_error: None,
            }],
        }
    }

    /// TEST-6: `message_count` counts DB messages, but the outbound array was split
    /// by `group_assistant_blocks` into `[Assistant{tool_use}, Tool{tool_result}]`
    /// pairs. A raw count-based cut can land BETWEEN an assistant tool_use and its
    /// Tool result, leaving the retained history starting on an orphan tool_result
    /// (every provider 400s on a tool_result with no preceding tool_use).
    /// `apply_summary_block` must snap the cut FORWARD past the orphan Tool turn.
    #[test]
    fn apply_block_snaps_cut_past_orphan_tool_result() {
        let mut req = ChatRequest {
            model: "x".into(),
            messages: vec![
                sys_msg("primary instructions"), // 0
                asst_tool_use_msg("A", "srv__a"), // 1
                tool_result_msg("A"),            // 2
                user_msg("m"),                   // 3
                asst_tool_use_msg("B", "srv__b"), // 4
                tool_result_msg("B"),            // 5
                user_msg("keep"),                // 6
            ],
            ..Default::default()
        };
        // message_count = 1 → naive drain(1..2) removes only the Assistant tool_use A,
        // leaving Tool{result A} as the new leading message (an orphan). The snap must
        // advance the cut to also drop that Tool.
        let s = fake_summary(Some(Uuid::new_v4()), 1, "condensed");
        apply_summary_block(&s, &mut req);

        // Expected: [System primary, System summary, User m, Assistant B, Tool B, User keep]
        assert!(matches!(req.messages[0].role, Role::System));
        assert!(matches!(req.messages[1].role, Role::System));
        assert!(
            !matches!(req.messages[2].role, Role::Tool),
            "retained history must not start on an orphan tool_result; got {:?}",
            req.messages[2].role
        );
        assert!(matches!(req.messages[2].role, Role::User));
        assert_eq!(request_text(&req, 2), "m");
        // The B pair survives intact and validly paired.
        assert!(matches!(req.messages[3].role, Role::Assistant));
        assert!(matches!(req.messages[4].role, Role::Tool));
    }

    #[test]
    fn render_incremental_prompt_substitutes_once() {
        // Sanity: both placeholders interpolate exactly once each.
        let out = render_incremental_prompt(
            "PREV={previous_summary} NEW={new_transcript} END",
            "S1",
            "T1",
        );
        assert_eq!(out, "PREV=S1 NEW=T1 END");
    }


    #[test]
    fn render_incremental_prompt_does_not_re_substitute_inserted_content() {
        // The prompt-injection guard: a previous_summary that contains
        // the literal {new_transcript} placeholder must NOT cause the
        // new transcript to leak into the previous slot. Sequential
        // .replace() would fail this; the single-pass implementation
        // passes.
        let prev = "summary ends with {new_transcript} literal";
        let new_tx = "SECRET-NEW-TURNS";
        let out = render_incremental_prompt(
            "P={previous_summary}|N={new_transcript}",
            prev,
            new_tx,
        );
        // previous_summary slot keeps the literal `{new_transcript}`
        // text verbatim; only the explicit `{new_transcript}` outside
        // the previous slot gets substituted.
        assert_eq!(
            out,
            "P=summary ends with {new_transcript} literal|N=SECRET-NEW-TURNS"
        );
        // Belt-and-suspenders: the new transcript appears exactly once
        // in the rendered prompt.
        assert_eq!(out.matches(new_tx).count(), 1);
    }


    #[test]
    fn render_incremental_prompt_handles_no_placeholders() {
        // Template with neither placeholder is returned unchanged.
        let out = render_incremental_prompt("nothing to substitute", "S", "T");
        assert_eq!(out, "nothing to substitute");
    }


    #[test]
    fn render_incremental_prompt_handles_only_one_placeholder() {
        let out = render_incremental_prompt("only {previous_summary}", "S", "T");
        assert_eq!(out, "only S");
        let out = render_incremental_prompt("only {new_transcript}", "S", "T");
        assert_eq!(out, "only T");
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


    // ── Custom prompts ARE used in the rendered LLM prompt (gap 3bb02236012c) ─
    // refresh_summary resolves the admin-overridden full/incremental templates
    // and renders them into the prompt sent to the LLM. These pin that a CUSTOM
    // template's literal instructions survive into the rendered prompt (the
    // override isn't silently dropped in favor of the default).

    #[test]
    fn render_full_prompt_uses_custom_template_and_substitutes_transcript() {
        let custom = "CUSTOM-FULL-INSTRUCTION: condense ->\n{transcript}\n<- end";
        let out = render_full_prompt(custom, "alice: hi\nbob: hey\n");
        assert!(out.starts_with("CUSTOM-FULL-INSTRUCTION:"), "custom template kept: {out}");
        assert!(out.contains("alice: hi"), "transcript substituted: {out}");
        assert!(!out.contains("{transcript}"), "placeholder consumed: {out}");
    }


    #[test]
    fn render_full_prompt_without_placeholder_keeps_custom_text() {
        // A custom template that omits {transcript} is still used verbatim
        // (the LLM gets the admin's instruction, not the default).
        let out = render_full_prompt("JUST SUMMARIZE TERSELY", "ignored transcript");
        assert_eq!(out, "JUST SUMMARIZE TERSELY");
    }


    #[test]
    fn default_full_prompt_differs_from_a_custom_one() {
        // Guards against a regression where the override is ignored: the custom
        // render must NOT equal the default-template render for the same input.
        let transcript = "u: q\na: r\n";
        let custom = render_full_prompt("MY OWN PROMPT {transcript}", transcript);
        let default = render_full_prompt(DEFAULT_FULL_SUMMARY_PROMPT, transcript);
        assert_ne!(custom, default, "a custom prompt must change the rendered text");
        assert!(custom.contains("MY OWN PROMPT"));
    }

    // ── Non-text message handling (gap 9846f7fe8f6d) ──────────────────────
    // build_transcript + decide_summarize_action must treat messages whose
    // only content is non-text (tool calls, file attachments → text == "")
    // as contributing nothing to the LLM transcript.

    #[test]
    fn build_transcript_skips_non_text_messages() {
        let msgs = vec![
            msg(Uuid::new_v4(), "user", "hello"),
            // A tool-only / file-only message: message_to_summarizable yields "".
            msg(Uuid::new_v4(), "assistant", ""),
            msg(Uuid::new_v4(), "user", "world"),
        ];
        let t = build_transcript(&msgs);
        assert!(t.contains("user: hello"), "text messages kept: {t:?}");
        assert!(t.contains("user: world"), "text messages kept: {t:?}");
        assert!(
            !t.contains("assistant:"),
            "a non-text (empty) message must be omitted from the transcript: {t:?}"
        );
    }


    #[test]
    fn all_non_text_messages_never_summarize() {
        // A branch whose messages are all non-text contributes 0 transcript
        // tokens, so summarization is a Noop (summarizer.rs:194-199 intent).
        let msgs = vec![
            msg(Uuid::new_v4(), "user", ""),
            msg(Uuid::new_v4(), "assistant", ""),
        ];
        assert_eq!(
            decide_summarize_action(&msgs, 10, 10, None),
            SummarizeAction::Noop,
            "all-non-text branch must not trigger summarization"
        );
    }


    // ---- message_to_summarizable (content-block text extraction) ----

    fn content_block(
        content: serde_json::Value,
    ) -> crate::modules::chat::core::models::content::MessageContent {
        let ty = content
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("text")
            .to_string();
        crate::modules::chat::core::models::content::MessageContent {
            id: Uuid::new_v4(),
            message_id: Uuid::new_v4(),
            content_type: ty,
            content,
            sequence_order: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }


    fn message_with(
        role: &str,
        blocks: Vec<serde_json::Value>,
    ) -> crate::modules::chat::core::types::MessageWithContent {
        crate::modules::chat::core::types::MessageWithContent {
            message: crate::modules::chat::core::models::message::Message {
                id: Uuid::new_v4(),
                role: role.to_string(),
                originated_from_id: Uuid::new_v4(),
                edit_count: 0,
                model_id: None,
                created_at: chrono::Utc::now(),
            },
            contents: blocks.into_iter().map(content_block).collect(),
        }
    }


    #[test]
    fn message_to_summarizable_extracts_only_text_blocks() {
        // A turn mixing a text block with a NON-text (thinking) block — only the
        // text contributes to the summarizable text.
        let m = message_with(
            "assistant",
            vec![
                serde_json::json!({ "type": "text", "text": "the answer is 42" }),
                serde_json::json!({ "type": "thinking", "thinking": "internal reasoning" }),
            ],
        );
        let s = message_to_summarizable(&m);
        assert_eq!(s.role, "assistant");
        assert_eq!(s.text, "the answer is 42", "only the text block contributes");
    }


    #[test]
    fn message_to_summarizable_non_text_only_turn_yields_empty_text() {
        // A turn with NO text block (thinking-only) → empty text, so the
        // downstream transcript/decide logic correctly skips it (no crash).
        let m = message_with(
            "user",
            vec![serde_json::json!({ "type": "thinking", "thinking": "just reasoning" })],
        );
        let s = message_to_summarizable(&m);
        assert_eq!(s.text, "", "a non-text-only turn must produce empty text");
    }


    /// Concurrent-refresh race (summarizer.rs:27-32 comment) on the per-branch
    /// `ON CONFLICT (branch_id) DO UPDATE` upsert (summarizer.rs:645-). Two
    /// background refreshes for the SAME branch can run at once; the upsert must
    /// converge them to exactly ONE row (no duplicate, no error), the surviving
    /// row carrying one racer's coherent (text, count) pair — never a torn mix.
    ///
    /// DB-gated soft-skip (mirrors `memory::reaper`'s in-source DB test): no
    /// `DATABASE_URL` / unreachable DB → green; runs for real against a migrated
    /// DB. Uses runtime sqlx (no compile-time DB needed).
    #[tokio::test]
    async fn concurrent_upsert_summary_for_same_branch_converges_to_one_row() {
        use sqlx::postgres::PgPoolOptions;

        let url = match std::env::var("DATABASE_URL") {
            Ok(u) => u,
            Err(_) => {
                eprintln!("skip: DATABASE_URL unset — no DB to exercise the upsert race");
                return;
            }
        };
        let pool = match PgPoolOptions::new().max_connections(4).connect(&url).await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("skip: DB unreachable ({e})");
                return;
            }
        };

        // Seed the FK chain: users -> conversations -> branches (the upsert's
        // branch_id is a PK + FK to branches ON DELETE CASCADE).
        let tag = Uuid::new_v4();
        let user_id: Uuid =
            sqlx::query_scalar("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
                .bind(format!("sumrace_{tag}"))
                .bind(format!("sumrace_{tag}@example.com"))
                .fetch_one(&pool)
                .await
                .expect("seed user");
        let conversation_id: Uuid =
            sqlx::query_scalar("INSERT INTO conversations (user_id) VALUES ($1) RETURNING id")
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .expect("seed conversation");
        let branch_id: Uuid =
            sqlx::query_scalar("INSERT INTO branches (conversation_id) VALUES ($1) RETURNING id")
                .bind(conversation_id)
                .fetch_one(&pool)
                .await
                .expect("seed branch");

        // Two concurrent refreshes for the SAME branch with DISTINCT payloads —
        // genuinely racing at the partial-PK ON CONFLICT (each its own pool conn).
        let (p1, p2) = (pool.clone(), pool.clone());
        let (r1, r2) = tokio::join!(
            upsert_summary(&p1, branch_id, "SUMMARY_ALPHA", None, 5, "model-a"),
            upsert_summary(&p2, branch_id, "SUMMARY_BETA", None, 7, "model-b"),
        );
        r1.expect("first concurrent upsert must succeed (ON CONFLICT resolves the race, not error)");
        r2.expect("second concurrent upsert must succeed (ON CONFLICT resolves the race, not error)");

        // Exactly ONE row for the branch (PK + ON CONFLICT collapse the race).
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM conversation_summaries WHERE branch_id = $1")
                .bind(branch_id)
                .fetch_one(&pool)
                .await
                .expect("count summary rows");
        assert_eq!(
            count, 1,
            "concurrent refresh must converge to exactly one summary row, never a duplicate"
        );

        // The surviving row is one racer's COHERENT (text, count) pair — the
        // upsert sets all columns from EXCLUDED, so no torn mix of the two.
        let (text, mc): (String, i32) = sqlx::query_as(
            "SELECT summary_text, message_count FROM conversation_summaries WHERE branch_id = $1",
        )
        .bind(branch_id)
        .fetch_one(&pool)
        .await
        .expect("fetch surviving summary row");
        assert!(
            (text == "SUMMARY_ALPHA" && mc == 5) || (text == "SUMMARY_BETA" && mc == 7),
            "surviving summary must be one racer's coherent (text,count) pair, got ({text}, {mc})"
        );

        // Cleanup (cascades to conversation/branch/summary).
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&pool)
            .await;
    }


    #[test]
    fn empty_or_whitespace_llm_summary_is_not_writable() {
        // The empty-LLM-response rejection: a blank/whitespace-only summary
        // from the model must be treated as non-writable (the refresh path
        // skips the upsert and returns early instead of persisting garbage).
        assert!(!summary_is_writable(""));
        assert!(!summary_is_writable("   "));
        assert!(!summary_is_writable("\n\t  \n"));
        // A real summary IS writable.
        assert!(summary_is_writable("The user prefers metric units."));
        assert!(summary_is_writable("  leading/trailing trimmed but non-empty  "));
    }


    #[test]
    fn window_override_none_is_identity() {
        // No model context window known → admin thresholds pass through.
        assert_eq!(apply_window_override(8000, 2000, None), (8000, 2000));
    }


    #[test]
    fn window_override_takes_the_smaller_trigger() {
        // override (0.75×window) below the admin cap → summarize earlier.
        assert_eq!(apply_window_override(8000, 2000, Some(3000)), (3000, 2000));
        // override ABOVE the admin cap → admin cap wins, keep_recent untouched.
        assert_eq!(apply_window_override(8000, 2000, Some(20000)), (8000, 2000));
    }


    #[test]
    fn window_override_reclamps_keep_recent_below_trigger() {
        // Small-context override pushing trigger below keep_recent must NOT
        // leave keep_recent >= trigger (which would silently disable
        // summarization). keep_recent is re-clamped to trigger-1.
        let (trigger, keep_recent) = apply_window_override(8000, 2000, Some(1500));
        assert_eq!(trigger, 1500);
        assert_eq!(keep_recent, 1499, "keep_recent re-clamped strictly below trigger");
        assert!(keep_recent < trigger, "summarization must still be able to fire");
    }


    #[test]
    fn window_override_handles_degenerate_tiny_trigger() {
        // Even a pathologically tiny override can't underflow keep_recent.
        let (trigger, keep_recent) = apply_window_override(8000, 2000, Some(1));
        assert_eq!(trigger, 1);
        assert_eq!(keep_recent, 0);
    }


    #[test]
    fn empty_or_whitespace_summary_is_not_writable() {
        // An empty or whitespace-only LLM response must be rejected so it can't
        // overwrite a good prior summary with a blank one.
        assert!(!summary_is_writable(""));
        assert!(!summary_is_writable("   "));
        assert!(!summary_is_writable("\n\t  \n"));
        // Any real content is writable.
        assert!(summary_is_writable("A concise summary."));
        assert!(summary_is_writable("  trimmed but non-empty  "));
    }
}
