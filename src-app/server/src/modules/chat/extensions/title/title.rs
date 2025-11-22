use crate::core::Repos;
// Title generation extension implementation

use async_trait::async_trait;
use axum::response::sse::Event;
use futures_util::StreamExt;
use sqlx::PgPool;
use std::convert::Infallible;

use ai_providers::{ChatMessage, ChatRequest, ContentBlock, Provider, Role};

use crate::common::AppError;
use crate::modules::chat::core::{
    extension::{ChatExtension, ExtensionAction, StreamContext},
    models::Message,
    
    types::streaming::SSEChatStreamEvent,
};
use crate::modules::chat::extensions::title::extension::SSEChatStreamTitleUpdatedData;

/// Title generation extension
///
/// Generates conversation titles automatically after the first message exchange.
pub struct TitleGenerationExtension {}

impl TitleGenerationExtension {
    pub fn new(_pool: PgPool) -> Self {
        Self {}
    }

    /// Generate title using AI
    async fn generate_title_with_ai(
        &self,
        provider: &Provider,
        model_name: &str,
        user_content: &str,
    ) -> Result<String, AppError> {
        // Create title generation prompt
        let title_prompt = format!(
            "Generate a concise, descriptive title (maximum 6 words) for a conversation that starts with this message: \"{}\"\n\nRespond with only the title, no quotes or additional text.",
            user_content.chars().take(200).collect::<String>()
        );

        // Build chat request
        let request = ChatRequest {
            model: model_name.to_string(),
            messages: vec![ChatMessage {
                role: Role::User,
                content: vec![ContentBlock::Text { text: title_prompt }],
            }],
            temperature: Some(0.7),
            max_tokens: Some(50),
            ..Default::default()
        };

        // Call AI provider and collect the stream
        let mut stream = provider
            .chat_stream(request)
            .await
            .map_err(|e| AppError::internal_error(format!("AI provider error: {}", e)))?;

        // Collect all chunks into a single string
        let mut full_content = String::new();
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result
                .map_err(|e| AppError::internal_error(format!("Stream error: {}", e)))?;

            // Extract text from content deltas
            for delta in &chunk.content {
                match delta {
                    ai_providers::ContentBlockDelta::TextDelta { delta, .. } => {
                        full_content.push_str(delta);
                    }
                    _ => {} // Ignore non-text deltas for title generation
                }
            }
        }

        // Clean up the title
        let clean_title = full_content
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .chars()
            .take(50)
            .collect::<String>();

        if clean_title.is_empty() {
            return Err(AppError::internal_error("Generated title is empty"));
        }

        Ok(clean_title)
    }

    /// Generate simple title from user content (fallback)
    fn generate_simple_title(&self, user_content: &str) -> String {
        let title = user_content.chars().take(50).collect::<String>();

        if title.is_empty() {
            "New Conversation".to_string()
        } else if user_content.len() > 50 {
            format!("{}...", title)
        } else {
            title
        }
    }

    /// Send title updated event via SSE
    fn send_title_event(
        &self,
        title: &str,
        tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) {
        if let Some(tx) = tx {
            let event = SSEChatStreamEvent::TitleUpdated(SSEChatStreamTitleUpdatedData {
                title: title.to_string(),
            });

            if let Err(e) = tx.send(Ok(event.into())) {
                eprintln!("ERROR: Failed to send titleUpdated event: {:?}", e);
            }
        }
    }
}

#[async_trait]
impl ChatExtension for TitleGenerationExtension {
    fn name(&self) -> &str {
        "title-generation"
    }

    async fn initialize(&self, _pool: &PgPool) -> Result<(), AppError> {
        println!("Title generation extension initialized");
        Ok(())
    }

    async fn after_llm_call(
        &self,
        context: &StreamContext,
        _final_message: &Message,
        tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<ExtensionAction, AppError> {
        // Check if conversation needs a title
        let conversation =
            Repos.chat.core.get_conversation( context.conversation_id, context.user_id)
                .await?
                .ok_or_else(|| AppError::not_found("Conversation"))?;

        // Skip if conversation already has a title
        if conversation.title.is_some() && !conversation.title.as_ref().unwrap().is_empty() {
            return Ok(ExtensionAction::Complete);
        }

        // Get conversation history
        let history = Repos.chat.core.get_conversation_history( context.branch_id).await?;

        // Count user and assistant messages (skip system messages)
        let message_count = history
            .iter()
            .filter(|m| m.message.role == "user" || m.message.role == "assistant")
            .count();

        // Only generate title after first exchange (1 user + 1 assistant = 2 messages)
        if message_count != 2 {
            return Ok(ExtensionAction::Complete);
        }

        // Find first user message
        let first_user_message = history
            .iter()
            .find(|msg| msg.message.role == "user")
            .ok_or_else(|| AppError::internal_error("No user message found"))?;

        // Extract text content
        let user_content = first_user_message
            .contents
            .iter()
            .find_map(|c| {
                c.parse_content()
                    .ok()
                    .and_then(|data| data.to_text().map(|s| s.to_string()))
            })
            .ok_or_else(|| AppError::internal_error("No text content in user message"))?;

        // Get model name and IDs from context metadata
        let model_name = context
            .metadata
            .get("model_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::internal_error("Model name not in context"))?;

        // Get provider type from context
        let provider_type = context
            .metadata
            .get("provider_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::internal_error("Provider type not in context"))?;

        // Get provider_id from context
        let provider_id_str = context
            .metadata
            .get("provider_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::internal_error("Provider ID not in context"))?;

        let provider_id = uuid::Uuid::parse_str(provider_id_str)
            .map_err(|_| AppError::internal_error("Invalid provider ID in context"))?;

        // Fetch provider from database for api_key and base_url (not in context for security)
        let provider_info = Repos.llm_provider
            .get_by_id(provider_id)
            .await
            .map_err(AppError::database_error)?
            .ok_or_else(|| AppError::internal_error("Provider not found"))?;

        // Get API key and base URL
        let api_key = provider_info.api_key.as_deref().unwrap_or("");
        let base_url = provider_info
            .base_url
            .as_deref()
            .ok_or_else(|| AppError::internal_error(
                format!("Provider '{}' has no base_url configured", provider_info.name)
            ))?;

        // Create provider for title generation
        let provider = Provider::new(provider_type, api_key, base_url)
            .map_err(|e| AppError::internal_error(format!("Failed to create provider: {}", e)))?;

        // Try to generate title with AI
        let title = match self
            .generate_title_with_ai(&provider, model_name, &user_content)
            .await
        {
            Ok(title) => title,
            Err(e) => {
                eprintln!("Error generating title with AI: {}", e);
                // Fallback to simple title
                self.generate_simple_title(&user_content)
            }
        };

        // Update conversation title
        Repos.chat.core
            .update_conversation(context.conversation_id, context.user_id, Some(Some(title.clone())))
            .await?;

        // Send title event
        self.send_title_event(&title, tx);

        Ok(ExtensionAction::Complete)
    }
}
