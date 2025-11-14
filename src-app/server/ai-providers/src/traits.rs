//! Core traits for AI providers

use crate::{
    error::ProviderError,
    models::{ChatRequest, EmbeddingsRequest, EmbeddingsResponse, StreamChatChunk},
};
use async_trait::async_trait;
use futures_core::Stream;
use std::pin::Pin;

/// Unified interface for AI providers
///
/// All providers implement this trait to provide a consistent API for streaming chat
/// and embeddings functionality. The trait is stateless - all configuration (API keys,
/// base URLs, etc.) must be passed as function parameters.
///
/// **STREAMING ONLY**: This library only supports streaming responses for optimal
/// real-time user experience. Non-streaming chat methods have been removed.
///
/// # Stateless Design
///
/// Providers are zero-sized structs or simple wrappers with no stored state. This design has several benefits:
/// - Simple: No complex initialization or state management
/// - Flexible: Different credentials can be used for each request
/// - Testable: Easy to mock and test without setup/teardown
/// - Thread-safe: No shared mutable state
///
/// # Example
///
/// ```no_run
/// use ai_providers::{OpenAIProvider, AIProvider, ChatRequest, ChatMessage};
/// use futures_util::StreamExt;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let provider = OpenAIProvider;
///
///     let request = ChatRequest {
///         model: "gpt-4".to_string(),
///         messages: vec![ChatMessage::user("Hello!")],
///         ..Default::default()
///     };
///
///     // All config passed as parameters (stateless)
///     let mut stream = provider.stream_chat(
///         "sk-...",                      // API key
///         "https://api.openai.com/v1",  // Base URL
///         request,
///     ).await?;
///
///     while let Some(chunk) = stream.next().await {
///         let chunk = chunk?;
///         print!("{}", chunk.content);
///     }
///
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait AIProvider: Send + Sync {
    /// Returns the human-readable name of the provider
    fn name(&self) -> &str;

    /// Sends a streaming chat completion request
    ///
    /// Returns a stream of response chunks that can be consumed incrementally.
    ///
    /// # Parameters
    ///
    /// - `api_key`: The API key for authentication
    /// - `base_url`: The base URL for the provider's API
    /// - `request`: The chat request containing messages, model, and parameters
    ///
    /// # Returns
    ///
    /// A stream of chat chunks that can be consumed as they arrive
    ///
    /// # Errors
    ///
    /// Returns `ProviderError` if the request fails to initiate or if there are
    /// errors during streaming.
    async fn stream_chat(
        &self,
        api_key: &str,
        base_url: &str,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChatChunk, ProviderError>> + Send>>, ProviderError>;

    /// Generates embeddings for the given input
    ///
    /// # Parameters
    ///
    /// - `api_key`: The API key for authentication
    /// - `base_url`: The base URL for the provider's API
    /// - `request`: The embeddings request containing input text and model
    ///
    /// # Returns
    ///
    /// The embeddings response containing vector representations
    ///
    /// # Errors
    ///
    /// Returns `ProviderError` if the request fails or if the provider doesn't
    /// support embeddings.
    async fn embeddings(
        &self,
        api_key: &str,
        base_url: &str,
        request: EmbeddingsRequest,
    ) -> Result<EmbeddingsResponse, ProviderError>;
}
