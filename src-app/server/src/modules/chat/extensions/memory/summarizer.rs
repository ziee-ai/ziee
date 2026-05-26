//! Conversation summarizer — when a branch exceeds N messages the
//! summarizer condenses the oldest messages into a single text block
//! stored in `conversation_summaries`. The retriever then replaces
//! those summarized messages with the summary when assembling the
//! prompt, freeing context budget.
//!
//! Phase 6 scaffold. The trigger logic + DB shape are wired up here;
//! actual integration with `convert_history_to_messages_with_extensions`
//! is gated behind `apply_summary_to_history` which callers invoke
//! with the freshly-loaded conversation history. The full integration
//! sits in the chat streaming path; this module exposes the primitives.

use ai_providers::{ChatMessage, ChatRequest, ContentBlock, Provider, Role};
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;

/// Trigger threshold — branches with more than this many user/assistant
/// messages get summarized. Set conservatively; raising costs nothing
/// but lowering risks losing too much context.
pub const SUMMARIZE_AFTER_N_MESSAGES: usize = 50;
/// How many recent messages we KEEP verbatim when summarizing.
pub const KEEP_RECENT: usize = 10;

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
    let row = sqlx::query_as::<_, ConversationSummary>(
        r#"
        SELECT branch_id, summary_text, summarized_up_to_id, message_count,
               model_used, created_at, updated_at
        FROM conversation_summaries
        WHERE branch_id = $1
        "#,
    )
    .bind(branch_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

/// Replace summarized messages in `chat_request` with the persisted
/// summary block. Idempotent — if no summary exists for this branch,
/// the request is left untouched.
pub async fn apply_summary_to_history(
    branch_id: Uuid,
    chat_request: &mut ChatRequest,
) -> Result<(), AppError> {
    let pool = Repos.memory.pool_clone();
    let summary = match fetch_summary(&pool, branch_id).await? {
        Some(s) => s,
        None => return Ok(()),
    };

    // Prepend the summary as a system message. Old messages are still
    // in the request — callers that want a hard truncation should
    // filter chat_request.messages by message id <= summarized_up_to_id
    // BEFORE calling this function. The summary supplements; it does
    // not replace by default.
    let block = format!(
        "## Earlier conversation summary ({} messages condensed):\n\n{}",
        summary.message_count, summary.summary_text
    );
    chat_request.messages.insert(
        0,
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

    if conv_msgs.len() <= SUMMARIZE_AFTER_N_MESSAGES {
        return Ok(()); // Nothing to summarize.
    }

    let cutoff = conv_msgs.len().saturating_sub(KEEP_RECENT);
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
    sqlx::query(
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
    )
    .bind(branch_id)
    .bind(&summary_text)
    .bind(summarized_up_to_id)
    .bind(to_summarize.len() as i32)
    .bind(&model.name)
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
