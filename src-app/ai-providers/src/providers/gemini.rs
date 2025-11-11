//! Gemini provider implementation (wrapper around gemini-rust library)

use crate::{
    conversion::gemini as conv,
    error::ProviderError,
    models::{ChatRequest, ChatResponse, EmbeddingsRequest, EmbeddingsResponse, StreamChatChunk},
    traits::AIProvider,
};
use async_stream::stream;
use async_trait::async_trait;
use futures_core::Stream;
use futures_util::TryStreamExt;
use gemini_rust::{Gemini, GeminiBuilder, Model};
use std::pin::Pin;

/// Gemini provider (zero-sized, stateless)
pub struct GeminiProvider;

impl GeminiProvider {
    /// Builds a Gemini client for a single request (stateless)
    fn build_client(api_key: &str, model: &str) -> Result<Gemini, ProviderError> {
        let gemini_model = if model.starts_with("models/") {
            Model::Custom(model.to_string())
        } else {
            Model::Custom(format!("models/{}", model))
        };

        GeminiBuilder::new(api_key)
            .with_model(gemini_model)
            .build()
            .map_err(|e| ProviderError::from(e))
    }
}

#[async_trait]
impl AIProvider for GeminiProvider {
    fn name(&self) -> &str {
        "Gemini"
    }

    fn provider_type(&self) -> &str {
        "gemini"
    }

    async fn chat(
        &self,
        api_key: &str,
        _base_url: &str, // Gemini library doesn't support custom base URLs easily
        request: ChatRequest,
    ) -> Result<ChatResponse, ProviderError> {
        // Build client
        let client = Self::build_client(api_key, &request.model)?;

        // Build generation request
        let mut gen_builder = client.generate_content();

        // Add system instruction if present
        if let Some(system) = conv::extract_system_instruction(&request.messages) {
            gen_builder = gen_builder.with_system_instruction(&system);
        }

        // Add conversation history
        let contents = conv::convert_messages_to_contents(&request.messages, &request.attachments);
        for content in contents {
            match content.role {
                Some(gemini_rust::Role::User) => {
                    if let Some(parts) = content.parts {
                        for part in parts {
                            if let gemini_rust::Part::Text { text, .. } = part {
                                gen_builder = gen_builder.with_user_message(&text);
                            }
                        }
                    }
                }
                Some(gemini_rust::Role::Model) => {
                    if let Some(parts) = content.parts {
                        for part in parts {
                            if let gemini_rust::Part::Text { text, .. } = part {
                                gen_builder = gen_builder.with_model_message(&text);
                            }
                        }
                    }
                }
                None => {}
            }
        }

        // Add generation config
        if let Some(temp) = request.temperature {
            gen_builder = gen_builder.with_temperature(temp);
        }
        if let Some(max_tokens) = request.max_tokens {
            gen_builder = gen_builder.with_max_output_tokens(max_tokens as i32);
        }
        if let Some(top_p) = request.top_p {
            gen_builder = gen_builder.with_top_p(top_p);
        }

        // Add tools if provided
        if !request.tools.is_empty() {
            let tools = conv::convert_tools(&request.tools);
            for tool in tools {
                gen_builder = gen_builder.with_tool(tool);
            }
        }

        // Add tool choice if provided
        if let Some(ref tool_choice) = request.tool_choice {
            let mode = conv::convert_tool_choice(tool_choice);
            gen_builder = gen_builder.with_function_calling_mode(mode);
        }

        // Add thinking configuration if provided
        if let Some(ref thinking) = request.thinking {
            if thinking.enabled {
                // Determine thinking budget
                let budget = if let Some(ref effort) = thinking.effort {
                    match effort {
                        crate::models::ThinkingEffort::Dynamic => -1, // Dynamic thinking
                        _ => thinking.budget_tokens.unwrap_or(-1), // Use provided or dynamic
                    }
                } else {
                    thinking.budget_tokens.unwrap_or(-1)
                };

                gen_builder = gen_builder.with_thinking_budget(budget);
                // Note: Thoughts are automatically included when thinking_budget is set
            }
        }

        // Send request
        let response = gen_builder.execute().await?;

        // Convert response
        Ok(conv::convert_generation_response(
            response,
            request.model.clone(),
        ))
    }

