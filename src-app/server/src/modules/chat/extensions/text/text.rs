use async_trait::async_trait;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{
    common::AppError,
    modules::chat::{
        core::extension::{ChatExtension, SendMessageRequest, StreamContext},
        core::types::streaming::ContentBlockDelta,
        extensions::text::types::TextContent,
    },
};
use ai_providers::ContentBlock;
use aide::axum::ApiRouter;

/// Accumulated content during streaming
#[derive(Debug, Clone)]
struct AccumulatedContent {
    index: usize,
    content: TextContent,
}

/// Text Extension
/// Handles plain text and thinking content for messages
pub struct TextExtension {
    // DB handle from the shared extension factory; plain-text/thinking streaming
    // needs no DB, so this handle is retained (for factory-shape parity) but unread.
    #[allow(dead_code)]
    pool: PgPool,
    /// Store accumulated deltas during streaming (conversation_id -> Vec<AccumulatedContent>)
    accumulated: Arc<Mutex<HashMap<String, Vec<AccumulatedContent>>>>,
}

impl TextExtension {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            accumulated: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl ChatExtension for TextExtension {
    fn name(&self) -> &str {
        "text"
    }

    async fn initialize(&self, _pool: &PgPool) -> Result<(), AppError> {
        tracing::info!("Text extension initialized");
        Ok(())
    }

    async fn provide_user_message_content(
        &self,
        _context: &StreamContext,
        send_request: &SendMessageRequest,
        text_content: &str,
    ) -> Result<Vec<crate::modules::chat::core::models::content::MessageContentData>, AppError> {
        // Create Text content from user's message
        if text_content.is_empty() {
            return Ok(Vec::new());
        }

        // Server-internal injectors (e.g. background push-to-resume) set
        // `content_as_observation` so the message renders as a distinct
        // observation card while still wire-mapping to plain user text. The flag is
        // `#[serde(skip)]` on the request, so a client cannot set it (no spoofing).
        let block = if send_request.content_as_observation {
            TextContent::Observation {
                text: text_content.to_string(),
            }
        } else {
            TextContent::Text {
                text: text_content.to_string(),
            }
        };

        Ok(vec![block.to_message_content()])
    }

    async fn process_delta(
        &self,
        delta: &ai_providers::ContentBlockDelta,
        _context: &StreamContext,
    ) -> Result<Option<ContentBlockDelta>, AppError> {
        // Convert ai-providers deltas to our ContentBlockDelta
        match delta {
            ai_providers::ContentBlockDelta::TextDelta { index, delta } => {
                Ok(Some(ContentBlockDelta::TextDelta {
                    index: *index,
                    delta: delta.clone(),
                }))
            }
            ai_providers::ContentBlockDelta::ThinkingDelta { index, delta } => {
                Ok(Some(ContentBlockDelta::ThinkingDelta {
                    index: *index,
                    delta: delta.clone(),
                }))
            }
            _ => Ok(None), // Not our delta
        }
    }

    async fn accumulate_delta(
        &self,
        delta: &ContentBlockDelta,
        context: &StreamContext,
    ) -> Result<(), AppError> {
        let conv_key = context.conversation_id.to_string();
        let mut accumulated = self.accumulated.lock().await;
        let acc_list = accumulated.entry(conv_key).or_insert_with(Vec::new);

        match delta {
            ContentBlockDelta::TextDelta { index, delta } => {
                // Find or create accumulator for this index
                let acc = acc_list
                    .iter_mut()
                    .find(|a| a.index == *index && matches!(a.content, TextContent::Text { .. }));

                if let Some(acc) = acc {
                    // Append to existing text
                    if let TextContent::Text { text } = &mut acc.content {
                        text.push_str(delta);
                    }
                } else {
                    // Create new accumulator
                    acc_list.push(AccumulatedContent {
                        index: *index,
                        content: TextContent::Text {
                            text: delta.clone(),
                        },
                    });
                }
            }
            ContentBlockDelta::ThinkingDelta { index, delta } => {
                // Find or create accumulator for this index
                let acc = acc_list.iter_mut().find(|a| {
                    a.index == *index && matches!(a.content, TextContent::Thinking { .. })
                });

                if let Some(acc) = acc {
                    // Append to existing thinking
                    if let TextContent::Thinking { thinking, .. } = &mut acc.content {
                        thinking.push_str(delta);
                    }
                } else {
                    // Create new accumulator
                    acc_list.push(AccumulatedContent {
                        index: *index,
                        content: TextContent::Thinking {
                            thinking: delta.clone(),
                            metadata: None,
                        },
                    });
                }
            }
            _ => {} // Not our delta
        }

        Ok(())
    }

    async fn get_accumulated_content(
        &self,
        context: &StreamContext,
    ) -> Result<Vec<(usize, crate::modules::chat::core::models::content::MessageContentData)>, AppError>
    {
        let conv_key = context.conversation_id.to_string();
        let mut accumulated = self.accumulated.lock().await;

        if let Some(acc_list) = accumulated.remove(&conv_key) {
            Ok(acc_list
                .into_iter()
                .map(|acc| (acc.index, acc.content.to_message_content()))
                .collect())
        } else {
            Ok(Vec::new())
        }
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router
    }

    fn handled_content_types(&self) -> Vec<&'static str> {
        vec!["text", "thinking", "observation"]
    }

