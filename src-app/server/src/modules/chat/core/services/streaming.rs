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
    extension::{ExtensionRegistry, SendMessageRequest, StreamContext},
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
            create_provider_from_model_id(&self.pool, request.model_id).await?;

        // Conditionally create user message (check extensions)
        // Extensions can prevent user message creation (e.g., MCP tool approval resumption)
        let user_message_id = if self.extension_registry
            .as_ref()
            .map(|reg| reg.should_create_user_message(&request))
            .unwrap_or(true)  // Default to true if no registry
        {
            let user_message =
                Repos.chat.core.create_message(branch_id, MessageRole::User.as_str()).await?;

            let user_content_data = MessageContentData::Text {
                text: request.content.clone(),
            };
            Repos.chat.core.create_content(user_message.id, "text", user_content_data, 0)
                .await?;

            Some(user_message.id)
        } else {
            None  // Extension prevented user message creation
        };

        // Get or create assistant message (BEFORE loop)
        // Extensions can provide existing message for continuation (e.g., MCP tool approval)
        let assistant_message_id = if let Some(reg) = &self.extension_registry {
            if let Some(msg_id) = reg.provide_assistant_message(&request, branch_id).await? {
                msg_id
            } else {
                // No extension provided message, create new one
                let msg = Repos.chat.core.create_message(branch_id, MessageRole::Assistant.as_str()).await?;
                msg.id
            }
        } else {
            // No extension registry, create new message
            let msg = Repos.chat.core.create_message(branch_id, MessageRole::Assistant.as_str()).await?;
            msg.id
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
            const MAX_ITERATIONS: u32 = 5;
            let mut iteration = 1u32;

            loop {
                // Guard against infinite loops
                if iteration > MAX_ITERATIONS {
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

                // Get conversation history
                let mut history = match Repos.chat.core.get_conversation_history( branch_id).await {
                    Ok(h) => h,
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        break;
                    }
                };

                // Filter out the just-created assistant message (it has no content yet)
                // This prevents sending empty messages to the AI provider
                history.retain(|msg_with_content| {
                    msg_with_content.message.id != assistant_message_id
                });

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
                    max_tokens: Some(4096),
                    ..Default::default()
                };

                // Call before_llm_call hooks
                if let Some(registry) = &extension_registry {
                    if let Err(e) = registry
                        .call_before_llm_call(&mut stream_context, &mut chat_request, &request, Some(&ext_tx))
                        .await
                    {
                        let _ = tx.send(Err(e));
                        break;
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

                // Always send Complete event after stream ends
                let (finish_reason, usage) = {
                    let acc = accumulator.lock().await;
                    (acc.finish_reason.clone(), acc.usage.clone())
                };

                // Use finish_reason from provider, or default to "stop" if not provided
                let final_finish_reason = finish_reason.unwrap_or_else(|| "stop".to_string());

                // Send a special ChatStreamChunk with finish_reason set (handler will convert to Complete event)
                let complete_chunk = ChatStreamChunk {
                    content: Vec::new(),
                    message_id: None,
                    conversation_id: Some(conversation_id),
                    branch_id: Some(branch_id),
                    finish_reason: Some(final_finish_reason),
                    usage,
                    error: None,
                };

                if tx.send(Ok(complete_chunk)).is_err() {
                    // Channel closed, stop streaming
                    return;
                }

                // Check extension action
                let action = {
                    let acc = accumulator.lock().await;
                    acc.extension_action.clone()
                };

                match action {
                    Some(crate::modules::chat::core::extension::ExtensionAction::Continue {
                        assistant_message_content,
                    }) => {
                        // Append tool results to existing assistant message
                        // Get current content count to calculate proper indices
                        let content_offset = match Repos.chat.core.get_message_with_content(assistant_message_id).await {
                            Ok(Some(msg_with_content)) => msg_with_content.contents.len() as i32,
                            Ok(None) => 0,  // No content yet (shouldn't happen)
                            Err(e) => {
                                let _ = tx.send(Err(e));
                                break;
                            }
                        };

                        // Tool results are added as content blocks to the same message
                        for (offset_index, content) in assistant_message_content.iter().enumerate() {
                            let content_type = content.content_type();
                            let actual_index = content_offset + offset_index as i32;
                            match Repos.chat.core.create_content(
                                assistant_message_id,
                                content_type,
                                content.clone(),
                                actual_index,
                            ).await {
                                Ok(_) => {
                                    tracing::info!("Appended content block {} to assistant message", actual_index);
                                }
                                Err(e) => {
                                    let _ = tx.send(Err(e));
                                    break;
                                }
                            }
                        }

                        iteration += 1;
                        // Continue loop with next LLM call
                    }
                    _ => {
                        // Complete or None - stop looping
                        break;
                    }
                }
            }
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

            // Convert content blocks
            let mut content_blocks = Vec::new();
            for content in &msg_with_content.contents {
                let content_data = content.parse_content()?;
                if let Some(block) = content_data.to_content_block() {
                    content_blocks.push(block);
                }
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

            // Convert role to AI provider role
            let ai_role = match role {
                MessageRole::User => ai_providers::Role::User,
                MessageRole::Assistant => ai_providers::Role::Assistant,
                MessageRole::System => continue, // Skip system messages for now
            };

            // Convert content blocks
            let mut content_blocks = Vec::new();
            for content in &msg_with_content.contents {
                let content_data = content.parse_content()?;

                // Handle Extension variants via registry
                let block = if let MessageContentData::Extension {
                    extension_name,
                    content: ext_content,
                } = &content_data
                {
                    // Extension content must be converted via registry
                    if let Some(registry) = extension_registry {
                        registry.convert_to_content_block(extension_name, ext_content)
                    } else {
                        None // No registry, skip extension content
                    }
                } else {
                    // Non-extension content: try extension transformation first
                    if let Some(registry) = extension_registry {
                        // Ask extension to transform content for LLM (e.g., file → text description)
                        match registry
                            .process_content_for_llm(&content_data, context)
                            .await?
                        {
                            Some(transformed_block) => Some(transformed_block),
                            None => content_data.to_content_block(), // Use default conversion
                        }
                    } else {
                        content_data.to_content_block() // Use default conversion
                    }
                };

                if let Some(b) = block {
                    content_blocks.push(b);
                }
            }

            messages.push(ChatMessage {
                role: ai_role,
                content: content_blocks,
            });
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

            // Convert content blocks
            let mut content_blocks = Vec::new();
            for content in &msg_with_content.contents {
                let content_data = content.parse_content()?;
                if let Some(block) = content_data.to_content_block() {
                    content_blocks.push(block);
                }
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

            // Serialize to JSON
            let content_json =
                serde_json::to_value(&content_data).map_err(|e| AppError::database_error(e))?;

            // Insert content block
            sqlx::query!(
                r#"
                INSERT INTO message_contents (message_id, content_type, content, sequence_order)
                VALUES ($1, $2, $3, $4)
                "#,
                self.assistant_message_id,
                accumulated.content_type,
                content_json,
                accumulated.index as i32
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
                let content_json = serde_json::to_value(&content_data)
                    .map_err(|e| AppError::database_error(e))?;

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
                    index as i32
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
