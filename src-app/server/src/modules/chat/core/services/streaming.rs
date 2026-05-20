// Streaming service infrastructure
#![allow(dead_code)]

// Streaming service - Core streaming logic with delta accumulation

use futures_util::{Stream, StreamExt};
use sqlx::PgPool;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;

use ai_providers::{ChatMessage, ChatRequest, StreamChatChunk as AiStreamChunk};

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::chat::core::{
    extension::{BeforeLlmAction, ExtensionRegistry, SendMessageRequest, StreamContext},
    models::{MessageContentData, MessageRole},
    types::{ChatStreamChunk, ContentBlockDelta},
};

/// Streaming service for chat messages
pub struct StreamingService {
    pool: PgPool,
    extension_registry: Option<Arc<ExtensionRegistry>>,
}

impl StreamingService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            extension_registry: None,
        }
    }

    pub fn with_extensions(mut self, registry: Arc<ExtensionRegistry>) -> Self {
        self.extension_registry = Some(registry);
        self
    }

    /// Send a message and stream the AI response
    /// This creates both the user message and assistant message, then streams content
    /// Supports extension-driven loop continuation for tool calling
    ///
    /// Returns a tuple of (content_stream, extension_event_receiver)
    /// The extension_event_receiver can be used to receive SSE events from extensions
    pub async fn send_message(
        &self,
        branch_id: Uuid,
        conversation_id: Uuid,
        user_id: Uuid,
        request: SendMessageRequest,
    ) -> Result<(
        Pin<Box<dyn Stream<Item = Result<ChatStreamChunk, AppError>> + Send>>,
        tokio::sync::mpsc::UnboundedReceiver<Result<axum::response::sse::Event, std::convert::Infallible>>,
    ), AppError>
    {
        // Create provider from model_id
        use crate::modules::chat::core::ai_provider::create_provider_from_model_id;

        let (provider, model_name, model_id, provider_id) =
            create_provider_from_model_id(request.model_id, user_id).await?;

        // Conditionally create user message (check extensions)
        // Extensions can prevent user message creation (e.g., MCP tool approval resumption)
        let user_message_id = if self.extension_registry
            .as_ref()
            .map(|reg| reg.should_create_user_message(&request))
            .unwrap_or(true)  // Default to true if no registry
        {
            // Create preliminary StreamContext for extensions to use
            // (provider metadata will be populated later in the loop)
            let preliminary_context = StreamContext {
                conversation_id,
                branch_id,
                message_id: None, // Assistant message not created yet
                user_id,
                pool: self.pool.clone(),
                metadata: std::collections::HashMap::new(),
                iteration: 0,
            };

            // Ask extensions for additional content blocks
            let extension_content = if let Some(registry) = &self.extension_registry {
                registry
                    .collect_user_message_content(&preliminary_context, &request, &request.content)
                    .await?
            } else {
                Vec::new()
            };

            // Extract context values to persist on the user message
            let msg_assistant_id = request.assistant_id;
            let msg_mcp_server_ids: Option<Vec<uuid::Uuid>> = request.mcp_config.as_ref().map(|c| {
                c.mcp_servers.iter().map(|s| s.server_id).collect()
            });

            // Create user message with context (model, assistant, mcp servers used)
            let user_message = Repos.chat.core.create_message(
                branch_id,
                MessageRole::User.as_str(),
                Some(request.model_id),
                msg_assistant_id,
                msg_mcp_server_ids,
            ).await?;

            // Create content blocks from extensions (text, files, etc.)
            // Extensions are called in priority order (text extension runs first at order 5)
            for (index, content_data) in extension_content.into_iter().enumerate() {
                Repos.chat.core.create_content(
                    user_message.id,
                    &content_data.content_type(),
                    content_data,
                    index as i32,
                )
                .await?;
            }

            Some(user_message.id)
        } else {
            None  // Extension prevented user message creation
        };

        // Get or create assistant message (BEFORE loop)
        // Extensions can provide existing message for continuation (e.g., MCP tool approval)
        let assistant_message_id = if let Some(reg) = &self.extension_registry {
            if let Some(msg_id) = reg.provide_assistant_message(&request, branch_id).await? {
                msg_id  // Existing message (resuming)
            } else {
                // No extension provided message, create new one
                let msg = Repos.chat.core.create_message(branch_id, MessageRole::Assistant.as_str(), None, None, None).await?;
                msg.id  // New message
            }
        } else {
            // No extension registry, create new message
            let msg = Repos.chat.core.create_message(branch_id, MessageRole::Assistant.as_str(), None, None, None).await?;
            msg.id  // New message
        };

        // Create channel for streaming output
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        // Create channel for extension events (SSE)
        let (ext_tx, ext_rx) = tokio::sync::mpsc::unbounded_channel();

        // Emit 'started' event BEFORE loop
        // This event communicates message IDs to client before content streaming begins
        {
            use crate::modules::chat::core::types::streaming::{SSEChatStreamEvent, SSEChatStreamStartedData};

            let started_event = SSEChatStreamEvent::Started(SSEChatStreamStartedData {
                user_message_id,
                conversation_id,
                branch_id,
            });

            if let Err(e) = ext_tx.send(Ok(started_event.into())) {
                return Err(AppError::internal_error(format!("Failed to send started event: {:?}", e)));
            }
        }

        // Clone data for spawned task
        let pool = self.pool.clone();
        let provider_for_task = provider.clone();
        let extension_registry = self.extension_registry.clone();

        // Move ext_tx into the spawned task to keep it alive
        // Spawn task to handle loop
        tokio::spawn(async move {
            // ext_tx is now owned by this task and will be kept alive
            // It's cloned for each accumulator iteration below
            // Safety limit to prevent runaway tool loops.
            // Extensions control actual loop behavior via ExtensionAction::Continue/Complete.
            // This is just a failsafe - MCP extension enforces user-configured max_iteration.
            const SAFETY_MAX_ITERATIONS: u32 = 1000;
            let mut iteration = 1u32;

            // OPTIMIZATION: Fetch history ONCE before loop, cache in memory
            // On Continue action, we append new content to cache instead of re-fetching
            let mut history = match Repos.chat.core.get_conversation_history(branch_id).await {
                Ok(h) => h,
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
            };

            // Filter out the assistant message ONLY if it's empty (no content yet)
            // On iteration 1: assistant message is empty → filter it out
            // On iteration 2+: assistant message has tool_use + tool_result → keep it
            // When resuming: message already has content → keep it
            history.retain(|msg_with_content| {
                if msg_with_content.message.id == assistant_message_id {
                    !msg_with_content.contents.is_empty()
                } else {
                    true
                }
            });

            loop {
                // Guard against infinite loops (safety limit, actual control via extensions)
                if iteration > SAFETY_MAX_ITERATIONS {
                    let error_chunk = ChatStreamChunk {
                        content: Vec::new(),
                        message_id: None,
                        conversation_id: Some(conversation_id),
                        branch_id: Some(branch_id),
                        finish_reason: Some("max_iterations".to_string()),
                        usage: None,
                        error: Some(crate::modules::chat::core::types::StreamError {
                            message: "Maximum tool calling iterations exceeded".to_string(),
                            code: Some("MAX_ITERATIONS_EXCEEDED".to_string()),
                        }),
                    };
                    let _ = tx.send(Ok(error_chunk));
                    break;
                }

                // Process content from database through extensions (enrichment phase)
                // This must happen before creating StreamContext to avoid borrowing issues
                if let Some(registry) = &extension_registry {
                    // Create temporary context for content processing
                    let temp_context = StreamContext {
                        conversation_id,
                        branch_id,
                        message_id: None,
                        user_id,
                        pool: pool.clone(),
                        metadata: std::collections::HashMap::new(),
                        iteration,
                    };

                    for msg_with_content in &mut history {
                        for content in &mut msg_with_content.contents {
                            let mut content_data = match content.parse_content() {
                                Ok(d) => d,
                                Err(e) => {
                                    let _ = tx.send(Err(e));
                                    break;
                                }
                            };

                            // Allow extension to enrich content (e.g., add download URLs)
                            if let Err(e) = registry
                                .process_content_from_db(&mut content_data, &temp_context)
                                .await
                            {
                                let _ = tx.send(Err(e));
                                break;
                            }

                            // Update content in history
                            // Note: We're working with a temporary structure here, so we don't persist changes
                            // Extensions should use metadata or cache if they need to persist enriched data
                        }
                    }
                }

                // Create context for content transformation
                let transform_context = StreamContext {
                    conversation_id,
                    branch_id,
                    message_id: None,
                    user_id,
                    pool: pool.clone(),
                    metadata: std::collections::HashMap::new(),
                    iteration,
                };

                // Convert to AI provider format
                let messages = match Self::convert_history_to_messages_with_extensions(
                    &history,
                    extension_registry.as_ref(),
                    &transform_context,
                )
                .await
                {
                    Ok(m) => m,
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        break;
                    }
                };

                // Create stream context
                let mut context_metadata = std::collections::HashMap::new();
                context_metadata.insert(
                    "provider_type".to_string(),
                    serde_json::json!(provider_for_task.provider_type()),
                );
                context_metadata.insert("model_name".to_string(), serde_json::json!(model_name));
                context_metadata.insert(
                    "model_id".to_string(),
                    serde_json::json!(model_id.to_string()),
                );
                context_metadata.insert(
                    "provider_id".to_string(),
                    serde_json::json!(provider_id.to_string()),
                );

                let mut stream_context = StreamContext {
                    conversation_id,
                    branch_id,
                    message_id: Some(assistant_message_id),
                    user_id,
                    pool: pool.clone(),
                    metadata: context_metadata,
                    iteration,
                };

                // Create chat request
                let mut chat_request = ChatRequest {
                    model: model_name.clone(),
                    messages,
                    temperature: Some(0.7),
                    max_tokens: Some(8192),
                    ..Default::default()
                };

                // Call before_llm_call hooks
                if let Some(registry) = &extension_registry {
                    match registry
                        .call_before_llm_call(&mut stream_context, &mut chat_request, &request, Some(&ext_tx))
                        .await
                    {
                        Ok(BeforeLlmAction::Continue) => {
                            // Continue with LLM call as normal
                        }
                        Ok(BeforeLlmAction::Complete) => {
                            // Skip LLM call, complete gracefully
                            tracing::info!("Skipping LLM call - extension requested completion");

                            // Send complete event
                            let _ = tx.send(Ok(ChatStreamChunk {
                                content: Vec::new(),
                                message_id: None,
                                conversation_id: Some(conversation_id),
                                branch_id: Some(branch_id),
                                finish_reason: Some("extension_complete".to_string()),
                                usage: None,
                                error: None,
                            }));
                            break;
                        }
                        Ok(BeforeLlmAction::CompleteWithContent { text }) => {
                            // Approved sampling tool returned is_final_response: true before LLM call.
                            // Stream the final text directly and skip the LLM entirely.
                            tracing::info!("Skipping LLM call - extension provided final content");

                            let content_offset = match Repos.chat.core.get_message_with_content(assistant_message_id).await {
                                Ok(Some(msg)) => msg.contents.len() as i32,
                                _ => 0,
                            };

                            if let Err(e) = Repos.chat.core.create_content(
                                assistant_message_id,
                                "text",
                                MessageContentData::Text { text: text.clone() },
                                content_offset,
                            ).await {
                                let _ = tx.send(Err(e));
                                break;
                            }

                            let text_chunk = ChatStreamChunk {
                                content: vec![ContentBlockDelta::TextDelta { index: 0, delta: text }],
                                message_id: Some(assistant_message_id),
                                conversation_id: Some(conversation_id),
                                branch_id: Some(branch_id),
                                finish_reason: None,
                                usage: None,
                                error: None,
                            };
                            let _ = tx.send(Ok(text_chunk));

                            let complete_chunk = ChatStreamChunk {
                                content: Vec::new(),
                                message_id: None,
                                conversation_id: Some(conversation_id),
                                branch_id: Some(branch_id),
                                finish_reason: Some("stop".to_string()),
                                usage: None,
                                error: None,
                            };
                            let _ = tx.send(Ok(complete_chunk));
                            break;
                        }
                        Err(e) => {
                            let _ = tx.send(Err(e));
                            break;
                        }
                    }
                }

                // Call AI provider
                let mut ai_stream = match provider_for_task.chat_stream(chat_request).await {
                    Ok(stream) => {
                        stream
                    }
                    Err(e) => {
                        let _ = tx.send(Err(AppError::internal_error(format!(
                            "AI provider error: {}",
                            e
                        ))));
                        break;
                    }
                };

                // Calculate content_offset from how many blocks the assistant message already has.
                // Iteration 1 has offset=0; each subsequent iteration starts after the previous
                // iteration's text+tool_use blocks (tool_results are added by the Continue handler).
                let content_offset = history
                    .iter()
                    .find(|m| m.message.id == assistant_message_id)
                    .map(|m| m.contents.len())
                    .unwrap_or(0);

                // Create accumulator with extension event channel
                // Clone ext_tx for this iteration (allows multiple iterations with same channel)
                let accumulator = Arc::new(Mutex::new(DeltaAccumulator {
                    pool: pool.clone(),
                    assistant_message_id,
                    content_blocks: Vec::new(),
                    conversation_id,
                    branch_id,
                    extension_registry: extension_registry.clone(),
                    stream_context: stream_context.clone(),
                    extension_action: None,
                    finish_reason: None,
                    usage: None,
                    extension_tx: Some(ext_tx.clone()),
                    finalized: false,
                    content_offset,
                }));

                // Stream chunks through accumulator
                let mut chunk_count = 0;
                while let Some(chunk_result) = ai_stream.next().await {
                    match chunk_result {
                        Ok(ai_chunk) => {
                            chunk_count += 1;
                            tracing::info!(
                                "Chunk #{} with {} deltas, finish_reason={:?}",
                                chunk_count,
                                ai_chunk.content.len(),
                                ai_chunk.finish_reason
                            );

                            let mut acc = accumulator.lock().await;
                            match acc.process_chunk(ai_chunk).await {
                                Ok(output_chunk) => {
                                    if tx.send(Ok(output_chunk)).is_err() {
                                        // Channel closed, stop streaming
                                        return;
                                    }
                                }
                                Err(e) => {
                                    let _ = tx.send(Err(e));
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(Err(AppError::internal_error(format!(
                                "Stream error: {}",
                                e
                            ))));
                            return;
                        }
                    }
                }

                tracing::info!("Streaming completed, total {} chunks", chunk_count);

                // Finalize the accumulator (write to database, call extensions)
                // This must happen BEFORE sending the Complete event
                {
                    let mut acc = accumulator.lock().await;
                    if let Err(e) = acc.finalize().await {
                        let _ = tx.send(Err(e));
                        return;
                    }
                }

                // Check extension action FIRST to decide if we should continue or complete
                let action = {
                    let acc = accumulator.lock().await;
                    acc.extension_action.clone()
                };

                match action {
                    Some(crate::modules::chat::core::extension::ExtensionAction::CompleteWithContent { text }) => {
                        // Tool result is a final user-facing answer (is_final_response: true).
                        // Emit the text as a delta, save it to the DB, then complete.

                        // Determine sequence_order for the new text block
                        let content_offset = match Repos.chat.core.get_message_with_content(assistant_message_id).await {
                            Ok(Some(msg)) => msg.contents.len() as i32,
                            _ => 0,
                        };

                        if let Err(e) = Repos.chat.core.create_content(
                            assistant_message_id,
                            "text",
                            MessageContentData::Text { text: text.clone() },
                            content_offset,
                        ).await {
                            let _ = tx.send(Err(e));
                            break;
                        }

                        // Stream the text as a single delta to the client
                        let text_chunk = ChatStreamChunk {
                            content: vec![ContentBlockDelta::TextDelta { index: 0, delta: text }],
                            message_id: Some(assistant_message_id),
                            conversation_id: Some(conversation_id),
                            branch_id: Some(branch_id),
                            finish_reason: None,
                            usage: None,
                            error: None,
                        };
                        let _ = tx.send(Ok(text_chunk));

                        // Send Complete event
                        let complete_chunk = ChatStreamChunk {
                            content: Vec::new(),
                            message_id: None,
                            conversation_id: Some(conversation_id),
                            branch_id: Some(branch_id),
                            finish_reason: Some("stop".to_string()),
                            usage: None,
                            error: None,
                        };
                        let _ = tx.send(Ok(complete_chunk));
                        break;
                    }
                    Some(crate::modules::chat::core::extension::ExtensionAction::Continue {
                        assistant_message_content,
                    }) => {
                        // Loop will continue - DON'T send complete event yet
                        // Append tool results to existing assistant message

                        // IMPORTANT: finalize() just wrote text + tool_use content to DB
                        // We need to sync our cache with what was persisted before appending tool_result
                        // Fetch the assistant message with content from DB (includes finalized content)
                        let assistant_msg_with_content = match Repos.chat.core.get_message_with_content(assistant_message_id).await {
                            Ok(Some(msg)) => msg,
                            Ok(None) => {
                                let _ = tx.send(Err(AppError::not_found("Assistant message not found")));
                                break;
                            }
                            Err(e) => {
                                let _ = tx.send(Err(e));
                                break;
                            }
                        };

                        // Update or create the assistant message in cache with finalized content
                        if let Some(assistant_msg) = history.iter_mut().find(|m| m.message.id == assistant_message_id) {
                            // Replace cache with DB state (text + tool_use from finalize)
                            assistant_msg.contents = assistant_msg_with_content.contents;
                        } else {
                            // First iteration: assistant message not in cache yet, add it
                            history.push(assistant_msg_with_content);
                        }

                        // Now get content_offset from the updated cache
                        let content_offset = history
                            .iter()
                            .find(|m| m.message.id == assistant_message_id)
                            .map(|m| m.contents.len() as i32)
                            .unwrap_or(0);

                        // Tool results are added as content blocks to the same message
                        // Collect created contents to append to cache after DB writes
                        let mut created_contents = Vec::new();

                        for (offset_index, content) in assistant_message_content.iter().enumerate() {
                            let content_type = content.content_type();
                            let actual_index = content_offset + offset_index as i32;
                            match Repos.chat.core.create_content(
                                assistant_message_id,
                                &content_type,
                                content.clone(),
                                actual_index,
                            ).await {
                                Ok(created) => {
                                    tracing::info!("Appended content block {} to assistant message", actual_index);
                                    created_contents.push(created);
                                }
                                Err(e) => {
                                    let _ = tx.send(Err(e));
                                    break;
                                }
                            }
                        }

                        // Append the tool_result contents to the cached history
                        if let Some(assistant_msg) = history.iter_mut().find(|m| m.message.id == assistant_message_id) {
                            assistant_msg.contents.extend(created_contents);
                        }

                        iteration += 1;
                        // Continue loop with next LLM call
                    }
                    _ => {
                        // Complete or None - send complete event and stop looping
                        let (finish_reason, usage) = {
                            let acc = accumulator.lock().await;
                            (acc.finish_reason.clone(), acc.usage.clone())
                        };

                        // Use finish_reason from provider, or default to "stop" if not provided
                        let final_finish_reason = finish_reason.unwrap_or_else(|| "stop".to_string());

                        // Send Complete event now that we're actually done
                        let complete_chunk = ChatStreamChunk {
                            content: Vec::new(),
                            message_id: None,
                            conversation_id: Some(conversation_id),
                            branch_id: Some(branch_id),
                            finish_reason: Some(final_finish_reason),
                            usage,
                            error: None,
                        };

                        let _ = tx.send(Ok(complete_chunk));
                        break;
                    }
                }
            }
            // Note: The loop already handles all complete event cases:
            // - Guard at start: sends error chunk when iteration > MAX_ITERATIONS
            // - Extension BeforeLlmAction::Complete: sends complete with extension_complete
            // - Normal completion (action is None or Complete): sends complete with provider finish_reason
            // No need for post-loop handling - this prevents duplicate complete events
        });

        // Return channel receiver as stream and extension event receiver
        Ok((Box::pin(UnboundedReceiverStream::new(rx)), ext_rx))
    }

    /// Convert conversation history to AI provider message format
    fn convert_history_to_messages(
        &self,
        history: &[crate::modules::chat::core::types::MessageWithContent],
    ) -> Result<Vec<ChatMessage>, AppError> {
        let mut messages = Vec::new();

        for msg_with_content in history {
            let role = msg_with_content
                .message
                .role_enum()
                .map_err(|e| AppError::internal_error(format!("Invalid message role: {}", e)))?;

            // Convert role to AI provider role
            let ai_role = match role {
                MessageRole::User => ai_providers::Role::User,
                MessageRole::Assistant => ai_providers::Role::Assistant,
                MessageRole::System => continue, // There should be no system message in the database
            };

            // Convert content blocks (all content now handled by extensions)
            let content_blocks = Vec::new();
            for content in &msg_with_content.contents {
                let _content_data = content.parse_content()?;
                // All content types are now extension types - this method shouldn't be used
                // Use convert_history_to_messages_with_extensions instead
                tracing::warn!("Using deprecated convert_history_to_messages without extensions - content may not be converted properly");
            }

            messages.push(ChatMessage {
                role: ai_role,
                content: content_blocks,
            });
        }

        Ok(messages)
    }

    /// Convert history to messages with extension support for content transformation
    /// This version supports extensions transforming content before sending to LLM
    ///
    /// IMPORTANT: For assistant messages containing tool_use and tool_result blocks,
    /// this function splits them into separate messages to comply with AI provider APIs:
    /// - tool_use blocks → Assistant message
    /// - tool_result blocks → Tool message (Role::Tool - unified interface)
    /// - other content (text) → Assistant message
    ///
    /// Each provider handles Role::Tool correctly:
    /// - Anthropic: converts to "user" with tool_result content
    /// - OpenAI: converts to "tool" role
    async fn convert_history_to_messages_with_extensions(
        history: &[crate::modules::chat::core::types::MessageWithContent],
        extension_registry: Option<&Arc<ExtensionRegistry>>,
        context: &StreamContext,
    ) -> Result<Vec<ChatMessage>, AppError> {
        let mut messages = Vec::new();

        for msg_with_content in history {
            let role = msg_with_content
                .message
                .role_enum()
                .map_err(|e| AppError::internal_error(format!("Invalid message role: {}", e)))?;

            // Skip system messages
            if role == MessageRole::System {
                continue;
            }

            // For assistant messages: reconstruct proper per-iteration interleaving.
            // The assistant DB message contains content from all previous iterations merged
            // together. Walk blocks in sequence_order and flush one Assistant+Tool pair each
            // time a complete tool_use → tool_result round trip is detected.
            if role == MessageRole::Assistant {
                let mut current_text: Vec<ai_providers::ContentBlock> = Vec::new();
                let mut current_tool_uses: Vec<ai_providers::ContentBlock> = Vec::new();
                let mut pending_ids: std::collections::HashSet<String> = Default::default();
                let mut current_results: Vec<ai_providers::ContentBlock> = Vec::new();

                for content in &msg_with_content.contents {
                    let content_data = content.parse_content()?;

                    // Skip FileAttachment in assistant messages — MCP artifacts are stored for
                    // UI display only. The ToolResult content already tells the LLM about them.
                    // Including them as image blocks confuses the LLM into embedding them inline.
                    if matches!(content_data, MessageContentData::FileAttachment { .. }) {
                        continue;
                    }

                    let block = if let Some(registry) = extension_registry {
                        match registry.process_content_for_llm(&content_data, context).await? {
                            Some(transformed_block) => Some(transformed_block),
                            None => {
                                let ext_content = serde_json::to_value(&content_data)
                                    .map_err(|e| AppError::internal_error(format!("Failed to serialize content: {}", e)))?;
                                registry.convert_extension_to_content_block(&ext_content)
                            }
                        }
                    } else {
                        None
                    };

                    if let Some(b) = block {
                        match &b {
                            ai_providers::ContentBlock::ToolUse { id, .. } => {
                                pending_ids.insert(id.clone());
                                current_tool_uses.push(b);
                            }
                            ai_providers::ContentBlock::ToolResult { tool_use_id, .. } => {
                                pending_ids.remove(tool_use_id);
                                current_results.push(b);
                                // All tool_uses for this iteration have results — flush one pair.
                                // Each provider handles Role::Tool correctly:
                                // - Anthropic: converts to "user" with tool_result content
                                // - OpenAI: converts to "tool" role
                                if pending_ids.is_empty() && !current_tool_uses.is_empty() {
                                    let assistant_content: Vec<_> = current_text
                                        .drain(..)
                                        .chain(current_tool_uses.drain(..))
                                        .collect();
                                    messages.push(ChatMessage {
                                        role: ai_providers::Role::Assistant,
                                        content: assistant_content,
                                    });
                                    messages.push(ChatMessage {
                                        role: ai_providers::Role::Tool,
                                        content: current_results.drain(..).collect(),
                                    });
                                }
                            }
                            _ => current_text.push(b),
                        }
                    }
                }

                // Trailing content: in-progress iteration with no result yet, or a pure-text
                // final answer. Anthropic requires text and tool_use in the same message, and
                // the conversation must NOT end with an assistant message — both are satisfied
                // because in normal operation the last block in history is always a tool_result
                // (the Continue handler only advances the loop after writing the result to DB).
                // During the approval flow, unmatched tool_uses here are intentional: before_llm_call
                // will append the real tool_results as a following User message.
                let trailing: Vec<_> = current_text.into_iter().chain(current_tool_uses).collect();
                if !trailing.is_empty() {
                    messages.push(ChatMessage {
                        role: ai_providers::Role::Assistant,
                        content: trailing,
                    });
                }

                continue; // skip the non-assistant path below
            }

            // Non-assistant messages (user): convert all blocks normally
            let mut all_blocks = Vec::new();
            for content in &msg_with_content.contents {
                let content_data = content.parse_content()?;

                let block = if let Some(registry) = extension_registry {
                    match registry.process_content_for_llm(&content_data, context).await? {
                        Some(transformed_block) => Some(transformed_block),
                        None => {
                            let ext_content = serde_json::to_value(&content_data)
                                .map_err(|e| AppError::internal_error(format!("Failed to serialize content: {}", e)))?;
                            registry.convert_extension_to_content_block(&ext_content)
                        }
                    }
                } else {
                    None
                };

                if let Some(b) = block {
                    all_blocks.push(b);
                }
            }

            if !all_blocks.is_empty() {
                let ai_role = match role {
                    MessageRole::User => ai_providers::Role::User,
                    MessageRole::Assistant => ai_providers::Role::Assistant,
                    MessageRole::System => continue,
                };
                messages.push(ChatMessage {
                    role: ai_role,
                    content: all_blocks,
                });
            }
        }

        Ok(messages)
    }

    /// Static version of convert_history_to_messages for use when extensions not available
    /// Kept for backward compatibility
    fn convert_history_to_messages_static(
        history: &[crate::modules::chat::core::types::MessageWithContent],
    ) -> Result<Vec<ChatMessage>, AppError> {
        let mut messages = Vec::new();

        for msg_with_content in history {
            let role = msg_with_content
                .message
                .role_enum()
                .map_err(|e| AppError::internal_error(format!("Invalid message role: {}", e)))?;

            // Convert role to AI provider role
            let ai_role = match role {
                MessageRole::User => ai_providers::Role::User,
                MessageRole::Assistant => ai_providers::Role::Assistant,
                MessageRole::System => continue, // Skip system messages for now
            };

            // Convert content blocks (all content now handled by extensions)
            let content_blocks = Vec::new();
            for content in &msg_with_content.contents {
                let _content_data = content.parse_content()?;
                // All content types are now extension types - this static method shouldn't be used
                // Use convert_history_to_messages_with_extensions with a registry instead
                tracing::warn!("Using deprecated convert_history_to_messages_static without extensions - content may not be converted properly");
            }

            messages.push(ChatMessage {
                role: ai_role,
                content: content_blocks,
            });
        }

        Ok(messages)
    }

    /// Transform AI provider stream to our ChatStreamChunk format with accumulation
    fn transform_stream(
        &self,
        ai_stream: Pin<
            Box<dyn Stream<Item = Result<AiStreamChunk, ai_providers::ProviderError>> + Send>,
        >,
        accumulator: Arc<Mutex<DeltaAccumulator>>,
    ) -> Pin<Box<dyn Stream<Item = Result<ChatStreamChunk, AppError>> + Send>> {
        use futures_util::StreamExt;

        let stream = ai_stream.then(move |result| {
            let accumulator = Arc::clone(&accumulator);
            async move {
                match result {
                    Ok(ai_chunk) => {
                        // Process the chunk and accumulate deltas
                        let mut acc = accumulator.lock().await;
                        acc.process_chunk(ai_chunk).await
                    }
                    Err(e) => Err(AppError::internal_error(format!(
                        "AI provider stream error: {}",
                        e
                    ))),
                }
            }
        });

        Box::pin(stream)
    }
}

