// Streaming service infrastructure

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

        let (provider, model_name, model_id, provider_id, model_params, model_caps) =
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

            // Whether ANY iteration of this turn persisted a user-visible answer.
            // The per-iteration `DeltaAccumulator` is recreated each loop, so its
            // own `produced_visible_content` reflects only the latest LLM response;
            // this OR-accumulates across iterations so the terminal
            // "empty completion" signal matches the WHOLE assistant message (which
            // is what the frontend inspects), not just the final iteration.
            let mut turn_produced_visible_content = false;

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
                // Apply model-level generation parameters (sampling). Temperature
                // is forwarded only when the user set it (never force-injected);
                // the provider adapter then gates params a model rejects.
                apply_model_params(&mut chat_request, &model_params);
                // Per-model capability override (editable DB row) → the top-priority
                // source of the parameter contract. The adapter falls back to the
                // curated catalog + provider model-family policy when a field is unset.
                chat_request.model_caps = Some(model_caps.to_param_contract());
                // Thinking resolved from the row → catalog → family policy.
                chat_request.thinking = thinking_config_for(
                    provider_for_task.provider_type(),
                    &model_name,
                    &model_caps,
                );
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

                // Wire-format invariant: exactly one tool_result per tool_use_id.
                // Runs HERE because this is after every extension mutation
                // (call_before_llm_call above may append tool_results) and before
                // the provider call below — the only point that sees the final
                // request. Ordered before the trimming: keep-last-K counts
                // tool_result blocks, so it must count the deduped set.
                let dropped_dupes = dedup_tool_results_by_id(&mut chat_request.messages);
                if !dropped_dupes.is_empty() {
                    // Should never fire: every known duplicate source is fixed at its
                    // origin. If it does, this names the conversation to investigate.
                    tracing::warn!(
                        conversation_id = %conversation_id,
                        message_id = %assistant_message_id,
                        iteration,
                        "dropped {} duplicate tool_result block(s) for tool_use_id(s) {:?} \
                         within one tool batch — a second result for one tool_use is \
                         rejected by the provider. The surviving (first) result is the one \
                         paired with its tool_use; this indicates a duplicate SOURCE that \
                         should be fixed at its origin.",
                        dropped_dupes.len(),
                        dropped_dupes,
                    );
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
                    produced_visible_content: false,
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
                    // Carry this iteration's visibility into the turn-level flag
                    // (the accumulator is dropped/recreated next iteration).
                    if acc.produced_visible_content {
                        turn_produced_visible_content = true;
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

                        // Canonicalize the provider's raw finish_reason into the
                        // unified vocabulary (stop/length/tool_calls/…) for the
                        // client, or default to "stop" if not provided. MCP
                        // sampling reads the raw provider value on its own path.
                        let mut final_finish_reason = finish_reason
                            .map(|r| {
                                ai_providers::FinishReason::canonicalize(
                                    ai_providers::ProviderFamily::from_provider_type(
                                        provider_for_task.provider_type(),
                                    ),
                                    &r,
                                )
                            })
                            .unwrap_or_else(|| "stop".to_string());

                        // Empty-completion guard: the turn terminated (no tool call to
                        // run) but produced NO user-visible answer across ANY iteration
                        // — only reasoning, or nothing. Without this the client just
                        // sees a bare `stop` and the chat appears to hang. Override the
                        // terminal finish_reason to "empty" (an authoritative signal for
                        // telemetry + the client) and log it. The frontend renders an
                        // inline notice for such a turn; it does NOT branch on
                        // finish_reason, so this is non-breaking. We deliberately do NOT
                        // emit an Err here (that would render as a hard
                        // conversation-level error banner).
                        if !turn_produced_visible_content {
                            tracing::warn!(
                                conversation_id = %conversation_id,
                                message_id = %assistant_message_id,
                                provider_finish_reason = %final_finish_reason,
                                "chat turn completed with no user-visible content and no tool call (empty completion)"
                            );
                            final_finish_reason = "empty".to_string();
                        }

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

}

/// Build a thinking config, or `None` when the model doesn't support thinking.
/// The thinking style is resolved dynamically: the editable DB model row →
/// curated catalog → provider model-family policy (so a user can enable thinking
/// on an uncatalogued model, and new models in a known family are auto-covered).
fn thinking_config_for(
    provider_type: &str,
    model_id: &str,
    caps: &crate::modules::llm_model::models::ModelCapabilities,
) -> Option<ai_providers::ThinkingConfig> {
    use ai_providers::{ThinkingConfig, ThinkingEffort, ThinkingMode};
    let family = ai_providers::ProviderFamily::from_provider_type(provider_type);
    let contract = caps.to_param_contract();
    let style = ai_providers::resolved_thinking_style(family, model_id, &contract)?;
    let cfg = if style == "budget" {
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

/// Map model-level generation parameters onto a `ChatRequest`. `temperature` is
/// forwarded ONLY when the user configured it — never force-injected — so a model
/// that rejects a non-default temperature isn't sent one it never asked for. The
/// provider adapter then gates params a model rejects. `max_tokens` keeps its
/// required-field default (Anthropic mandates the field).
fn apply_model_params(
    req: &mut ai_providers::ChatRequest,
    p: &crate::modules::llm_model::models::ModelParameters,
) {
    req.temperature = p.temperature;
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
    /// Anthropic thinking-block signature (captured from `signature_delta`).
    signature: Option<String>,
    /// Redacted-thinking opaque data (captured from a redacted_thinking block).
    redacted_data: Option<String>,
}

/// Reconstruct the provider `ContentBlock` that produced an accumulated block,
/// so the extension registry can convert it back into `MessageContentData` at
/// persist time (the reverse of the streamed-delta accumulation). Only the
/// built-in text/thinking shapes are reconstructable here; extension-owned
/// content types return `None` and persist via `get_accumulated_content`.
fn accumulated_to_content_block(
    accumulated: &AccumulatedContent,
) -> Option<ai_providers::ContentBlock> {
    match accumulated.content_type.as_str() {
        "text" => Some(ai_providers::ContentBlock::Text {
            text: accumulated.accumulated_text.clone(),
        }),
        // A redacted-thinking block carries opaque `data` (no text/signature);
        // a normal thinking block carries text + an optional signature.
        "thinking" => Some(match &accumulated.redacted_data {
            Some(data) => ai_providers::ContentBlock::RedactedThinking { data: data.clone() },
            None => ai_providers::ContentBlock::Thinking {
                thinking: accumulated.accumulated_text.clone(),
                signature: accumulated.signature.clone(),
            },
        }),
        _ => None,
    }
}

/// Does a persisted content block count as a **user-visible answer** for this turn?
///
/// Used to distinguish a turn that produced something for the user (text / tool
/// call / attachment) from a reasoning-only or empty turn. `thinking` blocks and
/// empty/whitespace `text` do NOT count — they are what make a turn *appear* to
/// hang with nothing shown. Any non-reasoning content type counts; the `text`
/// param is only consulted for the `"text"` type (extension blocks pass "").
fn is_visible_answer(content_type: &str, text: &str) -> bool {
    match content_type {
        "thinking" => false,
        "text" => !text.trim().is_empty(),
        // tool_use / tool_result / image / file_attachment / elicitation_request /
        // any other extension-owned content type is a user-visible answer.
        _ => true,
    }
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
    /// Set true during `finalize` when this turn persisted at least one
    /// user-visible answer block (text with content, tool_use, attachment, …).
    /// Stays false for a reasoning-only or empty turn — the terminal arm uses it
    /// to surface an "empty completion" notice instead of a silent `stop`.
    produced_visible_content: bool,
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

        // Assign sequence_order from a single running counter rather than
        // `base + block.index`: the per-block `index` values come from
        // independent sources (the streamed content blocks AND each extension's
        // own accumulator), so they can overlap and would collide on
        // (message_id, sequence_order). Insertion order here already reflects
        // the intended render order (content blocks first, then extension
        // content), so a monotonic counter preserves order while guaranteeing
        // the UNIQUE constraint (migration 124) is never tripped.
        let mut next_seq = base;

        for accumulated in &self.content_blocks {
            // Skip empty content blocks
            if accumulated.content_type.is_empty() {
                continue;
            }

            // Prefer the extension registry to convert the reconstructed provider
            // ContentBlock back into MessageContentData — the reverse of
            // `convert_extension_content`. This keeps the persist path delegating
            // to each extension's own converter (the text extension owns
            // text/thinking/redacted-thinking) instead of hardcoding the mapping
            // here. The inline `match` below is the fallback for a `ChatService`
            // built without `with_extensions` (no registry attached), preserving
            // the original behavior byte-for-byte.
            let content_data = accumulated_to_content_block(accumulated)
                .as_ref()
                .and_then(|block| {
                    self.extension_registry
                        .as_ref()
                        .and_then(|registry| registry.convert_from_content_block(block))
                })
                .map(|mut content| {
                    // The stateless block converter can't see the
                    // provider-reported reasoning-token count; restore it onto
                    // thinking blocks so it still lands in the persisted metadata.
                    if let MessageContentData::Thinking { metadata, .. } = &mut content {
                        if let Some(tokens) = self.reasoning_tokens {
                            metadata
                                .get_or_insert_with(|| {
                                    crate::modules::chat::extensions::text::types::ThinkingMetadata {
                                        token_count: None,
                                        signature: None,
                                        redacted_data: None,
                                    }
                                })
                                .token_count = Some(tokens);
                        }
                    }
                    content
                });

            let content_data = match content_data {
                Some(content) => content,
                // No registry attached (or no extension claimed the block):
                // fall back to the direct construction.
                None => match accumulated.content_type.as_str() {
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
                },
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
                next_seq
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
            next_seq += 1;

            if is_visible_answer(&accumulated.content_type, &accumulated.accumulated_text) {
                self.produced_visible_content = true;
            }
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

            for (_index, content_data) in extension_content {
                let content_type = content_data.content_type();
                // Use to_api_content() to flatten Extension variants
                let content_json = content_data.to_api_content();

                tracing::info!(
                    "Persisting extension content at index {}: type={}",
                    _index,
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
                    next_seq
                )
                .execute(&mut *tx)
                .await
                .map_err(AppError::database_error)?;
                next_seq += 1;

                // Extension content (tool_use / tool_result / attachments / …) is a
                // user-visible answer. Extract any `text` field so an extension that
                // emits a text-typed block (the text extension declares text/thinking)
                // is classified on its actual content, not assumed non-empty.
                let text = content_json
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if is_visible_answer(&content_type, text) {
                    self.produced_visible_content = true;
                }
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
// Exercised only by the unit tests below (the production stream path groups
// blocks inline), so compile it under test cfg to keep it dead-code-clean.
#[cfg(test)]
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

/// A `tool_result` standing in for a `tool_use` whose real result is absent from
/// the stored assistant message. EVERY provider (Anthropic/OpenAI/Gemini) rejects
/// a request that carries a `tool_use` with no matching `tool_result`, so rather
/// than drop the tool_use (which loses the model's own reasoning) we answer it
/// with an explicit `is_error` placeholder. Carries the tool_use's `name` so
/// Gemini's name-based `functionResponse` pairing stays correct.
fn synthetic_missing_tool_result(
    tool_use_id: &str,
    name: Option<String>,
) -> ai_providers::ContentBlock {
    ai_providers::ContentBlock::ToolResult {
        tool_use_id: tool_use_id.to_string(),
        name,
        content: vec![ai_providers::ContentBlock::Text {
            text: "Tool result unavailable (no result was recorded for this tool call)."
                .to_string(),
        }],
        is_error: Some(true),
    }
}

/// Emit one `[Assistant { text + tool_use }, Tool { tool_result }]` pair, draining
/// the accumulators. The Tool turn carries EXACTLY one result per tool_use, in
/// tool_use order — the real result when we captured it, otherwise a synthesized
/// `is_error` placeholder (`synthetic_missing_tool_result`). This is what guarantees
/// the wire-format invariant regardless of whether the tools succeeded, failed, or
/// left a gap.
///
/// `results_by_id` is drained here and is empty on return: the caller's capture guard
/// only ever inserts a result whose tool_use is OUTSTANDING in this batch, so there
/// are no orphans to leave behind (a `tool_result` with no preceding `tool_use` is
/// itself an invalid provider block, and is refused at capture rather than filtered
/// out here).
fn flush_assistant_tool_pair(
    messages: &mut Vec<ChatMessage>,
    current_text: &mut Vec<ai_providers::ContentBlock>,
    current_tool_uses: &mut Vec<ai_providers::ContentBlock>,
    results_by_id: &mut std::collections::HashMap<String, ai_providers::ContentBlock>,
) {
    let mut tool_content: Vec<ai_providers::ContentBlock> =
        Vec::with_capacity(current_tool_uses.len());
    for u in current_tool_uses.iter() {
        if let ai_providers::ContentBlock::ToolUse { id, name, .. } = u {
            let result = results_by_id
                .remove(id)
                .unwrap_or_else(|| synthetic_missing_tool_result(id, Some(name.clone())));
            tool_content.push(result);
        }
    }
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
        content: tool_content,
    });
    // Deliberately NO `results_by_id.clear()` here. The capture guard in
    // `group_assistant_blocks` only ever inserts a result whose tool_use is
    // OUTSTANDING in this batch, so the map's keys are a subset of
    // `current_tool_uses`' ids — every one of which the loop above just drained via
    // `remove`. The map is provably empty at this point; a clear() would be dead code
    // implying a hazard the capture guard already closes at the source.
    debug_assert!(
        results_by_id.is_empty(),
        "flush must consume every captured result; leftovers mean the capture guard \
         admitted a result for a tool_use outside this batch"
    );
}

/// Group ONE assistant message's already-converted blocks into provider-ready
/// ChatMessages, reconstructing per-iteration boundaries. Each time every
/// outstanding tool_use has received its tool_result, one
/// `[Assistant { text + tool_use }, Tool { tool_result }]` pair is flushed.
///
/// The wire-format invariant is upheld for every batch whose tools have RUN:
/// once a batch has produced ≥1 tool_result (the normal case — the MCP layer
/// persists a real OR `is_error` tool_result for every executed tool, so a
/// failed/parallel batch always has results), every tool_use in the emitted
/// Assistant turn is answered by a tool_result (real, or a synthesized `is_error`
/// placeholder) in the immediately following Tool turn, and orphan tool_results
/// (no matching tool_use) are dropped. Anthropic/OpenAI/Gemini all reject an
/// unpaired tool_use, so this is the single point that keeps the assembled
/// request valid regardless of tool OUTCOME.
///
/// The ONE case deliberately left as a bare Assistant turn is a trailing batch
/// with NO tool_result yet — the in-progress / awaiting-approval state, whose
/// real result is appended separately (the approval-resume path emits it as the
/// following User message). Synthesizing there would race that result, so it is
/// emitted alone; the pairing is completed by the appended results downstream.
///
/// Pure + registry-free so the invariant is directly unit-testable. Assumes
/// blocks arrive in `sequence_order` (guaranteed by the repository's atomic MAX+1
/// assignment).
pub fn group_assistant_blocks(blocks: Vec<ai_providers::ContentBlock>) -> Vec<ChatMessage> {
    let mut messages = Vec::new();
    let mut current_text: Vec<ai_providers::ContentBlock> = Vec::new();
    let mut current_tool_uses: Vec<ai_providers::ContentBlock> = Vec::new();
    let mut pending_ids: std::collections::HashSet<String> = Default::default();
    // Results captured since the last flush, keyed by tool_use_id. Keyed (not a
    // flat vec) so the flush can pair strictly by id and drop orphans.
    let mut results_by_id: std::collections::HashMap<String, ai_providers::ContentBlock> =
        Default::default();

    for b in blocks {
        match &b {
            ai_providers::ContentBlock::ToolUse { id, .. } => {
                pending_ids.insert(id.clone());
                current_tool_uses.push(b);
            }
            ai_providers::ContentBlock::ToolResult { tool_use_id, .. } => {
                let id = tool_use_id.clone();
                // Capture a result ONLY if it answers a tool_use still outstanding in
                // THIS batch. `remove` returns whether it was pending, so it is both
                // the resolve and the test.
                //   pending      → the first result for that use: capture it.
                //   not pending, already captured → a DUPLICATE result: drop (keep-first).
                //   not pending, not captured     → an ORPHAN (no tool_use precedes it):
                //     drop NOW. Letting it sit in the map would shadow a LATER real
                //     result for the same id via keep-first, and emit the stale one in
                //     its place.
                if pending_ids.remove(&id) {
                    results_by_id.insert(id, b);
                }
                // All outstanding tool_uses resolved — flush one Assistant/Tool pair.
                if pending_ids.is_empty() && !current_tool_uses.is_empty() {
                    flush_assistant_tool_pair(
                        &mut messages,
                        &mut current_text,
                        &mut current_tool_uses,
                        &mut results_by_id,
                    );
                }
            }
            _ => current_text.push(b),
        }
    }

    // Trailing blocks that never completed a full round-trip above.
    if !current_tool_uses.is_empty() {
        // Distinguish two trailing cases that look alike but must be handled
        // differently:
        //   * COMPLETED-BUT-PARTIAL batch — ≥1 tool_use in this batch already has a
        //     captured result (the tools ran; some result is missing/failed). Emit
        //     the Assistant turn AND a Tool turn answering EVERY id: real results
        //     where captured, a synthesized is_error placeholder for the gaps. This
        //     is the failed/parallel repro the fix targets.
        //   * IN-PROGRESS / AWAITING-APPROVAL batch — NO result captured for any
        //     tool_use yet. The real result is appended separately (the
        //     approval-resume path emits it as a following User message), so
        //     synthesizing here would race a result that is still coming. Emit the
        //     Assistant tool_use turn ALONE, preserving the long-standing behavior.
        let batch_has_result = current_tool_uses.iter().any(|u| match u {
            ai_providers::ContentBlock::ToolUse { id, .. } => results_by_id.contains_key(id),
            _ => false,
        });
        if batch_has_result {
            flush_assistant_tool_pair(
                &mut messages,
                &mut current_text,
                &mut current_tool_uses,
                &mut results_by_id,
            );
        } else {
            let assistant_content: Vec<_> = current_text
                .drain(..)
                .chain(current_tool_uses.drain(..))
                .collect();
            messages.push(ChatMessage {
                role: ai_providers::Role::Assistant,
                content: assistant_content,
            });
        }
    } else if !current_text.is_empty() {
        // Pure-text final answer (no trailing tool_use). `results_by_id` is empty
        // here — the capture guard above refuses any result answering no OUTSTANDING
        // tool_use, so orphans are dropped on arrival rather than accumulating for
        // this branch to discard.
        messages.push(ChatMessage {
            role: ai_providers::Role::Assistant,
            content: std::mem::take(&mut current_text),
        });
    }

    messages
}

/// Enforce the provider invariant that every `tool_use_id` is answered by EXACTLY
/// ONE `tool_result` **within its tool batch** (see SCOPE below — batch-wide, NOT
/// request-wide, and deliberately so). Anthropic rejects a second one outright
/// ("each tool_use must have a single result. Found multiple `tool_result` blocks
/// with id: …"), killing the turn.
///
/// `group_assistant_blocks` already guarantees this WITHIN one stored assistant
/// message, but it cannot see blocks appended AFTER it returns — a `before_llm_call`
/// extension may push more `tool_result`s onto the request (the approval-resume
/// path does). This is the last checkpoint before the wire, so it is the only
/// place the invariant can be enforced across ALL contributors.
///
/// Keep-FIRST, matching the rule `group_assistant_blocks` applies internally: the
/// first occurrence is the one sitting in the Tool turn immediately after its
/// Assistant turn, so keeping it also preserves Anthropic's "result immediately
/// after the tool_use" rule — keep-last could strand the survivor in a trailing
/// message. A message emptied by the drop is removed (an empty `content` array is
/// itself invalid).
///
/// SCOPE — per TURN GROUP, never global. A `tool_use_id` is only unique within ONE
/// assistant message: `resolve_unique_tool_use_id` seeds its used-set from
/// `WHERE message_id = $1`, and its own doc names the case it tolerates —
/// gpt-oss/harmony streams the non-unique constant `"tool_use"` for every call. So
/// the same id legitimately recurs in a LATER turn answering a DIFFERENT tool_use,
/// and OpenAI-compatible providers pair `tool_call_id` per adjacent turn, not
/// globally. Deduping across the whole request would drop that later turn's real
/// result and leave its tool_use unpaired — reintroducing the sibling regression
/// this must not cause. The seen-set therefore resets at each Assistant message
/// that opens a new tool batch.
///
/// This is a DEFENSE, not the fix: each duplicate SOURCE is fixed at its origin
/// (see `replace_or_collect_tool_results` in the mcp chat extension). Returns the
/// ids it dropped — empty on every healthy request — so the CALLER can log them
/// with its own conversation/message context; this fn stays pure and quiet.
/// A non-empty return is the tripwire that a new duplicate source appeared.
pub fn dedup_tool_results_by_id(messages: &mut Vec<ChatMessage>) -> Vec<String> {
    let mut dropped: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = Default::default();
    // Only messages this fn EMPTIES are removed — a message that arrived empty is
    // left exactly as it was found (not this fn's business to police).
    let mut emptied = vec![false; messages.len()];

    for (i, m) in messages.iter_mut().enumerate() {
        // A new Assistant tool batch opens a new id scope (see SCOPE above).
        if matches!(m.role, ai_providers::Role::Assistant)
            && m.content
                .iter()
                .any(|b| matches!(b, ai_providers::ContentBlock::ToolUse { .. }))
        {
            seen.clear();
        }

        let had_content = !m.content.is_empty();
        m.content.retain(|b| match b {
            ai_providers::ContentBlock::ToolResult { tool_use_id, .. } => {
                if seen.insert(tool_use_id.clone()) {
                    return true;
                }
                dropped.push(tool_use_id.clone());
                false
            }
            _ => true,
        });
        emptied[i] = had_content && m.content.is_empty();
    }

    let mut i = 0;
    messages.retain(|_| {
        let keep = !emptied[i];
        i += 1;
        keep
    });

    dropped
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
mod tests {
    use super::*;

    use ai_providers::ContentBlock;

    use serde_json::json;


    fn text(s: &str) -> ContentBlock {
        ContentBlock::Text {
            text: s.to_string(),
        }
    }

    fn acc(content_type: &str, text: &str, sig: Option<&str>, redacted: Option<&str>) -> AccumulatedContent {
        AccumulatedContent {
            content_type: content_type.to_string(),
            accumulated_text: text.to_string(),
            signature: sig.map(str::to_string),
            redacted_data: redacted.map(str::to_string),
        }
    }

    #[test]
    fn is_visible_answer_classifies_blocks() {
        // Reasoning-only / empty text do NOT count as a user-visible answer.
        assert!(!is_visible_answer("thinking", "some reasoning"));
        assert!(!is_visible_answer("text", ""));
        assert!(!is_visible_answer("text", "   \n\t "));

        // Real answers DO count.
        assert!(is_visible_answer("text", "hi"));
        assert!(is_visible_answer("text", "  padded  "));
        assert!(is_visible_answer("tool_use", ""));
        assert!(is_visible_answer("tool_result", ""));
        assert!(is_visible_answer("image", ""));
        assert!(is_visible_answer("file_attachment", ""));
        assert!(is_visible_answer("elicitation_request", ""));
    }

    #[test]
    fn accumulated_to_content_block_reconstructs_provider_shapes() {
        // Text → ContentBlock::Text.
        assert!(matches!(
            accumulated_to_content_block(&acc("text", "hi", None, None)),
            Some(ContentBlock::Text { text }) if text == "hi"
        ));

        // Thinking with a signature → ContentBlock::Thinking carrying it.
        assert!(matches!(
            accumulated_to_content_block(&acc("thinking", "reasoning", Some("sig"), None)),
            Some(ContentBlock::Thinking { thinking, signature })
                if thinking == "reasoning" && signature.as_deref() == Some("sig")
        ));

        // Redacted-thinking data wins → ContentBlock::RedactedThinking.
        assert!(matches!(
            accumulated_to_content_block(&acc("thinking", "", None, Some("opaque"))),
            Some(ContentBlock::RedactedThinking { data }) if data == "opaque"
        ));

        // Extension-owned types aren't reconstructable here (they persist via
        // get_accumulated_content).
        assert!(accumulated_to_content_block(&acc("tool_use", "", None, None)).is_none());
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


    fn thinking(text: &str) -> ContentBlock {
        ContentBlock::Thinking {
            thinking: text.to_string(),
            signature: None,
        }
    }

    fn error_tool_result(id: &str, content: &str) -> ContentBlock {
        ContentBlock::ToolResult {
            tool_use_id: id.to_string(),
            name: None,
            content: vec![ContentBlock::Text {
                text: content.to_string(),
            }],
            is_error: Some(true),
        }
    }

    fn result_ids(content: &[ContentBlock]) -> Vec<String> {
        content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::ToolResult { tool_use_id, .. } => Some(tool_use_id.clone()),
                _ => None,
            })
            .collect()
    }

    fn tool_use_ids(content: &[ContentBlock]) -> Vec<String> {
        content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::ToolUse { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect()
    }

    /// TEST-1: a co-located assistant message with THREE parallel tool_use blocks but
    /// only ONE matching tool_result (the failed-parallel repro) must still produce a
    /// valid `[Assistant{thinking,text,use×3}, Tool{result×3}]` — the real result plus
    /// a synthesized `is_error` result for each unanswered id, in tool_use order, each
    /// carrying the tool_use's name. Thinking/text stay on the assistant side.
    #[test]
    fn group_assistant_blocks_pairs_partial_parallel_batch() {
        let blocks = vec![
            thinking("planning the analyses"),
            text("Running pathway + consensus."),
            tool_use("A", "srv__run_pathway_analysis"),
            tool_use("B", "srv__run_consensus_analysis"),
            tool_use("C", "srv__run_consensus_analysis"),
            tool_result("A", "ok-A"),
        ];
        let msgs = group_assistant_blocks(blocks);

        assert_eq!(msgs.len(), 2, "must be exactly one Assistant/Tool pair");

        // Assistant turn: thinking + text + all three tool_use, in order.
        assert!(matches!(msgs[0].role, ai_providers::Role::Assistant));
        assert_eq!(msgs[0].content.len(), 5);
        assert!(matches!(msgs[0].content[0], ContentBlock::Thinking { .. }));
        assert!(matches!(msgs[0].content[1], ContentBlock::Text { .. }));
        assert_eq!(tool_use_ids(&msgs[0].content), vec!["A", "B", "C"]);

        // Tool turn: exactly one result per tool_use, in tool_use order.
        assert!(matches!(msgs[1].role, ai_providers::Role::Tool));
        assert_eq!(result_ids(&msgs[1].content), vec!["A", "B", "C"]);

        // A is the REAL result (is_error None); B and C are synthesized is_error
        // placeholders carrying the tool_use name (Gemini pairs by name).
        match &msgs[1].content[0] {
            ContentBlock::ToolResult { is_error, .. } => assert_eq!(*is_error, None),
            _ => panic!("expected tool_result"),
        }
        for (i, want_name) in [(1usize, "srv__run_consensus_analysis"), (2, "srv__run_consensus_analysis")] {
            match &msgs[1].content[i] {
                ContentBlock::ToolResult { is_error, name, .. } => {
                    assert_eq!(*is_error, Some(true), "synthesized result must be is_error");
                    assert_eq!(name.as_deref(), Some(want_name), "synthesized result carries the tool_use name");
                }
                _ => panic!("expected tool_result"),
            }
        }
    }

    /// TEST-2: a fully-matched parallel batch still groups to exactly ONE pair with
    /// the real results only — no synthesized results, no regression.
    #[test]
    fn group_assistant_blocks_matched_parallel_batch_unchanged() {
        let blocks = vec![
            tool_use("A", "srv__a"),
            tool_use("B", "srv__b"),
            tool_result("A", "ra"),
            tool_result("B", "rb"),
        ];
        let msgs = group_assistant_blocks(blocks);

        assert_eq!(msgs.len(), 2);
        assert!(matches!(msgs[0].role, ai_providers::Role::Assistant));
        assert_eq!(tool_use_ids(&msgs[0].content), vec!["A", "B"]);
        assert!(matches!(msgs[1].role, ai_providers::Role::Tool));
        assert_eq!(result_ids(&msgs[1].content), vec!["A", "B"]);
        for b in &msgs[1].content {
            if let ContentBlock::ToolResult { is_error, .. } = b {
                assert_eq!(*is_error, None, "no synthesized results when all matched");
            }
        }
    }

    /// TEST-3: a real FAILED tool_result (`is_error: Some(true)`) is preserved verbatim
    /// and paired to its tool_use — a failed tool is never left unpaired.
    #[test]
    fn group_assistant_blocks_preserves_failed_tool_result() {
        let blocks = vec![tool_use("A", "srv__a"), error_tool_result("A", "boom")];
        let msgs = group_assistant_blocks(blocks);

        assert_eq!(msgs.len(), 2);
        assert!(matches!(msgs[1].role, ai_providers::Role::Tool));
        assert_eq!(msgs[1].content.len(), 1);
        match &msgs[1].content[0] {
            ContentBlock::ToolResult { tool_use_id, is_error, content, .. } => {
                assert_eq!(tool_use_id, "A");
                assert_eq!(*is_error, Some(true));
                assert!(matches!(content[0], ContentBlock::Text { .. }));
            }
            _ => panic!("expected tool_result"),
        }
    }

    /// TEST-4: an orphan tool_result (no matching tool_use) is dropped — no orphan Tool
    /// turn, no dangling Assistant tool_use.
    #[test]
    fn group_assistant_blocks_drops_orphan_tool_result() {
        // Lone orphan result → nothing emitted.
        let msgs = group_assistant_blocks(vec![tool_result("X", "orphan")]);
        assert!(msgs.is_empty(), "a lone orphan tool_result emits nothing");

        // A valid pair followed by an orphan result: the orphan is dropped, the pair
        // stays valid. Exactly one tool_result (the real one) is emitted overall.
        let msgs = group_assistant_blocks(vec![
            tool_use("A", "srv__a"),
            tool_result("A", "ra"),
            tool_result("X", "orphan"),
        ]);
        assert_eq!(msgs.len(), 2);
        let total_results: usize = msgs
            .iter()
            .map(|m| result_ids(&m.content).len())
            .sum();
        assert_eq!(total_results, 1, "orphan result must be dropped");
        assert_eq!(result_ids(&msgs[1].content), vec!["A"]);
    }

    /// TEST-7: an orphan tool_result arriving BEFORE the batch completes (interleaved
    /// mid-stream) is dropped from the flushed Tool turn. Pre-fix it rode along inside
    /// the Tool message (`mem::take` of a flat `current_results`), producing an invalid
    /// tool_result with no matching tool_use. This is the per-flush orphan-drop that the
    /// trailing-only orphan test does NOT exercise.
    #[test]
    fn group_assistant_blocks_drops_mid_stream_orphan_result() {
        let blocks = vec![
            tool_use("A", "srv__a"),
            tool_result("X", "orphan-before-flush"), // no tool_use X in this batch
            tool_result("A", "ra"),
        ];
        let msgs = group_assistant_blocks(blocks);

        assert_eq!(msgs.len(), 2);
        assert!(matches!(msgs[1].role, ai_providers::Role::Tool));
        assert_eq!(
            result_ids(&msgs[1].content),
            vec!["A"],
            "mid-stream orphan X must be dropped from the flushed Tool turn"
        );
    }

    /// TEST-8: a duplicate tool_result for the same id keeps the FIRST and drops the
    /// duplicate — exactly one result per tool_use reaches the provider. A co-pending
    /// second tool_use (B) DEFERS the flush so the duplicate A result arrives while A is
    /// still in `results_by_id`, exercising the keep-first `or_insert` branch (pre-fix,
    /// the flat result vec carried BOTH A results into the Tool turn → three results).
    #[test]
    fn group_assistant_blocks_dedups_duplicate_result() {
        let blocks = vec![
            tool_use("A", "srv__a"),
            tool_use("B", "srv__b"),
            tool_result("A", "first"),
            tool_result("A", "dup"), // duplicate arrives BEFORE the flush (B still pending)
            tool_result("B", "rb"),
        ];
        let msgs = group_assistant_blocks(blocks);

        assert_eq!(msgs.len(), 2);
        assert!(matches!(msgs[1].role, ai_providers::Role::Tool));
        assert_eq!(
            result_ids(&msgs[1].content),
            vec!["A", "B"],
            "exactly one result per tool_use — the duplicate A is dropped"
        );
        match &msgs[1].content[0] {
            ContentBlock::ToolResult { content, .. } => assert!(
                matches!(&content[0], ContentBlock::Text { text } if text == "first"),
                "the FIRST A result is kept, the duplicate dropped"
            ),
            _ => panic!("expected tool_result"),
        }
    }

    /// TEST-5 (fix-duplicate-tool-result): a stale ORPHAN result cannot shadow a later
    /// REAL result for the same id — here with a flush (the A batch) in between.
    /// Pre-fix, the keep-first `or_insert` captured the orphan and emitted "stale" as
    /// X's result. Now the capture guard refuses a result that answers no OUTSTANDING
    /// tool_use, so the orphan never enters the map at all. (TEST-16 covers the harder
    /// half: the same hazard with NO intervening flush.)
    #[test]
    fn group_assistant_blocks_later_real_result_beats_stale_orphan() {
        let blocks = vec![
            tool_result("X", "stale"), // orphan: no tool_use X yet
            tool_use("A", "srv__a"),
            tool_result("A", "ra"), // flushes the A batch; the X orphan must not survive it
            tool_use("X", "srv__x"),
            tool_result("X", "real"),
        ];
        let msgs = group_assistant_blocks(blocks);

        // [Assistant{A}, Tool{ra}, Assistant{X}, Tool{real}]
        assert_eq!(msgs.len(), 4);
        assert_eq!(result_ids(&msgs[3].content), vec!["X"]);
        match &msgs[3].content[0] {
            ContentBlock::ToolResult { content, .. } => assert!(
                matches!(&content[0], ContentBlock::Text { text } if text == "real"),
                "X must carry its REAL result, not the stale pre-flush orphan"
            ),
            _ => panic!("expected tool_result"),
        }
    }

    /// TEST-16: the orphan hazard with NO intervening flush. `[result X(stale),
    /// use X, result X(real)]` — nothing flushes before X's real result arrives, so
    /// `results_by_id.clear()` never runs and cannot help. Only refusing to capture a
    /// result that answers no OUTSTANDING tool_use closes this half.
    #[test]
    fn group_assistant_blocks_orphan_before_its_use_does_not_shadow_the_real_result() {
        let blocks = vec![
            tool_result("X", "stale"), // orphan: no tool_use X outstanding
            tool_use("X", "srv__x"),
            tool_result("X", "real"),
        ];
        let msgs = group_assistant_blocks(blocks);

        assert_eq!(msgs.len(), 2);
        assert_eq!(result_ids(&msgs[1].content), vec!["X"]);
        match &msgs[1].content[0] {
            ContentBlock::ToolResult { content, .. } => assert!(
                matches!(&content[0], ContentBlock::Text { text } if text == "real"),
                "the orphan that PRECEDED the tool_use must not shadow the real result"
            ),
            _ => panic!("expected tool_result"),
        }
    }

    /// TEST-18 — a CHARACTERIZATION test: it pins PRE-EXISTING behavior (it passes on
    /// base too, by design) because that behavior is the load-bearing premise for a
    /// decision elsewhere in this diff, and a future edit that "helpfully" synthesized
    /// a result here would silently invalidate it.
    ///
    /// For a batch where NOTHING ran, the trailing branch emits a BARE Assistant turn
    /// with no Tool message — correct for awaiting-approval (its result is still
    /// coming), fatal for a tool that will never produce one: the tool_use is then
    /// unpaired on EVERY subsequent request and the provider rejects all of them (the
    /// branch is bricked). THAT is why `execute_approved_tools_sync`'s AlreadyClaimed
    /// and Failed paths must emit an is_error result rather than skip silently or bail
    /// — a blind-audit round found exactly that regression in this branch's own
    /// earlier attempt.
    #[test]
    fn group_assistant_blocks_resultless_batch_emits_a_bare_unpaired_assistant_turn() {
        let msgs = group_assistant_blocks(vec![tool_use("A", "srv__a")]);

        assert_eq!(msgs.len(), 1, "no Tool turn is synthesized for a resultless batch");
        assert!(matches!(msgs[0].role, ai_providers::Role::Assistant));
        assert_eq!(tool_use_ids(&msgs[0].content), vec!["A"]);
        assert!(
            result_ids(&msgs[0].content).is_empty(),
            "the tool_use is UNPAIRED — which is only safe when a result is still \
             coming (awaiting approval). Any path that abandons a tool_use without a \
             result lands here permanently."
        );
    }

    /// TEST-2 (fix-duplicate-tool-result): the chokepoint defense keeps the FIRST
    /// tool_result for a repeated id and drops later ones, so exactly one result per
    /// tool_use reaches the provider. Non-duplicate blocks keep their relative order.
    #[test]
    fn dedup_tool_results_keeps_first_and_drops_later_duplicates() {
        let mut msgs = vec![
            ChatMessage {
                role: ai_providers::Role::Assistant,
                content: vec![tool_use("A", "srv__a"), tool_use("B", "srv__b")],
            },
            ChatMessage {
                role: ai_providers::Role::Tool,
                content: vec![tool_result("A", "ra"), tool_result("B", "placeholder")],
            },
            // The duplicate: a later message re-answers B.
            ChatMessage {
                role: ai_providers::Role::User,
                content: vec![tool_result("B", "real"), text("trailing note")],
            },
        ];
        let dropped = dedup_tool_results_by_id(&mut msgs);

        assert_eq!(dropped, vec!["B"], "the dropped id is reported to the caller");
        assert_eq!(msgs.len(), 3, "no message became empty here");
        assert_eq!(result_ids(&msgs[1].content), vec!["A", "B"]);
        assert!(
            result_ids(&msgs[2].content).is_empty(),
            "the later duplicate B result is dropped"
        );
        // Keep-first: the surviving B is the one adjacent to its tool_use.
        match &msgs[1].content[1] {
            ContentBlock::ToolResult { content, .. } => assert!(
                matches!(&content[0], ContentBlock::Text { text } if text == "placeholder"),
                "keep-FIRST: the block paired with the tool_use survives"
            ),
            _ => panic!("expected tool_result"),
        }
        // A non-tool_result block in the same message is untouched.
        assert!(matches!(&msgs[2].content[0], ContentBlock::Text { text } if text == "trailing note"));
    }

    /// TEST-3 (fix-duplicate-tool-result): the defense never perturbs a healthy
    /// request — an already-valid one is byte-identical after the pass.
    #[test]
    fn dedup_tool_results_is_a_noop_on_a_valid_request() {
        let build = || {
            vec![
                ChatMessage {
                    role: ai_providers::Role::Assistant,
                    content: vec![text("thinking out loud"), tool_use("A", "srv__a")],
                },
                ChatMessage {
                    role: ai_providers::Role::Tool,
                    content: vec![tool_result("A", "ra")],
                },
                ChatMessage {
                    role: ai_providers::Role::User,
                    content: vec![text("next question")],
                },
            ]
        };
        let before = build();
        let mut after = build();
        let dropped = dedup_tool_results_by_id(&mut after);

        assert!(dropped.is_empty(), "a valid request drops nothing");

        // Deep equality — role/len alone would miss a swapped or mutated block.
        assert_eq!(
            serde_json::to_value(&after).unwrap(),
            serde_json::to_value(&before).unwrap(),
            "a valid request must come out byte-identical"
        );
    }

    /// TEST-4 (fix-duplicate-tool-result): a message whose ONLY block is dropped as a
    /// duplicate is removed entirely — an empty `content` array is itself rejected by
    /// the provider, so dedup must not trade one invalid request for another.
    #[test]
    fn dedup_tool_results_removes_a_message_it_empties() {
        let mut msgs = vec![
            ChatMessage {
                role: ai_providers::Role::Assistant,
                content: vec![tool_use("A", "srv__a")],
            },
            ChatMessage {
                role: ai_providers::Role::Tool,
                content: vec![tool_result("A", "ra")],
            },
            // Sole block is a duplicate → the whole message must go.
            ChatMessage {
                role: ai_providers::Role::User,
                content: vec![tool_result("A", "dup")],
            },
        ];
        let dropped = dedup_tool_results_by_id(&mut msgs);

        assert_eq!(dropped, vec!["A"]);
        assert_eq!(msgs.len(), 2, "the emptied User message is removed");
        assert!(msgs.iter().all(|m| !m.content.is_empty()));
        assert!(matches!(msgs[1].role, ai_providers::Role::Tool));
    }

    /// A tool_use_id is only unique WITHIN one assistant message —
    /// `resolve_unique_tool_use_id` seeds its used-set per `message_id`, and its own
    /// doc names the case: gpt-oss/harmony streams the non-unique constant
    /// `"tool_use"` for every call. The SAME id therefore recurs legitimately in a
    /// later turn, answering a DIFFERENT tool_use. Deduping globally would drop the
    /// later turn's real result and leave its tool_use unpaired — the very rejection
    /// this defense exists to prevent. Dedup is scoped to one turn group.
    #[test]
    fn dedup_tool_results_allows_the_same_id_reused_across_turns() {
        let mut msgs = vec![
            // Turn 1 — gpt-oss's constant id.
            ChatMessage {
                role: ai_providers::Role::Assistant,
                content: vec![tool_use("tool_use", "srv__search")],
            },
            ChatMessage {
                role: ai_providers::Role::Tool,
                content: vec![tool_result("tool_use", "turn-1 result")],
            },
            // Turn 2 — same id, a genuinely different call.
            ChatMessage {
                role: ai_providers::Role::Assistant,
                content: vec![tool_use("tool_use", "srv__read_file")],
            },
            ChatMessage {
                role: ai_providers::Role::Tool,
                content: vec![tool_result("tool_use", "turn-2 result")],
            },
        ];
        let dropped = dedup_tool_results_by_id(&mut msgs);

        assert!(
            dropped.is_empty(),
            "a reused id in a DIFFERENT turn is not a duplicate — nothing may be dropped"
        );
        assert_eq!(msgs.len(), 4, "no turn may be dropped");
        assert_eq!(
            result_ids(&msgs[1].content),
            vec!["tool_use"],
            "turn 1 keeps its result"
        );
        assert_eq!(
            result_ids(&msgs[3].content),
            vec!["tool_use"],
            "turn 2's tool_use must KEEP its own result — a different call that happens \
             to reuse the id"
        );
        match &msgs[3].content[0] {
            ContentBlock::ToolResult { content, .. } => assert!(
                matches!(&content[0], ContentBlock::Text { text } if text == "turn-2 result"),
                "turn 2 must carry ITS result, not turn 1's"
            ),
            _ => panic!("expected tool_result"),
        }
    }

    /// A message that arrived EMPTY is not this fn's business — it is left alone
    /// (only messages dedup itself empties are removed).
    #[test]
    fn dedup_tool_results_leaves_a_pre_existing_empty_message_alone() {
        let mut msgs = vec![ChatMessage {
            role: ai_providers::Role::User,
            content: vec![],
        }];
        let dropped = dedup_tool_results_by_id(&mut msgs);
        assert!(dropped.is_empty());
        assert_eq!(msgs.len(), 1);
    }

    // TEST-15: thinking resolves via row → catalog → family policy.
    #[test]
    fn thinking_config_for_resolves_row_catalog_family() {
        use crate::modules::llm_model::models::ModelCapabilities;
        use ai_providers::ThinkingMode;
        let none = ModelCapabilities::default();
        // Catalog: adaptive thinking model.
        let cfg = thinking_config_for("anthropic", "claude-opus-4-7", &none).expect("thinking enabled");
        assert_eq!(cfg.mode, ThinkingMode::Adaptive);
        assert!(cfg.effort.is_some());
        // Catalog: a non-thinking model (haiku is catalogued but not thinking-capable).
        assert!(thinking_config_for("anthropic", "claude-haiku-4-5", &none).is_none());
        assert!(thinking_config_for("openai", "gpt-4o", &none).is_none());
        // Family pattern: an uncatalogued o-series model still enables thinking.
        assert!(thinking_config_for("openai", "o5-mini", &none).is_some());
        // Row override enables thinking on an otherwise-unknown model.
        let row = ModelCapabilities {
            supports_thinking: Some(true),
            ..Default::default()
        };
        assert!(thinking_config_for("openai", "totally-unknown", &row).is_some());
        // Row override disables thinking even for a thinking-capable family.
        let off = ModelCapabilities {
            supports_thinking: Some(false),
            ..Default::default()
        };
        assert!(thinking_config_for("anthropic", "claude-opus-4-7", &off).is_none());
    }


    // TEST-15: temperature is forwarded only when set (no forced 0.7); max_tokens
    // keeps its required-field default.
    #[test]
    fn apply_model_params_maps_and_omits_temperature() {
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

        // Empty params: temperature is OMITTED (never force-injected); max_tokens
        // still gets its required-field default.
        let mut req2 = ai_providers::ChatRequest::default();
        apply_model_params(&mut req2, &ModelParameters::default());
        assert_eq!(req2.temperature, None);
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


    /// Recall-roundtrip linkage: a CLEARED older result's placeholder must carry
    /// the EXACT `tool_use_id` inside a `get_tool_result(...)` hint, so the model
    /// can recover the full result via the tool_result_mcp recall path (whose own
    /// retrieval is covered by tests/tool_result_mcp). Without the right id in the
    /// placeholder the roundtrip is impossible.
    #[test]
    fn cleared_placeholder_carries_tool_use_id_for_recall() {
        let big = "x".repeat(400);
        let mut msgs: Vec<ChatMessage> = (0..5)
            .map(|i| tool_result_msg(&format!("tu{i}"), &big))
            .collect();
        clear_old_tool_results(&mut msgs, 100, 1);

        // The oldest result (tu0) is cleared and its placeholder names tu0 in a
        // get_tool_result hint.
        let txt = match &msgs[0].content[0] {
            ContentBlock::ToolResult { content, .. } => match &content[0] {
                ContentBlock::Text { text } => text.clone(),
                _ => String::new(),
            },
            _ => String::new(),
        };
        assert!(txt.contains("get_tool_result"), "placeholder must point at get_tool_result: {txt}");
        assert!(
            txt.contains("tool_use_id=\"tu0\""),
            "placeholder must carry the cleared result's exact tool_use_id for recall: {txt}"
        );
    }
}
