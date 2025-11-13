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
use crate::modules::chat::core::{
    extension::{ExtensionRegistry, SendMessageRequest, StreamContext},
    models::{MessageContentData, MessageRole},
    repository::{contents as content_repo, messages as msg_repo},
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
    pub async fn send_message(
        &self,
        branch_id: Uuid,
        conversation_id: Uuid,
        user_id: Uuid,
        request: SendMessageRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatStreamChunk, AppError>> + Send>>, AppError>
    {
        // Create provider from model_id
        use crate::modules::chat::core::ai_provider::create_provider_from_model_id;

        let (provider, model_name, model_id, provider_id) =
            create_provider_from_model_id(&self.pool, request.model_id).await?;

        // Create initial user message
        let user_message =
            msg_repo::create_message(&self.pool, branch_id, MessageRole::User.as_str()).await?;

        let user_content_data = MessageContentData::Text {
            text: request.content.clone(),
        };
        content_repo::create_content(&self.pool, user_message.id, "text", user_content_data, 0)
            .await?;

        // Create channel for streaming output
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        // Clone data for spawned task
        let pool = self.pool.clone();
        let provider_for_task = provider.clone();
        let extension_registry = self.extension_registry.clone();

        // Spawn task to handle loop
        tokio::spawn(async move {
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
                        title: None,
                        error: Some(crate::modules::chat::core::types::StreamError {
                            message: "Maximum tool calling iterations exceeded".to_string(),
                            code: Some("MAX_ITERATIONS_EXCEEDED".to_string()),
                        }),
                    };
                    let _ = tx.send(Ok(error_chunk));
                    break;
                }

                // Create assistant message for this iteration
                let assistant_message = match msg_repo::create_message(
                    &pool,
                    branch_id,
                    MessageRole::Assistant.as_str(),
                )
                .await
                {
                    Ok(msg) => msg,
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        break;
                    }
                };

                // Get conversation history
                let history = match msg_repo::get_conversation_history(&pool, branch_id).await {
                    Ok(h) => h,
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        break;
                    }
                };

                // Convert to AI provider format
                let messages = match Self::convert_history_to_messages_static(&history) {
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
                    message_id: Some(assistant_message.id),
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
                        .call_before_llm_call(&mut stream_context, &mut chat_request, &request)
                        .await
                    {
                        let _ = tx.send(Err(e));
                        break;
                    }
                }

                // Call AI provider
                let mut ai_stream = match provider_for_task.chat_stream(chat_request).await {
                    Ok(stream) => stream,
                    Err(e) => {
                        let _ = tx.send(Err(AppError::internal_error(format!(
                            "AI provider error: {}",
                            e
                        ))));
                        break;
                    }
                };

                // Create accumulator
                let accumulator = Arc::new(Mutex::new(DeltaAccumulator {
                    pool: pool.clone(),
                    assistant_message_id: assistant_message.id,
                    content_blocks: Vec::new(),
                    conversation_id,
                    branch_id,
                    extension_registry: extension_registry.clone(),
                    stream_context: stream_context.clone(),
                    extension_action: None,
                }));

                // Stream chunks through accumulator
                while let Some(chunk_result) = ai_stream.next().await {
                    match chunk_result {
                        Ok(ai_chunk) => {
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

                // Check extension action
                let action = {
                    let acc = accumulator.lock().await;
                    acc.extension_action.clone()
                };

                match action {
                    Some(crate::modules::chat::core::extension::ExtensionAction::Continue {
                        user_message_content,
                    }) => {
                        // Create continuation message with tool results
                        match Self::create_continuation_message_static(
                            &pool,
                            branch_id,
                            user_message_content,
                        )
                        .await
                        {
                            Ok(_continuation_msg_id) => {
                                iteration += 1;
                                // Continue loop
                            }
                            Err(e) => {
                                let _ = tx.send(Err(e));
                                break;
                            }
                        }
                    }
                    _ => {
                        // Complete or None - stop looping
                        break;
                    }
                }
            }
        });

        // Return channel receiver as stream
        Ok(Box::pin(UnboundedReceiverStream::new(rx)))
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

    /// Static version of convert_history_to_messages for use in spawned task
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

    /// Create continuation message with tool results
    /// Returns the ID of the created message
    async fn create_continuation_message_static(
        pool: &PgPool,
        branch_id: Uuid,
        user_message_content: Vec<MessageContentData>,
    ) -> Result<Uuid, AppError> {
        // Create user message for tool results
        let continuation_message =
            msg_repo::create_message(pool, branch_id, MessageRole::User.as_str()).await?;

        // Store content blocks (tool results, etc.)
        for (index, content_data) in user_message_content.iter().enumerate() {
            content_repo::create_content(
                pool,
                continuation_message.id,
                content_data.content_type(),
                content_data.clone(),
                index as i32,
            )
            .await?;
        }

        Ok(continuation_message.id)
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
}

impl DeltaAccumulator {
    /// Process an AI provider chunk and accumulate deltas in memory
    async fn process_chunk(
        &mut self,
        ai_chunk: AiStreamChunk,
    ) -> Result<ChatStreamChunk, AppError> {
        let mut output_chunk = ChatStreamChunk {
            content: Vec::new(),
            message_id: Some(self.assistant_message_id),
            conversation_id: Some(self.conversation_id),
            branch_id: Some(self.branch_id),
            finish_reason: ai_chunk.finish_reason.clone(),
            usage: ai_chunk
                .usage
                .as_ref()
                .map(|u| crate::modules::chat::core::types::Usage {
                    input_tokens: Some(u.prompt_tokens),
                    output_tokens: Some(u.completion_tokens),
                }),
            title: None,
            error: None,
        };

        // Process each content delta - accumulate in memory
        for ai_delta in &ai_chunk.content {
            if let Some(delta) = ContentBlockDelta::from_ai_providers_delta(ai_delta) {
                // Accumulate delta in memory (no DB write)
                self.accumulate_delta_in_memory(&delta);

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
    fn accumulate_delta_in_memory(&mut self, delta: &ContentBlockDelta) {
        match delta {
            ContentBlockDelta::TextDelta {
                index,
                delta,
                content_id: _,
            } => {
                self.ensure_content_block_exists(*index, "text");
                if let Some(block) = self.content_blocks.get_mut(*index) {
                    block.accumulated_text.push_str(delta);
                }
            }
            ContentBlockDelta::ThinkingDelta {
                index,
                delta,
                content_id: _,
            } => {
                self.ensure_content_block_exists(*index, "thinking");
                if let Some(block) = self.content_blocks.get_mut(*index) {
                    block.accumulated_text.push_str(delta);
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
                _ => continue, // Skip unknown types
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

        tx.commit().await.map_err(AppError::database_error)?;

        // Call extension hooks after database write completes
        if let Some(registry) = &self.extension_registry {
            // Fetch the complete message from database
            let final_message = msg_repo::get_message(&self.pool, self.assistant_message_id)
                .await?
                .ok_or_else(|| AppError::internal_error("Message not found after finalize"))?;

            // Call after_llm_call hooks and store the result
            // (no SSE channel since stream already closed)
            match registry
                .call_after_llm_call(&self.stream_context, &final_message, None)
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

        Ok(())
    }
}