/// Accumulated content block in memory
#[derive(Debug, Clone)]
struct AccumulatedContent {
    content_type: String,
    accumulated_text: String,
    index: usize,
}

/// Delta accumulator - Manages delta accumulation in memory
/// Writes to database ONLY when streaming finishes (memory accumulation strategy)
struct DeltaAccumulator {
    pool: PgPool,
    assistant_message_id: Uuid,
    content_blocks: Vec<AccumulatedContent>,
    conversation_id: Uuid,
    branch_id: Uuid,
    extension_registry: Option<Arc<ExtensionRegistry>>,
    stream_context: StreamContext,
    /// Action returned by extensions after LLM call (set after finalize)
    extension_action: Option<crate::modules::chat::core::extension::ExtensionAction>,
    /// Finish reason from AI provider (stored when stream completes)
    finish_reason: Option<String>,
    /// Usage data from AI provider (stored when stream completes)
    usage: Option<crate::modules::chat::core::types::Usage>,
    /// Channel for extension events (SSE)
    extension_tx: Option<tokio::sync::mpsc::UnboundedSender<Result<axum::response::sse::Event, std::convert::Infallible>>>,
    /// Flag to track if finalize() has been called (prevents double-finalization)
    finalized: bool,
    /// Offset into the assistant message's global sequence_order for this iteration.
    /// Iteration 1 starts at 0, iteration 2 starts at 3 (after text+tool_use+result), etc.
    content_offset: usize,
}

