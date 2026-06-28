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
        // The persisted ids (available synchronously, before generation runs):
        // the user message (None if an extension suppressed it) + the assistant
        // message the reply streams into.
        Option<Uuid>,
        Uuid,
        Pin<Box<dyn Stream<Item = Result<ChatStreamChunk, AppError>> + Send>>,
        tokio::sync::mpsc::UnboundedReceiver<Result<axum::response::sse::Event, std::convert::Infallible>>,
    ), AppError>
    {
        // Create provider from model_id
        use crate::modules::chat::core::ai_provider::create_provider_from_model_id;

        let (provider, model_name, model_id, provider_id, model_params) =
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

            // Only `model_id` remains as opaque storage on the
            // message row. Per-message ASSISTANT attribution (was
            // `messages.assistant_id`) and MCP server snapshot (was
            // `messages.mcp_server_ids UUID[]`) moved into module-
            // owned join tables (migrations 74 + 75) — written by
            // each bridge's `after_user_message_created` hook a few
            // lines below. Chat no longer knows about either.
            let user_message = Repos.chat.core.create_message(
                branch_id,
                MessageRole::User.as_str(),
                Some(request.model_id),
            ).await?;

            // Give extensions a chance to persist per-message state
            // into their own tables (mcp's server snapshot, plus any
            // future similar bookkeeping). Runs OUTSIDE the message
            // INSERT transaction — a failure here leaves the message
            // saved without the extension's bookkeeping, which the
            // extension's read path handles by degrading to "use
            // current state."
            if let Some(registry) = &self.extension_registry {
                registry
                    .after_user_message_created(
                        &preliminary_context,
                        &user_message,
                        &request,
                    )
                    .await?;
            }

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
                let msg = Repos.chat.core.create_message(branch_id, MessageRole::Assistant.as_str(), None).await?;
                msg.id  // New message
            }
        } else {
            // No extension registry, create new message
            let msg = Repos.chat.core.create_message(branch_id, MessageRole::Assistant.as_str(), None).await?;
            msg.id  // New message
        };

        // Create channel for streaming output
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        // Create channel for extension events (titleUpdated / MCP tool events) —
        // raw SSE `Event`s. The detached consumer in `start_generation` forwards
        // each onto the chat-token stream via `publish_raw_event`. The `started`
        // frame is emitted by that consumer (it has the ids), so it is NOT sent
        // here.
        let (ext_tx, ext_rx) = tokio::sync::mpsc::unbounded_channel();

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
                                    // Fatal: corrupt content can't be enriched.
                                    // Surface the error and stop the whole task — a
                                    // bare `break` exits only this inner loop and
                                    // would wastefully keep generating off
                                    // partially-processed history.
                                    let _ = tx.send(Err(e));
                                    return;
                                }
                            };

                            // Allow extension to enrich content (e.g., add download URLs)
                            if let Err(e) = registry
                                .process_content_from_db(&mut content_data, &temp_context)
                                .await
                            {
                                let _ = tx.send(Err(e));
                                return;
                            }

                            // Update content in history
                            // Note: We're working with a temporary structure here, so we don't persist changes
                            // Extensions should use metadata or cache if they need to persist enriched data
                        }
                    }
                }

                // Model/provider metadata, built once and shared by BOTH the
                // history-replay transform context and the live stream context.
                // The transform context needs it so `process_content_for_llm`
                // can resolve the model's tool-capability during replay — that's
                // what drives the Track A recency-drop (old text attachments are
                // dropped from the replay for tool-capable models, since they're
                // listed in the injected manifest + read on demand). With an
                // empty map the capability check short-circuits to false and the
                // drop never fires, re-inlining every old attachment each turn.
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

                // Memoize the model's tool-capability into the shared metadata up
                // front (one DB+catalog lookup per iteration) so the per-block
                // replay path (process_content_for_llm) reads the cached boolean
                // instead of re-resolving it for every attachment block.
                let tool_capable =
                    crate::modules::file::available_files::ensure_model_tools_capable(
                        &mut context_metadata,
                    )
                    .await;

                // Resolve the conversation's available files ONCE per iteration
                // and seed them here, so the manifest injection (file
                // before_llm_call) and the replay recency-drop
                // (process_content_for_llm) share a SINGLE resolution and can't
                // disagree — a resolve failure makes both degrade to the safe
                // inline path rather than dropping content with no manifest.
                // Only the tool-capable path reads the seed (manifest + drop both
                // gate on tool-capability), so skip the 3-4 DB queries otherwise.
                if tool_capable {
                    crate::modules::file::available_files::seed_available_files(
                        &mut context_metadata,
                        conversation_id,
                        user_id,
                    )
                    .await;
                }

                // Create context for content transformation
                let transform_context = StreamContext {
                    conversation_id,
                    branch_id,
                    message_id: None,
                    user_id,
                    pool: pool.clone(),
                    metadata: context_metadata.clone(),
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

                // Create stream context (reuses the metadata built above; the
                // before_llm_call hooks further seed `model_tools_capable`).
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
                    ..Default::default()
                };
                // Apply model-level generation parameters (sampling), with defaults
                // preserved when unset. The provider gates params it rejects
                // (e.g. Anthropic Opus 4.7/4.8 drop temperature/top_p/top_k).
                apply_model_params(&mut chat_request, &model_params);
                // Registry-gated thinking (None for models that don't support it).
                chat_request.thinking =
                    thinking_config_for(provider_for_task.provider_type(), &model_name);
                // OpenAI prompt-cache routing key (ignored by other providers).
                chat_request.prompt_cache_key = Some(conversation_id.to_string());

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
                            // Approved tool returned audience=["user"] content before LLM call.
                            // Stream the final text directly and skip the LLM entirely.
                            tracing::info!("Skipping LLM call - extension provided final content");

                            if let Err(e) = Repos.chat.core.append_content(
                                assistant_message_id,
                                "text",
                                MessageContentData::Text { text: text.clone() },
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

                // Context trimming: once the assembled context grows past a
                // token threshold, clear the CONTENT of older tool_result blocks
                // (keeping the matching tool_use + the most recent K results) so
                // long agentic loops don't re-send every old tool output. Only
                // what's SENT is trimmed — stored history is untouched, and the
                // model can re-read a file (cheap) if it needs a cleared result.
                clear_old_tool_results(
                    &mut chat_request.messages,
                    CLEAR_TOOL_RESULTS_TOKEN_THRESHOLD,
                    KEEP_LAST_TOOL_RESULTS,
                );

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
                    reasoning_tokens: None,
                    extension_tx: Some(ext_tx.clone()),
                    finalized: false,
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
                                        // Receiver dropped mid-stream — the consumer
                                        // cancelled (or panicked). Persist whatever was
                                        // accumulated so the message isn't saved empty;
                                        // `finalize()` is idempotent via its `finalized`
                                        // flag, so this never double-writes.
                                        let _ = acc.finalize().await;
                                        return;
                                    }
                                }
                                Err(e) => {
                                    // Persist whatever streamed so far before
                                    // surfacing the error — `finalize()` is
                                    // idempotent, so this mirrors the
                                    // receiver-dropped path and stops a
                                    // mid-stream failure from discarding the
                                    // partial assistant message.
                                    let _ = acc.finalize().await;
                                    let _ = tx.send(Err(e));
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            // Provider stream errored mid-generation: persist
                            // the partial message (idempotent finalize) before
                            // reporting the error, same as the cancel path.
                            {
                                let mut acc = accumulator.lock().await;
                                let _ = acc.finalize().await;
                            }
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
                        // Tool result is a final user-facing answer (audience=["user"]).
                        // Emit the text as a delta, save it to the DB, then complete.

                        if let Err(e) = Repos.chat.core.append_content(
                            assistant_message_id,
                            "text",
                            MessageContentData::Text { text: text.clone() },
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

                        // Tool results are appended to the same assistant message.
                        // append_content assigns sequence_order atomically (MAX+1), so a
                        // result can never collide with the next iteration's tool_use even
                        // if the in-memory cache lags. Collect created rows to update cache.
                        let mut created_contents = Vec::new();

                        for content in assistant_message_content.iter() {
                            let content_type = content.content_type();
                            match Repos.chat.core.append_content(
                                assistant_message_id,
                                &content_type,
                                content.clone(),
                            ).await {
                                Ok(created) => {
                                    tracing::info!("Appended content block {} to assistant message", created.sequence_order);
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

        // Return the persisted ids + the channel receiver as a stream + the
        // extension event receiver. The generation runs detached in the spawned
        // task above; ids are known synchronously.
        Ok((
            user_message_id,
            assistant_message_id,
            Box::pin(UnboundedReceiverStream::new(rx)),
            ext_rx,
        ))
    }

    /// Fire-and-forget send: persist the user + assistant messages, return their
    /// ids immediately, and drive generation in a DETACHED consumer task that
    /// pushes live frames to the per-user chat-token stream (scoped to the
    /// subscribers of this conversation) instead of a response stream. The
    /// generation runs to completion even if the sender disconnects.
    pub async fn start_generation(
        &self,
        branch_id: Uuid,
        conversation_id: Uuid,
        user_id: Uuid,
        // Accepted for call-site symmetry with the streaming path; the
        // detached completion emit deliberately uses origin=None (see below).
        _origin_conn: Option<Uuid>,
        request: SendMessageRequest,
    ) -> Result<(Option<Uuid>, Uuid), AppError> {
        use crate::modules::chat::core::types::streaming::{
            SSEChatStreamCompleteData, SSEChatStreamErrorData, SSEChatStreamEvent,
            SSEChatStreamStartedData,
        };
        use crate::modules::chat::stream::{ChatStreamFrame, publish_frame};
        use crate::utils::cancellation::CANCELLATION_TRACKER;
        use futures_util::StreamExt as _;

        // Serialize: at most ONE in-flight generation per conversation. The
        // replay buffer is keyed by conversation and carries no message id to
        // demux two concurrent turns, so a second send (rapid double-send /
        // edit-while-generating) is rejected rather than corrupting the buffer.
        if !crate::modules::chat::stream::begin_generation(conversation_id) {
            return Err(AppError::new(
                axum::http::StatusCode::CONFLICT,
                "GENERATION_IN_PROGRESS",
                "A reply is already being generated for this conversation",
            ));
        }

        let (user_message_id, assistant_message_id, mut chunk_stream, mut ext_rx) =
            match self
                .send_message(branch_id, conversation_id, user_id, request)
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    // Setup failed before the streaming loop — release the slot.
                    crate::modules::chat::stream::end_generation(conversation_id);
                    return Err(e);
                }
            };

        // Stop-generation token, keyed by the assistant message id.
        let cancel_token = CANCELLATION_TRACKER.create_token(assistant_message_id).await;
        let owner_id = user_id;

        tokio::spawn(async move {
            // Backstop: if we never emit a terminal frame (panic / early exit),
            // the guard emits an Error (dropping the replay buffer) AND removes
            // the cancellation token, on every unwind path.
            let mut guard = TerminalGuard {
                owner_id,
                conversation_id,
                assistant_message_id,
                done: false,
            };

            let mut ext_open = true;

            // `started` frame — seeds the message on receiving devices and opens
            // the replay buffer for mid-stream join.
            publish_frame(
                owner_id,
                ChatStreamFrame::new(
                    conversation_id,
                    SSEChatStreamEvent::Started(SSEChatStreamStartedData {
                        user_message_id,
                        conversation_id,
                        branch_id,
                    }),
                ),
            );

            let mut cancelled = false;
            loop {
                tokio::select! {
                    maybe = chunk_stream.next() => match maybe {
                        Some(Ok(chunk)) => {
                            let terminal = chunk.finish_reason.is_some();
                            let event = if terminal {
                                SSEChatStreamEvent::Complete(SSEChatStreamCompleteData {
                                    finish_reason: chunk.finish_reason.clone().unwrap_or_default(),
                                    usage: chunk.usage.clone(),
                                })
                            } else {
                                SSEChatStreamEvent::Content(chunk)
                            };
                            publish_frame(owner_id, ChatStreamFrame::new(conversation_id, event));
                            if terminal {
                                guard.done = true;
                                break;
                            }
                        }
                        Some(Err(e)) => {
                            publish_frame(owner_id, ChatStreamFrame::new(
                                conversation_id,
                                SSEChatStreamEvent::Error(SSEChatStreamErrorData {
                                    message: e.to_string(),
                                    code: Some("STREAM_ERROR".into()),
                                }),
                            ));
                            guard.done = true;
                            break;
                        }
                        None => {
                            // The generation task ended without a terminal chunk.
                            // Every error path inside it sends an `Err` first (→ the
                            // arm above), so reaching `None` means a genuinely
                            // unexpected end (e.g. an early return / panic) — surface
                            // it as an error rather than silently completing.
                            publish_frame(owner_id, ChatStreamFrame::new(
                                conversation_id,
                                SSEChatStreamEvent::Error(SSEChatStreamErrorData {
                                    message: "Generation ended unexpectedly".into(),
                                    code: Some("STREAM_CLOSED".into()),
                                }),
                            ));
                            guard.done = true;
                            break;
                        }
                    },
                    // Forward extension events (titleUpdated, MCP tool start/
                    // complete/progress, approval/elicitation prompts, artifacts)
                    // — raw SSE events — onto the same per-conversation token
                    // stream so the human-in-the-loop tool gate + tool cards
                    // still work. The client routes them to whichever
                    // conversation the connection is subscribed to.
                    maybe_ext = ext_rx.recv(), if ext_open => match maybe_ext {
                        Some(Ok(raw)) => {
                            crate::modules::chat::stream::publish_raw_event(
                                owner_id,
                                conversation_id,
                                raw,
                            );
                        }
                        _ => ext_open = false,
                    },
                    _ = tokio::time::sleep(std::time::Duration::from_millis(250)) => {
                        if cancel_token.is_cancelled().await {
                            cancelled = true;
                            break;
                        }
                    }
                }
            }

            if cancelled {
                publish_frame(owner_id, ChatStreamFrame::new(
                    conversation_id,
                    SSEChatStreamEvent::Complete(SSEChatStreamCompleteData {
                        finish_reason: "cancelled".into(),
                        usage: None,
                    }),
                ));
                guard.done = true;
                // Dropping the chunk stream closes the generation's output
                // channel, stopping the still-running generation on its next send.
                drop(chunk_stream);
            }

            // Drain tail extension events (e.g. `titleUpdated`). These are emitted
            // synchronously from `finalize()`'s `after_llm_call` hook, which runs
            // BEFORE the terminal chunk is sent, so anything bound for this turn is
            // already enqueued by the time we observe the terminal frame — a
            // non-blocking drain suffices (no late event can still be in flight).
            while let Ok(Ok(raw)) = ext_rx.try_recv() {
                crate::modules::chat::stream::publish_raw_event(
                    owner_id,
                    conversation_id,
                    raw,
                );
            }

            CANCELLATION_TRACKER.remove_download(assistant_message_id).await;

            // Turn complete: notify the user's OTHER surfaces (sidebar list +
            // any device with this conversation NOT open) to refetch. `finalize`
            // committed before the terminal chunk, so the rows are fresh.
            crate::modules::sync::publish(
                crate::modules::sync::SyncEntity::Conversation,
                crate::modules::sync::SyncAction::Update,
                conversation_id,
                crate::modules::sync::Audience::owner(owner_id),
                // Detached completion task: emit with origin=None so EVERY
                // surface (incl. the originating connection's other tabs)
                // refetches the now-committed turn. (Convention: background/
                // completion emits never suppress the origin.)
                None,
            );
        });

        Ok((user_message_id, assistant_message_id))
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
                // Invariant: blocks arrive in strictly-increasing sequence_order (the
                // repository assigns it atomically via MAX+1). The pairing walk below
                // relies on this. If a regression ever reintroduces colliding/non-monotonic
                // orders, the tool_use/tool_result pairing can silently break and the
                // provider will reject the request — surface it loudly instead.
                let monotonic = msg_with_content
                    .contents
                    .windows(2)
                    .all(|w| w[0].sequence_order < w[1].sequence_order);
                if !monotonic {
                    tracing::warn!(
                        "non-monotonic sequence_order in assistant message {}; tool_use/tool_result pairing may be unreliable",
                        msg_with_content.message.id
                    );
                    debug_assert!(
                        monotonic,
                        "non-monotonic sequence_order in assistant message {}",
                        msg_with_content.message.id
                    );
                }

                // Convert each stored block to a provider ContentBlock (registry-driven),
                // then group into per-iteration Assistant/Tool pairs. Grouping is a pure
                // function (group_assistant_blocks) so the wire-format invariant is unit-testable.
                let mut blocks: Vec<ai_providers::ContentBlock> = Vec::new();
                for content in &msg_with_content.contents {
                    let content_data = content.parse_content()?;

                    // Let any extension claim this content as
                    // skip-from-assistant-forwarding. The file extension
                    // does this for `FileAttachment` blocks produced from
                    // MCP tool results (UI-only artifacts the LLM already
                    // heard about via ToolResult). Used to be a chat-side
                    // `matches!(MessageContentData::FileAttachment { .. })`
                    // — naming an extension-contributed variant in chat's
                    // source. The skip decision now lives in the
                    // extension that owns the variant.
                    if let Some(registry) = extension_registry
                        && registry
                            .should_skip_in_assistant_forwarding(&content_data, context)
                            .await?
                    {
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
                        blocks.push(b);
                    }
                }

                messages.extend(group_assistant_blocks(blocks));

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

/// Build a thinking config from the model registry, or `None` when the model is
/// unknown or doesn't support thinking. Registry-driven only (no DB/UI toggle).
fn thinking_config_for(provider_type: &str, model_id: &str) -> Option<ai_providers::ThinkingConfig> {
    use ai_providers::{ThinkingConfig, ThinkingEffort, ThinkingMode};
    let caps = ai_providers::registry_lookup(provider_type, model_id)?;
    if caps.supports_thinking != Some(true) {
        return None;
    }
    let cfg = if caps.thinking_style.as_deref() == Some("budget") {
        ThinkingConfig {
            mode: ThinkingMode::Enabled,
            budget_tokens: Some(4096),
            effort: Some(ThinkingEffort::High),
            include_thinking: true,
        }
    } else {
        ThinkingConfig {
            mode: ThinkingMode::Adaptive,
            budget_tokens: None,
            effort: Some(ThinkingEffort::High),
            include_thinking: true,
        }
    };
    Some(cfg)
}

/// Map model-level generation parameters onto a `ChatRequest`. Defaults preserve
/// the historical behavior (`temperature` 0.7 / `max_tokens` 8192) when unset.
fn apply_model_params(
    req: &mut ai_providers::ChatRequest,
    p: &crate::modules::llm_model::models::ModelParameters,
) {
    req.temperature = p.temperature.or(Some(0.7));
    // Guard against a negative/zero i32 wrapping to a huge u32; fall back to the default.
    req.max_tokens = p
        .max_tokens
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n > 0)
        .or(Some(8192));
    req.top_p = p.top_p;
    req.top_k = p.top_k;
    req.presence_penalty = p.presence_penalty;
    req.frequency_penalty = p.frequency_penalty;
    req.seed = p.seed;
    req.stop = p.stop.clone().filter(|s| !s.is_empty());
}

/// Accumulated content block in memory
#[derive(Debug, Clone)]
struct AccumulatedContent {
    content_type: String,
    accumulated_text: String,
    index: usize,
    /// Anthropic thinking-block signature (captured from `signature_delta`).
    signature: Option<String>,
    /// Redacted-thinking opaque data (captured from a redacted_thinking block).
    redacted_data: Option<String>,
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
    /// Reasoning/thinking tokens reported by the provider (final chunk).
    reasoning_tokens: Option<u32>,
    /// Channel for extension events (SSE)
    extension_tx: Option<tokio::sync::mpsc::UnboundedSender<Result<axum::response::sse::Event, std::convert::Infallible>>>,
    /// Flag to track if finalize() has been called (prevents double-finalization)
    finalized: bool,
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
            self.reasoning_tokens = usage.reasoning_tokens;
            self.usage = Some(usage.clone().into());
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
            // Thinking signature / redacted-thinking: attach to the in-memory
            // block and do NOT stream to the client (not user-visible).
            match ai_delta {
                ai_providers::ContentBlockDelta::ThinkingSignatureDelta { index, signature } => {
                    self.ensure_content_block_exists(*index, "thinking");
                    if let Some(block) = self.content_blocks.get_mut(*index) {
                        block.signature = Some(signature.clone());
                    }
                    continue;
                }
                ai_providers::ContentBlockDelta::RedactedThinkingDelta { index, data } => {
                    self.ensure_content_block_exists(*index, "thinking");
                    if let Some(block) = self.content_blocks.get_mut(*index) {
                        block.redacted_data = Some(data.clone());
                    }
                    continue;
                }
                _ => {}
            }

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
                signature: None,
                redacted_data: None,
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

        // Determine the starting sequence_order for THIS iteration's blocks directly
        // from the DB (MAX+1), inside the transaction. Previously this came from a
        // stale in-memory `content_offset`, which on the parallel-tool-call path
        // could lag behind tool_results the Continue handler had already appended,
        // making a later tool_use collide with an earlier tool_result. The streaming
        // indices below stay relative to this fresh base, preserving in-response order.
        let base: i32 = sqlx::query_scalar!(
            r#"SELECT COALESCE(MAX(sequence_order), -1) + 1 AS "next!"
               FROM message_contents WHERE message_id = $1"#,
            self.assistant_message_id
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

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
                    metadata: if accumulated.signature.is_some()
                        || accumulated.redacted_data.is_some()
                        || self.reasoning_tokens.is_some()
                    {
                        Some(crate::modules::chat::extensions::text::types::ThinkingMetadata {
                            token_count: self.reasoning_tokens,
                            signature: accumulated.signature.clone(),
                            redacted_data: accumulated.redacted_data.clone(),
                        })
                    } else {
                        None
                    },
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
                base + accumulated.index as i32
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
                    base + index as i32
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
                    tracing::error!("Extension error in after_llm_call: {}", e);
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

/// Group ONE assistant message's already-converted blocks into provider-ready
/// ChatMessages, reconstructing per-iteration boundaries. Each time every
/// outstanding tool_use has received its tool_result, one
/// `[Assistant { text + tool_use }, Tool { tool_result }]` pair is flushed.
/// Trailing unmatched blocks (in-progress iteration, pure-text final answer, or
/// an approval-flow tool_use awaiting its result) become a final Assistant turn.
///
/// Pure + registry-free so the wire-format invariant — every tool_use in an
/// Assistant turn is resolved by a tool_result in the immediately following Tool
/// turn — is directly unit-testable. Assumes blocks arrive in `sequence_order`
/// (guaranteed by the repository's atomic MAX+1 assignment).
pub fn group_assistant_blocks(blocks: Vec<ai_providers::ContentBlock>) -> Vec<ChatMessage> {
    let mut messages = Vec::new();
    let mut current_text: Vec<ai_providers::ContentBlock> = Vec::new();
    let mut current_tool_uses: Vec<ai_providers::ContentBlock> = Vec::new();
    let mut pending_ids: std::collections::HashSet<String> = Default::default();
    let mut current_results: Vec<ai_providers::ContentBlock> = Vec::new();

    for b in blocks {
        match &b {
            ai_providers::ContentBlock::ToolUse { id, .. } => {
                pending_ids.insert(id.clone());
                current_tool_uses.push(b);
            }
            ai_providers::ContentBlock::ToolResult { tool_use_id, .. } => {
                pending_ids.remove(tool_use_id);
                current_results.push(b);
                // All outstanding tool_uses resolved — flush one Assistant/Tool pair.
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
                        content: std::mem::take(&mut current_results),
                    });
                }
            }
            _ => current_text.push(b),
        }
    }

    let trailing: Vec<_> = current_text.into_iter().chain(current_tool_uses).collect();
    if !trailing.is_empty() {
        messages.push(ChatMessage {
            role: ai_providers::Role::Assistant,
            content: trailing,
        });
    }

    messages
}

/// Panic/early-exit backstop for the detached generation consumer. If the
/// consumer task ends WITHOUT having emitted a terminal frame (a panic, or an
/// unexpected drop), this emits an `error` frame on drop so the conversation's
/// replay buffer is always released. `publish_frame` is synchronous, so it is
/// safe to call from `Drop`.
struct TerminalGuard {
    owner_id: Uuid,
    conversation_id: Uuid,
    assistant_message_id: Uuid,
    done: bool,
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        use crate::modules::chat::core::types::streaming::{
            SSEChatStreamErrorData, SSEChatStreamEvent,
        };
        // Emit a terminal Error frame — drops the conversation's replay buffer
        // (and releases the in-flight generation slot).
        crate::modules::chat::stream::publish_frame(
            self.owner_id,
            crate::modules::chat::stream::ChatStreamFrame::new(
                self.conversation_id,
                SSEChatStreamEvent::Error(SSEChatStreamErrorData {
                    message: "Generation task aborted".into(),
                    code: Some("STREAM_ABORTED".into()),
                }),
            ),
        );
        // Remove the cancellation token, which the normal path awaits at the end
        // of the task — skipped on an unwind, so do it here (spawn since Drop is
        // sync and removal is async). Without this the tracker map would leak.
        let id = self.assistant_message_id;
        tokio::spawn(async move {
            crate::utils::cancellation::CANCELLATION_TRACKER
                .remove_download(id)
                .await;
        });
    }
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

    #[test]
    fn thinking_config_for_registry_gated() {
        use ai_providers::ThinkingMode;
        // Adaptive thinking model.
        let cfg = thinking_config_for("anthropic", "claude-opus-4-7").expect("thinking enabled");
        assert_eq!(cfg.mode, ThinkingMode::Adaptive);
        assert!(cfg.effort.is_some());
        // Non-thinking model.
        assert!(thinking_config_for("openai", "gpt-4o").is_none());
        // Unknown model.
        assert!(thinking_config_for("anthropic", "no-such-model").is_none());
    }

    #[test]
    fn apply_model_params_maps_and_defaults() {
        use crate::modules::llm_model::models::ModelParameters;

        // Configured params flow through.
        let mut req = ai_providers::ChatRequest::default();
        let params = ModelParameters {
            temperature: Some(0.3),
            top_k: Some(20),
            stop: Some(vec!["END".into()]),
            ..Default::default()
        };
        apply_model_params(&mut req, &params);
        assert_eq!(req.temperature, Some(0.3));
        assert_eq!(req.top_k, Some(20));
        assert_eq!(req.stop, Some(vec!["END".to_string()]));

        // Empty params fall back to the historical defaults.
        let mut req2 = ai_providers::ChatRequest::default();
        apply_model_params(&mut req2, &ModelParameters::default());
        assert_eq!(req2.temperature, Some(0.7));
        assert_eq!(req2.max_tokens, Some(8192));
        assert!(req2.top_k.is_none());
    }

    #[test]
    fn thinking_block_groups_before_tool_use() {
        // A thinking block must precede tool_use in the assembled assistant turn.
        let msgs = group_assistant_blocks(vec![
            ai_providers::ContentBlock::Thinking {
                thinking: "reasoning".into(),
                signature: Some("sig".into()),
            },
            tool_use("call_1", "search"),
        ]);
        let assistant = msgs
            .iter()
            .find(|m| matches!(m.role, ai_providers::Role::Assistant))
            .expect("assistant message");
        let first_kinds: Vec<_> = assistant
            .content
            .iter()
            .map(|b| matches!(b, ai_providers::ContentBlock::Thinking { .. }))
            .collect();
        assert!(first_kinds.first().copied().unwrap_or(false), "thinking must come first");
    }
}

