//! `MemoryExtension` — wires the retrieval (before_llm_call) and
//! extraction (after_llm_call) hooks into the chat stream.

use async_trait::async_trait;
use axum::response::sse::Event;
use sqlx::PgPool;
use std::convert::Infallible;

use ai_providers::ChatRequest;

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
        if let Err(e) = super::retriever::retrieve_and_inject(context.user_id, request).await {
            tracing::warn!("memory.before_llm_call: retrieve_and_inject error: {e}");
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
        tokio::spawn(async move {
            super::extractor::extract_and_persist(
                user_id,
                user_text,
                assistant_text,
                source_message_id,
            )
            .await;
        });

        Ok(ExtensionAction::Complete)
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