impl DeltaAccumulator {
    /// Process an AI provider chunk and accumulate deltas in memory
    async fn process_chunk(
        &mut self,
        ai_chunk: AiStreamChunk,
    ) -> Result<ChatStreamChunk, AppError> {
        // Store finish_reason and usage when stream completes
        if let Some(finish_reason) = ai_chunk.finish_reason.clone() {
            self.finish_reason = Some(finish_reason);
        }
        if let Some(usage) = ai_chunk.usage.as_ref() {
            self.usage = Some(crate::modules::chat::core::types::Usage {
                input_tokens: Some(usage.prompt_tokens),
                output_tokens: Some(usage.completion_tokens),
            });
        }

        let mut output_chunk = ChatStreamChunk {
            content: Vec::new(),
            message_id: Some(self.assistant_message_id),
            conversation_id: Some(self.conversation_id),
            branch_id: Some(self.branch_id),
            finish_reason: None,  // Don't include finish_reason in content chunks
            usage: None,          // Don't include usage in content chunks
            error: None,
        };

        // Process each content delta - accumulate in memory
        for ai_delta in &ai_chunk.content {
            // Try core conversion first
            let delta = if let Some(core_delta) = ContentBlockDelta::from_ai_providers_delta(ai_delta) {
                Some(core_delta)
            } else if let Some(registry) = &self.extension_registry {
                // Let extensions handle unknown deltas
                registry.process_delta(ai_delta, &self.stream_context).await?
            } else {
                None
            };

            if let Some(delta) = delta {
                // Accumulate delta in memory (no DB write)
                self.accumulate_delta_in_memory(&delta).await;

                // Add to output chunk for streaming to client
                output_chunk.content.push(delta);
            }
        }

        // If streaming finished, write all accumulated content to database
        if ai_chunk.finish_reason.is_some() {
            self.finalize().await?;
        }

        Ok(output_chunk)
    }