    async fn process_content_for_llm(
        &self,
        content: &crate::modules::chat::core::models::content::MessageContentData,
        _context: &StreamContext,
    ) -> Result<Option<ContentBlock>, AppError> {
        // Try to extract TextContent from MessageContentData::Extension
        let text_content = match TextContent::from_message_content(content) {
            Some(tc) => tc,
            None => return Ok(None), // Not a text extension content
        };

        // Convert to ContentBlock
        match text_content {
            TextContent::Text { text } => Ok(Some(ContentBlock::Text { text })),
            // A system/observation block is wire-visible as PLAIN USER TEXT (it
            // rides a user-role message, so it becomes Role::User text) — never
            // System (which is dropped from context). This is what lets the resumed
            // model actually SEE the background result.
            TextContent::Observation { text } => Ok(Some(ContentBlock::Text { text })),
            TextContent::Thinking { thinking, metadata } => {
                let signature = metadata.as_ref().and_then(|m| m.signature.clone());
                let redacted = metadata.as_ref().and_then(|m| m.redacted_data.clone());
                if let Some(data) = redacted {
                    // Redacted thinking replays as its own block.
                    Ok(Some(ContentBlock::RedactedThinking { data }))
                } else if signature.is_some() {
                    // Real thinking block with signature — the provider forwards it.
                    Ok(Some(ContentBlock::Thinking { thinking, signature }))
                } else {
                    // No signature → omit (the crate would otherwise drop it, and a
                    // signature-less thinking block is rejected by Anthropic).
                    Ok(None)
                }
            }
        }
    }

    async fn process_content_from_db(
        &self,
        _content: &mut crate::modules::chat::core::models::content::MessageContentData,
        _context: &StreamContext,
    ) -> Result<(), AppError> {
        Ok(())
    }

    fn convert_from_content_block(&self, block: &ContentBlock) -> Option<crate::modules::chat::core::models::content::MessageContentData> {
        match block {
            ContentBlock::Text { text } => {
                let content = TextContent::Text { text: text.clone() };
                Some(content.to_message_content())
            }
            ContentBlock::Thinking { thinking, signature } => {
                let content = TextContent::Thinking {
                    thinking: thinking.clone(),
                    metadata: signature.as_ref().map(|sig| super::types::ThinkingMetadata {
                        token_count: None,
                        signature: Some(sig.clone()),
                        redacted_data: None,
                    }),
                };
                Some(content.to_message_content())
            }
            ContentBlock::RedactedThinking { data } => {
                let content = TextContent::Thinking {
                    thinking: String::new(),
                    metadata: Some(super::types::ThinkingMetadata {
                        token_count: None,
                        signature: None,
                        redacted_data: Some(data.clone()),
                    }),
                };
                Some(content.to_message_content())
            }
            _ => None, // Not our content type
        }
    }
}

#[cfg(test)]
mod observation_tests {
    //! TEST-9 (bg-push-resume, DEC-1 observation content type). The `observation`
    //! block is a ziee-internal SYSTEM/observation type: it must WIRE-map to a
    //! plain-text provider block (user-visible context, never skipped), and the
    //! server-internal `content_as_observation` request flag must make the text
    //! extension emit an `observation`-typed block instead of `text`.
    use super::*;
    use crate::modules::chat::core::models::content::MessageContentData;

    /// A pool that never connects (the tested methods don't touch the DB).
    fn lazy_pool() -> PgPool {
        sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgresql://localhost/unused")
            .expect("lazy pool")
    }

    fn ctx(pool: PgPool) -> StreamContext {
        StreamContext {
            conversation_id: uuid::Uuid::new_v4(),
            branch_id: uuid::Uuid::new_v4(),
            message_id: None,
            user_id: uuid::Uuid::new_v4(),
            pool,
            metadata: HashMap::new(),
            iteration: 1,
        }
    }

    fn request(as_observation: bool) -> SendMessageRequest {
        let mut r: SendMessageRequest = serde_json::from_value(serde_json::json!({
            "content": "ignored",
            "model_id": uuid::Uuid::new_v4().to_string(),
            "branch_id": uuid::Uuid::new_v4().to_string(),
        }))
        .expect("build SendMessageRequest");
        r.content_as_observation = as_observation;
        r
    }

    // An `observation` content block converts to a plain USER-visible text block on
    // the wire (NOT skipped like a signature-less thinking block) — this is what
    // lets the resumed model actually see the background result.
    #[tokio::test]
    async fn observation_wire_maps_to_text_block() {
        let ext = TextExtension::new(lazy_pool());
        let block: MessageContentData = TextContent::Observation {
            text: "[Background task complete] Result: hello".into(),
        }
        .to_message_content();
        assert_eq!(block.content_type(), "observation", "stored as the observation type");

        let out = ext
            .process_content_for_llm(&block, &ctx(lazy_pool()))
            .await
            .expect("convert ok");
        match out {
            Some(ContentBlock::Text { text }) => {
                assert!(text.contains("Background task complete"), "carries the text: {text}");
            }
            other => panic!("observation must wire-map to a user-visible Text block, got {other:?}"),
        }
    }

    // With `content_as_observation` set, the text extension emits an
    // `observation`-typed block; without it, a plain `text` block.
    #[tokio::test]
    async fn flag_selects_observation_vs_text_block() {
        let ext = TextExtension::new(lazy_pool());

        let obs = ext
            .provide_user_message_content(&ctx(lazy_pool()), &request(true), "the result")
            .await
            .expect("provide ok");
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].content_type(), "observation", "flag set → observation block");

        let plain = ext
            .provide_user_message_content(&ctx(lazy_pool()), &request(false), "a normal message")
            .await
            .expect("provide ok");
        assert_eq!(plain.len(), 1);
        assert_eq!(plain[0].content_type(), "text", "flag unset → plain text block");
    }
}
