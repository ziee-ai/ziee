//! `MemoryExtension` — wires the retrieval (before_llm_call) and
//! extraction (after_llm_call) hooks into the chat stream.

use aide::axum::ApiRouter;
use async_trait::async_trait;
use axum::response::sse::Event;
use sqlx::PgPool;
use std::convert::Infallible;

use ai_providers::{ChatMessage, ChatRequest, ContentBlock, Role};

/// System-prompt nudge injected on tool-capable models so the assistant saves
/// durable facts itself (inline self-save) instead of relying on the silent
/// background extractor.
const MEMORY_SAVE_NUDGE: &str = "## Saving memories\n\
    When the user states a durable, non-obvious fact about themselves — \
    especially an explicit 'remember …' request — call the `remember` tool to \
    persist it. Save only durable facts, not ephemeral chatter. Choose the \
    narrowest scope that fits: `conversation` = only this thread; `project` = \
    this project's work; `user` = always true about the person.";

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::chat::core::extension::{
    BeforeLlmAction, ChatExtension, ExtensionAction, StreamContext,
};
use crate::modules::chat::core::extension::request::SendMessageRequest;
use crate::modules::chat::core::models::Message;
use crate::modules::chat::core::models::content::MessageContent;
use crate::modules::chat::core::types::MessageWithContent;

pub struct MemoryExtension {
    #[allow(dead_code)]
    pool: PgPool,
}

impl MemoryExtension {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ChatExtension for MemoryExtension {
    fn name(&self) -> &str {
        "memory"
    }

    async fn initialize(&self, _pool: &PgPool) -> Result<(), AppError> {
        tracing::info!("Memory extension initialized");
        Ok(())
    }