    /// Accumulate a delta in memory (no database writes during streaming)
    async fn accumulate_delta_in_memory(&mut self, delta: &ContentBlockDelta) {
        match delta {
            ContentBlockDelta::TextDelta { index, delta } => {
                self.ensure_content_block_exists(*index, "text");
                if let Some(block) = self.content_blocks.get_mut(*index) {
                    block.accumulated_text.push_str(delta);
                }
            }
            ContentBlockDelta::ThinkingDelta { index, delta } => {
                self.ensure_content_block_exists(*index, "thinking");
                if let Some(block) = self.content_blocks.get_mut(*index) {
                    block.accumulated_text.push_str(delta);
                }
            }
            // Extension deltas - delegate to extensions
            _ => {
                if let Some(registry) = &self.extension_registry {
                    registry.accumulate_delta(delta, &self.stream_context).await.ok();
                }
            }
        }
    }

    /// Ensure content block exists in memory at specified index
    fn ensure_content_block_exists(&mut self, index: usize, content_type: &str) {
        // Resize vector if needed
        while self.content_blocks.len() <= index {
            self.content_blocks.push(AccumulatedContent {
                content_type: String::new(),
                accumulated_text: String::new(),
                index: self.content_blocks.len(),
            });
        }

        // Set content type if not already set
        if self.content_blocks[index].content_type.is_empty() {
            self.content_blocks[index].content_type = content_type.to_string();
        }
    }

