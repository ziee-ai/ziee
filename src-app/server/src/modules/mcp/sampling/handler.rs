// MCP Sampling Handler
// Provides the platform-side implementation of MCP sampling protocol.
// When an MCP server sends sampling/createMessage, this handler is called
// to perform an LLM completion and return the result.

use async_trait::async_trait;
use futures_util::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use ai_providers::{ChatMessage, ChatRequest, ContentBlockDelta, Role};

use crate::common::AppError;
use crate::modules::chat::core::ai_provider::create_provider_from_model_id;

use super::models::{SamplingContent, SamplingCreateMessageRequest, SamplingCreateMessageResult};

/// Trait for handling MCP sampling requests
#[async_trait]
pub trait SamplingHandler: Send + Sync {
    /// Handle a sampling/createMessage request from an MCP server.
    /// The handler calls the LLM and returns the result.
    async fn create_message(
        &self,
        request: SamplingCreateMessageRequest,
    ) -> Result<SamplingCreateMessageResult, AppError>;
}

/// Sampling handler that uses the current conversation's model.
///
/// The provider is initialized once at construction time (2 DB queries),
/// then reused for all subsequent sampling calls within the same tool execution.
/// This eliminates per-call DB overhead and enables HTTP connection pool reuse
/// across sequential sampling requests from the MCP server.
pub struct ChatSamplingHandler {
    provider: Arc<ai_providers::Provider>,
    model_name: String,
}

impl ChatSamplingHandler {
    /// Initialize once: look up the model + provider from the DB, build the HTTP client.
    /// All subsequent `create_message()` calls reuse this cached state.
    pub async fn new(model_id: Uuid, user_id: Uuid) -> Result<Self, AppError> {
        // user_id is needed by `create_provider_from_model_id` to resolve
        // the API key (falls back to user's personal key when the system
        // key is missing — a behavior added on main after this branch
        // forked, where the call was `(pool, model_id)`).
        let (provider, model_name, _, _, _, _) =
            create_provider_from_model_id(model_id, user_id).await?;
        Ok(Self { provider, model_name })
    }
}

#[async_trait]
impl SamplingHandler for ChatSamplingHandler {
    async fn create_message(
        &self,
        request: SamplingCreateMessageRequest,
    ) -> Result<SamplingCreateMessageResult, AppError> {
        let mut messages: Vec<ChatMessage> = Vec::new();

        // Inject system prompt if provided
        if let Some(ref sys) = request.system_prompt {
            messages.push(ChatMessage {
                role: Role::System,
                content: vec![ai_providers::ContentBlock::Text { text: sys.clone() }],
            });
        }

        // Convert sampling messages, handling all content types
        for msg in &request.messages {
            let role = match msg.role.as_str() {
                "assistant" => Role::Assistant,
                _ => Role::User,
            };

            let content_blocks: Vec<ai_providers::ContentBlock> = match &msg.content {
                SamplingContent::Text { text } => {
                    vec![ai_providers::ContentBlock::Text { text: text.clone() }]
                }
                SamplingContent::Image { data, mime_type } => {
                    vec![ai_providers::ContentBlock::Image {
                        source: ai_providers::ImageSource::Base64 {
                            media_type: mime_type.clone(),
                            data: data.clone(),
                        },
                    }]
                }
                SamplingContent::Unknown => {
                    tracing::warn!("[sampling] Unknown content type in sampling message — skipping");
                    vec![]
                }
            };

            if !content_blocks.is_empty() {
                messages.push(ChatMessage { role, content: content_blocks });
            }
        }

        let chat_request = ChatRequest {
            model: self.model_name.clone(),
            messages,
            temperature: request.temperature.map(|t| t as f32),
            max_tokens: Some(request.max_tokens),
            ..Default::default()
        };

        tracing::info!(
            "[sampling] Starting LLM call (streaming): model={}, max_tokens={}",
            self.model_name, request.max_tokens
        );

        // Use streaming so we get the first tokens in seconds, not minutes.
        // Non-streaming (complete()) waits for ALL tokens before returning (~180s for long responses).
        // Streaming matches exactly how normal chat works — fast TTFT ~2-5s.
        let mut stream = self.provider.chat_stream(chat_request)
            .await
            .map_err(|e| AppError::internal_error(format!("Sampling LLM stream failed: {}", e)))?;

        tracing::info!("[sampling] Stream started successfully");

        let collect_future = async {
            let mut collected = String::new();
            let mut finish_reason: Option<String> = None;
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if chunk.finish_reason.is_some() {
                            finish_reason = chunk.finish_reason.clone();
                        }
                        for delta in chunk.content {
                            if let ContentBlockDelta::TextDelta { delta: text_delta, .. } = delta {
                                collected.push_str(&text_delta);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("[sampling] Stream chunk error: {}", e);
                        break;
                    }
                }
            }
            (collected, finish_reason)
        };

        let (text, finish_reason) = tokio::time::timeout(Duration::from_secs(600), collect_future)
            .await
            .map_err(|_| {
                tracing::warn!("[sampling] Stream collection timed out after 600s");
                AppError::internal_error("Sampling LLM call timed out after 600s")
            })?;

        tracing::info!("[sampling] LLM call completed, got {} chars", text.len());

        // Map provider finish_reason → MCP stopReason (camelCase per MCP spec)
        let stop_reason = finish_reason.map(|r| match r.as_str() {
            // Anthropic: "end_turn", OpenAI: "stop", Gemini: "STOP"
            "end_turn" | "stop" | "STOP" | "eos_token" => "endTurn".to_string(),
            "max_tokens" | "length" | "MAX_TOKENS" => "maxTokens".to_string(),
            "stop_sequence" | "STOP_SEQUENCE" => "stopSequence".to_string(),
            other => other.to_string(),
        });

        Ok(SamplingCreateMessageResult {
            role: "assistant".to_string(),
            content: SamplingContent::Text { text },
            model: self.model_name.clone(),
            stop_reason,
        })
    }
}