    /// Retrieve relevant memories and inject as a system block.
    /// Errors here are silently swallowed inside `retrieve_and_inject`
    /// — memory must never break chat. We always return Continue.
    async fn before_llm_call(
        &self,
        context: &mut StreamContext,
        request: &mut ChatRequest,
        _send_request: &SendMessageRequest,
        _tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<BeforeLlmAction, AppError> {
        let assistant_id = context
            .metadata
            .get("assistant_id")
            .and_then(|v| v.as_str())
            .and_then(|s| uuid::Uuid::parse_str(s).ok());

        // Summarizer: drop the summarized prefix of chat_request.messages
        // and replace it with the persisted summary block. Net effect on
        // the LLM call: [System*, SummaryBlock, RecentTurns] instead of
        // [System*, AllOldTurns, RecentTurns]. Real prompt-side budget
        // freed proportionally to summary.message_count. Plan §6 Phase 6.
        if let Err(e) =
            super::super::engine::summarizer::apply_summary_to_history(context.branch_id, request).await
        {
            tracing::warn!("memory.before_llm_call: summary apply failed: {e}");
        }

        if let Err(e) = super::retriever::retrieve_and_inject(
            context.user_id,
            Some(context.conversation_id),
            assistant_id,
            request,
        )
        .await
        {
            tracing::warn!("memory.before_llm_call: retrieve_and_inject error: {e}");
        }

        // Inline self-save (Track B): on tool-capable models, attach the memory
        // `remember` tool (via the MCP extension) and nudge the model to use it.
        // The background extractor is skipped for these models in after_llm_call,
        // so saving is transparent + scoped rather than a silent separate call.
        let tool_capable =
            crate::modules::file::available_files::model_supports_tools(&context.metadata).await;
        if tool_capable {
            if let Ok(admin) = Repos.memory.get_admin_settings().await {
                if admin.enabled {
                    // Honor the per-user extraction opt-out. Inline self-save
                    // REPLACES the background extractor on tool-capable models
                    // (after_llm_call skips the extractor when tool_capable), so
                    // it must obey the SAME `extraction_enabled` gate the
                    // extractor enforces (engine/extractor.rs) — the privacy-first
                    // default is OFF (migration 56). Without this, a user who
                    // turned memory capture off would still get the `remember`
                    // tool attached + the assistant nudged to persist facts.
                    let opted_in = Repos
                        .memory
                        .get_or_init_user_settings(context.user_id)
                        .await
                        .map(|s| s.extraction_enabled)
                        .unwrap_or(false);
                    if opted_in {
                        context
                            .metadata
                            .insert("attach_memory_mcp".to_string(), serde_json::json!("true"));
                        request.messages.insert(
                            0,
                            ChatMessage {
                                role: Role::System,
                                content: vec![ContentBlock::Text {
                                    text: MEMORY_SAVE_NUDGE.to_string(),
                                }],
                            },
                        );
                    }
                }
            }
        }

        Ok(BeforeLlmAction::Continue)
    }

    /// Kick off background extraction. Returns immediately so the
    /// chat stream can finish; the extraction LLM call happens in a
    /// detached tokio task.
    async fn after_llm_call(
        &self,
        context: &StreamContext,
        final_message: &Message,
        _tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<ExtensionAction, AppError> {
        // Collect the last user + assistant message text. If we can't
        // extract enough text from the recent history, skip.
        let history = Repos.chat.core.get_conversation_history(context.branch_id).await?;

        let assistant_text = extract_text_from_message(final_message.id, &history);
        let user_text = history
            .iter()
            .filter(|m| m.message.role == "user")
            .last()
            .map(|m| collect_text(&m.contents))
            .unwrap_or_default();

        if user_text.trim().is_empty() && assistant_text.trim().is_empty() {
            return Ok(ExtensionAction::Complete);
        }

        let user_id = context.user_id;
        let source_message_id = Some(final_message.id);

        // Tool-capable models do their own inline self-save via the `remember`
        // tool (see before_llm_call), so skip the silent background extractor for
        // them — avoids double-saving. Non-capable models keep the extractor.
        let tool_capable =
            crate::modules::file::available_files::model_supports_tools(&context.metadata).await;
        if !tool_capable {
            tokio::spawn(async move {
                super::super::engine::extractor::extract_and_persist(
                    user_id,
                    user_text,
                    assistant_text,
                    source_message_id,
                )
                .await;
            });
        }

        // Auto-refresh the summarizer when the branch crosses the
        // threshold. Fire-and-forget (separate spawn so it can run
        // concurrently with extraction). Plan §6 Phase 6.
        let branch_id = context.branch_id;
        let message_count = history.len();
        // Fraction-of-window: clamp the flat summary trigger by 0.75× the CHAT
        // model's context window (local: native context_length; cloud: registry)
        // so a small-context local model summarizes before it overflows. None
        // when the window is unknown → the flat admin threshold stands.
        let trigger_override: Option<usize> =
            crate::modules::file::available_files::model_context_window(&context.metadata)
                .await
                .map(|w| (w as f64 * 0.75) as usize);
        tokio::spawn(async move {
            // Load admin once — we need both the trigger threshold and
            // the model id from the same row.
            let admin = match crate::core::Repos.memory.get_admin_settings().await {
                Ok(a) => a,
                Err(_) => return,
            };
            // Token-aware trigger: the authority is the (token-based)
            // `decide_summarize_action` inside `refresh_summary`. Keep only a
            // cheap guard that skips brand-new branches (avoids a needless
            // history reload before there's anything worth summarizing).
            if message_count < 4 {
                return;
            }
            if !admin.enabled {
                return;
            }
            // Use the admin's configured default_extraction_model_id as
            // the summarization model (separate column would be nicer
            // but the plan's schema only ships one).
            let Some(model_id) = admin.default_extraction_model_id else {
                return;
            };
            if let Err(e) = super::super::engine::summarizer::refresh_summary(
                branch_id,
                model_id,
                trigger_override,
            )
            .await
            {
                tracing::warn!("memory.summarizer: refresh failed for branch {branch_id}: {e}");
            }
        });

        Ok(ExtensionAction::Complete)
    }

    /// Register the memory bridge's HTTP routes. Owned by the bridge
    /// so chat doesn't have to know they exist. Exposes
    /// `GET`/`PUT /api/conversations/{id}/memory-mode` — replaces the
    /// `memory_mode` branch that used to live in chat's
    /// `PUT /api/conversations/{id}` handler (migration 76 dropped
    /// the column).
    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(super::memory_mode_routes::memory_mode_router())
    }
}

fn extract_text_from_message(
    target_id: uuid::Uuid,
    history: &[MessageWithContent],
) -> String {
    history
        .iter()
        .find(|m| m.message.id == target_id)
        .map(|m| collect_text(&m.contents))
        .unwrap_or_default()
}

/// Pull text out of message contents.
///
/// `MessageContentData` variants are composed by extensions at compile
/// time via the macro — we can't pattern-match `MessageContentData::Text(..)`
/// here without dragging the text extension as a hard dep. Mirror the
/// pattern used in `chat/extensions/title/title.rs`: serialize each
/// parsed content to a JSON value and read `type=="text"`.
fn collect_text(contents: &[MessageContent]) -> String {
    let mut buf = String::new();
    for c in contents {
        let Ok(data) = c.parse_content() else { continue };
        let Ok(value) = serde_json::to_value(&data) else { continue };
        if value.get("type").and_then(|t| t.as_str()) == Some("text") {
            if let Some(text) = value.get("text").and_then(|t| t.as_str()) {
                if !buf.is_empty() {
                    buf.push('\n');
                }
                buf.push_str(text);
            }
        }
    }
    buf
}