// ── Context trimming ─────────────────────────────────────────────────────────

/// Clear old tool_result content once the assembled context exceeds this many
/// estimated tokens (chars/4). ~à la Anthropic's `clear_tool_uses` default.
const CLEAR_TOOL_RESULTS_TOKEN_THRESHOLD: usize = 30_000;
/// How many of the most-recent tool_result blocks to keep intact.
const KEEP_LAST_TOOL_RESULTS: usize = 6;
/// Per-result char ceiling for a KEPT tool_result. The keep-last-K window
/// is a fixed COUNT, so a handful of oversized recent results could still
/// blow the context budget. Truncate any kept result whose text payload
/// exceeds this; the model can re-call the tool to see the full output.
const MAX_KEPT_TOOL_RESULT_CHARS: usize = 8000;
/// Marker appended to a kept-but-truncated tool_result. `{id}` is the
/// `tool_use_id`, so the model can recover the FULL result exactly via the
/// `get_tool_result` tool (a read of stored history — no re-execution).
fn kept_truncation_marker(tool_use_id: &str) -> String {
    format!(
        "\n[…truncated to save context; call get_tool_result(tool_use_id=\"{tool_use_id}\") for the full result]"
    )
}

/// Rough char count of a content block's text payload (for token estimation).
fn block_text_chars(b: &ai_providers::ContentBlock) -> usize {
    use ai_providers::ContentBlock as CB;
    match b {
        CB::Text { text } => text.chars().count(),
        CB::Thinking { thinking, .. } => thinking.chars().count(),
        CB::ToolUse { input, .. } => input.to_string().chars().count(),
        CB::ToolResult { content, .. } => content.iter().map(block_text_chars).sum(),
        _ => 0,
    }
}