    async fn stream_chat(
        &self,
        api_key: &str,
        _base_url: &str,
        request: ChatRequest,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<StreamChatChunk, ProviderError>> + Send>>,
        ProviderError,
    > {
        // Build client
        let client = Self::build_client(api_key, &request.model)?;

        // Build generation request
        let mut gen_builder = client.generate_content();

        // Add system instruction
        if let Some(system) = conv::extract_system_instruction(&request.messages) {
            gen_builder = gen_builder.with_system_instruction(&system);
        }

        // Add conversation history
        let contents = conv::convert_messages_to_contents(&request.messages, &request.attachments);
        for content in contents {
            match content.role {
                Some(gemini_rust::Role::User) => {
                    if let Some(parts) = content.parts {
                        for part in parts {
                            if let gemini_rust::Part::Text { text, .. } = part {
                                gen_builder = gen_builder.with_user_message(&text);
                            }
                        }
                    }
                }
                Some(gemini_rust::Role::Model) => {
                    if let Some(parts) = content.parts {
                        for part in parts {
                            if let gemini_rust::Part::Text { text, .. } = part {
                                gen_builder = gen_builder.with_model_message(&text);
                            }
                        }
                    }
                }
                None => {}
            }
        }

        // Add generation config
        if let Some(temp) = request.temperature {
            gen_builder = gen_builder.with_temperature(temp);
        }
        if let Some(max_tokens) = request.max_tokens {
            gen_builder = gen_builder.with_max_output_tokens(max_tokens as i32);
        }
        if let Some(top_p) = request.top_p {
            gen_builder = gen_builder.with_top_p(top_p);
        }

        // Add tools if provided
        if !request.tools.is_empty() {
            let tools = conv::convert_tools(&request.tools);
            for tool in tools {
                gen_builder = gen_builder.with_tool(tool);
            }
        }

        // Add tool choice if provided
        if let Some(ref tool_choice) = request.tool_choice {
            let mode = conv::convert_tool_choice(tool_choice);
            gen_builder = gen_builder.with_function_calling_mode(mode);
        }

        // Add thinking configuration if provided
        if let Some(ref thinking) = request.thinking {
            if thinking.enabled {
                // Determine thinking budget
                let budget = if let Some(ref effort) = thinking.effort {
                    match effort {
                        crate::models::ThinkingEffort::Dynamic => -1,
                        _ => thinking.budget_tokens.unwrap_or(-1),
                    }
                } else {
                    thinking.budget_tokens.unwrap_or(-1)
                };

                gen_builder = gen_builder.with_thinking_budget(budget);
                // Note: Thoughts are automatically included when thinking_budget is set
            }
        }

        // Get streaming response
        let mut gemini_stream = gen_builder.execute_stream().await?;

        // Create our stream
        let output_stream = stream! {
            while let Some(result) = gemini_stream.try_next().await.transpose() {
                match result {
                    Ok(response) => {
                        if let Some(chunk) = conv::convert_stream_chunk_from_response(&response) {
                            yield Ok(chunk);
                        }
                    }
                    Err(e) => {
                        yield Err(ProviderError::from(e));
                        break;
                    }
                }
            }
        };

        Ok(Box::pin(output_stream))
    }

    async fn embeddings(
        &self,
        api_key: &str,
        _base_url: &str,
        request: EmbeddingsRequest,
    ) -> Result<EmbeddingsResponse, ProviderError> {
        // Build client
        let client = Self::build_client(api_key, &request.model)?;

        // Gemini supports batch embeddings
        if request.input.len() == 1 {
            // Single embedding
            let response = client
                .embed_content()
                .with_text(&request.input[0])
                .execute()
                .await?;

            Ok(conv::convert_embeddings_response(response))
        } else {
            // Batch embeddings
            let response = client
                .embed_content()
                .with_chunks(request.input.clone())
                .execute_batch()
                .await?;

            Ok(conv::convert_batch_embeddings_response(response))
        }
    }
}