    /// Finalize accumulation - write all accumulated content to database
    /// This is called ONCE when streaming completes (finish_reason is received)
    async fn finalize(&mut self) -> Result<(), AppError> {
        tracing::info!(
            "Finalize called for message_id={}",
            self.assistant_message_id
        );

        // Check if already finalized (prevents double-finalization)
        if self.finalized {
            tracing::info!("Already finalized, skipping");
            return Ok(());
        }

        // Write all accumulated content blocks to database in a single transaction
        let mut tx = self.pool.begin().await.map_err(AppError::database_error)?;

        for accumulated in &self.content_blocks {
            // Skip empty content blocks
            if accumulated.content_type.is_empty() {
                continue;
            }

            // Create MessageContentData from accumulated text
            let content_data = match accumulated.content_type.as_str() {
                "text" => MessageContentData::Text {
                    text: accumulated.accumulated_text.clone(),
                },
                "thinking" => MessageContentData::Thinking {
                    thinking: accumulated.accumulated_text.clone(),
                    metadata: None,
                },
                _ => continue, // Skip unknown types (extensions handle their own)
            };

            // Serialize to JSON (flattened for Extension variants)
            let content_json = content_data.to_api_content();

            // Insert content block
            sqlx::query!(
                r#"
                INSERT INTO message_contents (message_id, content_type, content, sequence_order)
                VALUES ($1, $2, $3, $4)
                "#,
                self.assistant_message_id,
                accumulated.content_type,
                content_json,
                (self.content_offset + accumulated.index) as i32
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
        }

        // Get accumulated content from extensions and persist to database
        if let Some(registry) = &self.extension_registry {
            let extension_content = registry
                .get_accumulated_content(&self.stream_context)
                .await?;

            tracing::info!(
                "Extension get_accumulated_content returned {} items for message {}",
                extension_content.len(),
                self.assistant_message_id
            );

            for (index, content_data) in extension_content {
                let content_type = content_data.content_type();
                // Use to_api_content() to flatten Extension variants
                let content_json = content_data.to_api_content();

                tracing::info!(
                    "Persisting extension content at index {}: type={}",
                    index,
                    content_type
                );

                sqlx::query!(
                    r#"
                    INSERT INTO message_contents (message_id, content_type, content, sequence_order)
                    VALUES ($1, $2, $3, $4)
                    "#,
                    self.assistant_message_id,
                    content_type,
                    content_json,
                    (self.content_offset + index) as i32
                )
                .execute(&mut *tx)
                .await
                .map_err(AppError::database_error)?;
            }
        }

        tx.commit().await.map_err(AppError::database_error)?;

        // Call extension hooks after database write completes
        if let Some(registry) = &self.extension_registry {
            // Fetch the complete message from database
            let final_message = Repos.chat.core.get_message( self.assistant_message_id)
                .await?
                .ok_or_else(|| AppError::internal_error("Message not found after finalize"))?;

            // Call after_llm_call hooks and store the result
            // Pass the SSE channel so extensions can send events
            match registry
                .call_after_llm_call(&self.stream_context, &final_message, self.extension_tx.as_ref())
                .await
            {
                Ok(action) => {
                    self.extension_action = Some(action);
                }
                Err(e) => {
                    // Log error but don't fail the stream
                    eprintln!("Extension error in after_llm_call: {}", e);
                    // Default to Complete on error
                    self.extension_action =
                        Some(crate::modules::chat::core::extension::ExtensionAction::Complete);
                }
            }
        }

        // Mark as finalized to prevent double-finalization
        self.finalized = true;

        Ok(())
    }
}

/// Group already-categorized content blocks into provider-ready ChatMessage(s).
///
/// For an Assistant turn with tool blocks the result is two messages:
///   `[Assistant { text + tool_use }, Tool { tool_result }]`
/// matching Anthropic's requirement that text and tool_use share a single
/// assistant message, and that the conversation does NOT end on an assistant
/// turn. The `Role::Tool` message is a unified abstraction — each provider
/// maps it correctly (Anthropic → "user" with tool_result content, OpenAI →
/// "tool" role).
///
/// For non-tool messages (or User messages) all blocks are combined into a
/// single message with the original role.
fn group_blocks_into_provider_messages(
    role: MessageRole,
    tool_use_blocks: Vec<ai_providers::ContentBlock>,
    tool_result_blocks: Vec<ai_providers::ContentBlock>,
    other_blocks: Vec<ai_providers::ContentBlock>,
) -> Vec<ChatMessage> {
    let mut messages = Vec::new();

    if role == MessageRole::Assistant
        && (!tool_use_blocks.is_empty() || !tool_result_blocks.is_empty())
    {
        // 1. Text + tool_use blocks → single Assistant message (text before tool_use).
        let assistant_content: Vec<_> = other_blocks.into_iter().chain(tool_use_blocks).collect();
        if !assistant_content.is_empty() {
            messages.push(ChatMessage {
                role: ai_providers::Role::Assistant,
                content: assistant_content,
            });
        }
        // 2. Tool result blocks → Tool message (unified).
        if !tool_result_blocks.is_empty() {
            messages.push(ChatMessage {
                role: ai_providers::Role::Tool,
                content: tool_result_blocks,
            });
        }
    } else {
        let all_blocks: Vec<_> = [tool_use_blocks, tool_result_blocks, other_blocks]
            .into_iter()
            .flatten()
            .collect();
        if !all_blocks.is_empty() {
            let ai_role = match role {
                MessageRole::User => ai_providers::Role::User,
                MessageRole::Assistant => ai_providers::Role::Assistant,
                MessageRole::System => return messages, // skipped at caller
            };
            messages.push(ChatMessage {
                role: ai_role,
                content: all_blocks,
            });
        }
    }

    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_providers::ContentBlock;
    use serde_json::json;

    fn text(s: &str) -> ContentBlock {
        ContentBlock::Text {
            text: s.to_string(),
        }
    }

    fn tool_use(id: &str, name: &str) -> ContentBlock {
        ContentBlock::ToolUse {
            id: id.to_string(),
            name: name.to_string(),
            input: json!({}),
        }
    }

    fn tool_result(id: &str, content: &str) -> ContentBlock {
        ContentBlock::ToolResult {
            tool_use_id: id.to_string(),
            name: None,
            content: vec![ContentBlock::Text {
                text: content.to_string(),
            }],
            is_error: None,
        }
    }

    /// The core fix: an Assistant turn with text + tool_use + tool_result must
    /// produce TWO messages — `[Assistant { text + tool_use }, Tool { tool_result }]`
    /// — not three with text as a trailing assistant message. Anthropic rejects
    /// the latter (text + tool_use must share a message; conversation must not
    /// end on an assistant turn).
    #[test]
    fn assistant_with_text_tool_use_and_tool_result_groups_to_two_messages() {
        let msgs = group_blocks_into_provider_messages(
            MessageRole::Assistant,
            vec![tool_use("call_1", "search")],
            vec![tool_result("call_1", "ok")],
            vec![text("Let me search for that.")],
        );

        assert_eq!(msgs.len(), 2, "must produce exactly two provider messages");

        // First: Assistant with text BEFORE tool_use (Anthropic requires this order)
        assert!(matches!(msgs[0].role, ai_providers::Role::Assistant));
        assert_eq!(msgs[0].content.len(), 2);
        assert!(matches!(msgs[0].content[0], ContentBlock::Text { .. }));
        assert!(matches!(msgs[0].content[1], ContentBlock::ToolUse { .. }));

        // Second: Tool with tool_result (provider maps Role::Tool appropriately)
        assert!(matches!(msgs[1].role, ai_providers::Role::Tool));
        assert_eq!(msgs[1].content.len(), 1);
        assert!(matches!(msgs[1].content[0], ContentBlock::ToolResult { .. }));
    }

    /// Even with NO text, tool_use + tool_result still produce two messages.
    #[test]
    fn assistant_with_tool_use_and_tool_result_only_groups_to_two_messages() {
        let msgs = group_blocks_into_provider_messages(
            MessageRole::Assistant,
            vec![tool_use("call_1", "search")],
            vec![tool_result("call_1", "ok")],
            vec![],
        );

        assert_eq!(msgs.len(), 2);
        assert!(matches!(msgs[0].role, ai_providers::Role::Assistant));
        assert_eq!(msgs[0].content.len(), 1);
        assert!(matches!(msgs[0].content[0], ContentBlock::ToolUse { .. }));
        assert!(matches!(msgs[1].role, ai_providers::Role::Tool));
    }

    /// tool_result without tool_use (e.g. resumed approval) still emits the
    /// Tool message but skips the empty Assistant message.
    #[test]
    fn assistant_with_only_tool_result_emits_single_tool_message() {
        let msgs = group_blocks_into_provider_messages(
            MessageRole::Assistant,
            vec![],
            vec![tool_result("call_1", "ok")],
            vec![],
        );

        assert_eq!(msgs.len(), 1);
        assert!(matches!(msgs[0].role, ai_providers::Role::Tool));
    }

    /// Plain assistant text (no tool blocks) stays as a single Assistant message
    /// — the categorization path must not fire.
    #[test]
    fn assistant_with_only_text_emits_single_assistant_message() {
        let msgs = group_blocks_into_provider_messages(
            MessageRole::Assistant,
            vec![],
            vec![],
            vec![text("Hello!")],
        );

        assert_eq!(msgs.len(), 1);
        assert!(matches!(msgs[0].role, ai_providers::Role::Assistant));
        assert_eq!(msgs[0].content.len(), 1);
        assert!(matches!(msgs[0].content[0], ContentBlock::Text { .. }));
    }

    /// User messages are never split — even if they (hypothetically) carried tool
    /// blocks, they're combined into one User message.
    #[test]
    fn user_messages_are_never_split() {
        let msgs = group_blocks_into_provider_messages(
            MessageRole::User,
            vec![],
            vec![],
            vec![text("question")],
        );

        assert_eq!(msgs.len(), 1);
        assert!(matches!(msgs[0].role, ai_providers::Role::User));
    }

    /// Empty blocks → no message emitted.
    #[test]
    fn no_blocks_emits_no_messages() {
        let msgs = group_blocks_into_provider_messages(
            MessageRole::Assistant,
            vec![],
            vec![],
            vec![],
        );
        assert!(msgs.is_empty());
    }

    /// Regression guard for the OLD bug: the previous implementation emitted
    /// `[Assistant { tool_use }, Tool { tool_result }, Assistant { text }]` —
    /// three messages, with the conversation ending on an Assistant turn. This
    /// test fails loudly if anyone reintroduces that order.
    #[test]
    fn no_trailing_assistant_message_after_tool_result() {
        let msgs = group_blocks_into_provider_messages(
            MessageRole::Assistant,
            vec![tool_use("call_1", "search")],
            vec![tool_result("call_1", "ok")],
            vec![text("Let me search.")],
        );
        // The LAST message must NOT be an Assistant turn — Anthropic rejects
        // conversations that end on an assistant message.
        let last = msgs.last().expect("at least one message");
        assert!(
            !matches!(last.role, ai_providers::Role::Assistant),
            "last message must not be Assistant (would break Anthropic no-prefill rule); \
             got role={:?}",
            last.role
        );
    }
}