/// When the assembled context exceeds `threshold_tokens`, replace the *content*
/// of older `tool_result` blocks with a short placeholder, keeping the matching
/// `tool_use` blocks and the most recent `keep_last` results intact. Mutates only
/// the outbound messages — stored history is untouched and the model can re-call
/// a tool (e.g. `read_file`) if it needs a cleared result. Provider-agnostic.
fn clear_old_tool_results(
    messages: &mut [ChatMessage],
    threshold_tokens: usize,
    keep_last: usize,
) {
    let total_chars: usize = messages
        .iter()
        .flat_map(|m| m.content.iter())
        .map(block_text_chars)
        .sum();
    // Shared chars→tokens heuristic (ceil/4), same as the summarizer.
    if crate::common::tokens::tokens_from_chars(total_chars) <= threshold_tokens {
        return;
    }

    // Positions of every tool_result block, in order.
    let mut positions: Vec<(usize, usize)> = Vec::new();
    for (mi, m) in messages.iter().enumerate() {
        for (bi, b) in m.content.iter().enumerate() {
            if matches!(b, ai_providers::ContentBlock::ToolResult { .. }) {
                positions.push((mi, bi));
            }
        }
    }
    // Older results (everything before the keep-last window) get their CONTENT
    // replaced with a placeholder. `saturating_sub` yields 0 when there are
    // `<= keep_last` results, so nothing OLD is cleared in that case — but the
    // kept-window cap below still runs (a handful of oversized recent results
    // can blow the budget on their own).
    let clear_until = positions.len().saturating_sub(keep_last);
    for &(mi, bi) in &positions[..clear_until] {
        if let ai_providers::ContentBlock::ToolResult { content, tool_use_id, .. } =
            &mut messages[mi].content[bi]
        {
            // Carry the tool_use_id so the model can recover the EXACT result via
            // get_tool_result (read of stored history; no re-execution).
            let tid = tool_use_id.clone();
            *content = vec![ai_providers::ContentBlock::Text {
                text: format!(
                    "[tool result cleared to save context — call get_tool_result(tool_use_id=\"{tid}\") to retrieve the full result]"
                ),
            }];
        }
    }

    // Bound the KEPT window too: keep-last is a fixed count, so even the
    // surviving results can blow the budget if a few are oversized. Truncate
    // any kept tool_result whose text payload exceeds the per-result ceiling.
    // The matching tool_use is left intact, and the model can re-call the tool
    // to recover the full output. Outbound copy only — stored history is not
    // touched here.
    for &(mi, bi) in &positions[clear_until..] {
        if let ai_providers::ContentBlock::ToolResult { content, tool_use_id, .. } =
            &mut messages[mi].content[bi]
        {
            let chars: usize =
                content.iter().map(block_text_chars).sum();
            if chars > MAX_KEPT_TOOL_RESULT_CHARS {
                let tid = tool_use_id.clone();
                let truncated = truncate_kept_result(content, &tid);
                // Preserve any Image blocks — only the oversized TEXT is
                // truncated. Image bytes aren't counted in the char budget
                // (block_text_chars is text-only), and a tool's returned image
                // (chart/figure) is usually the valuable part, so dropping it on
                // a text-size overflow would lose model-relevant content.
                let images: Vec<ai_providers::ContentBlock> = content
                    .iter()
                    .filter(|b| matches!(b, ai_providers::ContentBlock::Image { .. }))
                    .cloned()
                    .collect();
                let mut new_content = vec![ai_providers::ContentBlock::Text { text: truncated }];
                new_content.extend(images);
                *content = new_content;
            }
        }
    }
}

/// Flatten a kept tool_result's text payload and cut it to
/// `MAX_KEPT_TOOL_RESULT_CHARS`, appending the re-call marker. Char-safe
/// (truncates on a `char` boundary, not a byte index).
fn truncate_kept_result(
    content: &[ai_providers::ContentBlock],
    tool_use_id: &str,
) -> String {
    use ai_providers::ContentBlock as CB;
    let mut flat = String::new();
    for b in content {
        match b {
            CB::Text { text } => flat.push_str(text),
            CB::Thinking { thinking, .. } => flat.push_str(thinking),
            CB::ToolUse { input, .. } => {
                flat.push_str(&input.to_string())
            }
            _ => {}
        }
    }
    let kept: String =
        flat.chars().take(MAX_KEPT_TOOL_RESULT_CHARS).collect();
    format!("{kept}{}", kept_truncation_marker(tool_use_id))
}

#[cfg(test)]
mod trim_tests {
    use super::*;
    use ai_providers::{ChatMessage, ContentBlock, Role};

    fn tool_result_msg(id: &str, text: &str) -> ChatMessage {
        ChatMessage {
            role: Role::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: id.to_string(),
                name: None,
                content: vec![ContentBlock::Text {
                    text: text.to_string(),
                }],
                is_error: None,
            }],
        }
    }

    fn is_cleared(m: &ChatMessage) -> bool {
        matches!(&m.content[0], ContentBlock::ToolResult { content, .. }
            if matches!(&content[0], ContentBlock::Text { text } if text.contains("cleared")))
    }

    #[test]
    fn clears_old_keeps_recent_past_threshold() {
        let big = "x".repeat(400); // ~100 est tokens each
        let mut msgs: Vec<ChatMessage> = (0..10)
            .map(|i| tool_result_msg(&format!("t{i}"), &big))
            .collect();
        // ~1000 tokens total, threshold 100 → trim; keep last 2.
        clear_old_tool_results(&mut msgs, 100, 2);
        assert!(is_cleared(&msgs[0]), "oldest cleared");
        assert!(is_cleared(&msgs[7]), "8th-from-end cleared");
        assert!(!is_cleared(&msgs[8]), "kept last 2");
        assert!(!is_cleared(&msgs[9]), "kept last 2");
    }

    #[test]
    fn noop_under_threshold() {
        let mut msgs = vec![tool_result_msg("t", "small")];
        clear_old_tool_results(&mut msgs, 30_000, 2);
        assert!(!is_cleared(&msgs[0]), "nothing trimmed under threshold");
    }

    #[test]
    fn noop_when_fewer_than_keep_last() {
        let big = "x".repeat(4000); // way over threshold
        let mut msgs = vec![tool_result_msg("a", &big), tool_result_msg("b", &big)];
        clear_old_tool_results(&mut msgs, 100, 6);
        assert!(!is_cleared(&msgs[0]));
        assert!(!is_cleared(&msgs[1]));
    }

    fn result_text_chars(m: &ChatMessage) -> usize {
        match &m.content[0] {
            ContentBlock::ToolResult { content, .. } => {
                content.iter().map(block_text_chars).sum()
            }
            _ => 0,
        }
    }

    #[test]
    fn caps_oversized_kept_results() {
        // Several oversized results, all within the keep-last window, so the
        // count-based clear leaves them untouched — the per-result cap must
        // still bound them.
        let huge = "y".repeat(50_000); // ~12.5k est tokens each
        let mut msgs: Vec<ChatMessage> = (0..6)
            .map(|i| tool_result_msg(&format!("k{i}"), &huge))
            .collect();
        // keep_last == count, so clear_until == 0: nothing is cleared, but
        // every kept result is over MAX_KEPT_TOOL_RESULT_CHARS.
        clear_old_tool_results(&mut msgs, 100, 6);

        for (i, m) in msgs.iter().enumerate() {
            assert!(!is_cleared(m), "result {i} should be kept, not cleared");
            let chars = result_text_chars(m);
            assert!(
                chars
                    <= MAX_KEPT_TOOL_RESULT_CHARS
                        + kept_truncation_marker("k0").chars().count(),
                "kept result {i} not bounded: {chars} chars",
            );
        }

        // The post-trim estimate must stay bounded: 6 results each at most
        // (cap + marker) chars, /4 for the token estimate.
        let total_chars: usize = msgs
            .iter()
            .flat_map(|m| m.content.iter())
            .map(block_text_chars)
            .sum();
        let bound = 6
            * (MAX_KEPT_TOOL_RESULT_CHARS
                + kept_truncation_marker("k0").chars().count());
        assert!(
            total_chars <= bound,
            "post-trim total {total_chars} exceeds bound {bound}",
        );
    }

    #[test]
    fn small_kept_results_not_truncated() {
        // Oversized older results get cleared; the kept tail is small and must
        // NOT pick up a truncation marker.
        let big = "z".repeat(400);
        let mut msgs: Vec<ChatMessage> = (0..10)
            .map(|i| tool_result_msg(&format!("s{i}"), &big))
            .collect();
        clear_old_tool_results(&mut msgs, 100, 2);
        // Last two kept and well under the cap → untouched text.
        for m in &msgs[8..] {
            assert!(!is_cleared(m));
            let txt = match &m.content[0] {
                ContentBlock::ToolResult { content, .. } => match &content[0] {
                    ContentBlock::Text { text } => text.clone(),
                    _ => String::new(),
                },
                _ => String::new(),
            };
            assert!(
                !txt.contains("truncated to save context"),
                "small kept result should not be truncated",
            );
        }
    }

    // audit id all-643c33d76832 — trimming→recall ROUNDTRIP handoff. Existing
    // tests assert a cleared block merely contains "cleared"; this asserts the
    // placeholder names the EXACT tool_use_id in a get_tool_result(...) call so
    // the model can recall the full result from stored history (the bridge to
    // tool_result_mcp::get_tool_result). A wrong/missing id = a dead pointer.
    #[test]
    fn cleared_placeholder_carries_get_tool_result_recall_pointer() {
        let mut msgs: Vec<ChatMessage> = (0..10)
            .map(|i| tool_result_msg(&format!("tu-{i}"), &"x".repeat(2000)))
            .collect();
        clear_old_tool_results(&mut msgs, 100, 2);

        let ContentBlock::ToolResult { content, .. } = &msgs[0].content[0] else {
            panic!("expected a ToolResult block");
        };
        let ContentBlock::Text { text } = &content[0] else {
            panic!("cleared content should be a single Text placeholder");
        };
        let needle = format!("get_tool_result(tool_use_id=\"{}\")", "tu-0");
        assert!(
            text.contains(&needle),
            "placeholder must carry the recall pointer for its own tool_use_id: {text}"
        );
    }
}
