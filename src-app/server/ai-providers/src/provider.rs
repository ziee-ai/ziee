//! Unified Provider API
//!
//! This module provides a simplified, ergonomic API for using AI providers.
//! Instead of dealing with different provider structs and passing credentials
//! to each method, you create a Provider instance and call methods directly.
//!
//! # Example
//!
//! ```no_run
//! use ai_providers::{Provider, ChatRequest, ChatMessage};
//! use futures_util::StreamExt;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create provider with credentials
//!     let provider = Provider::new(
//!         "openai",
//!         "sk-...",
//!         "https://api.openai.com/v1"
//!     )?;
//!
//!     // Stream chat without passing credentials again
//!     let request = ChatRequest {
//!         model: "gpt-4".to_string(),
//!         messages: vec![ChatMessage::user("Hello!")],
//!         ..Default::default()
//!     };
//!
//!     let mut stream = provider.chat_stream(request).await?;
//!     while let Some(chunk) = stream.next().await {
//!         print!("{:?}", chunk?.content);
//!     }
//!
//!     Ok(())
//! }
//! ```

use crate::{
    error::ProviderError,
    models::{ChatRequest, EmbeddingsRequest, EmbeddingsResponse, FileUpload, FileUploadResponse, StreamChatChunk},
    providers::{AnthropicProvider, GeminiProvider, OpenAIProvider},
    traits::AIProvider,
};
use futures_core::Stream;
use std::pin::Pin;

/// Unified provider that wraps different AI provider implementations
///
/// This struct provides a simple, ergonomic API by storing credentials
/// and delegating to the appropriate underlying provider implementation.
///
/// HTTP requests use the process-wide hardened client from
/// [`crate::providers::http_client`] (pooled, timeouts, no redirects).
pub struct Provider {
    inner: Box<dyn AIProvider>,
    api_key: String,
    base_url: String,
    provider_type: String,
}

impl Provider {
    /// Creates a new Provider instance
    ///
    /// # Arguments
    ///
    /// * `provider_type` - The type of provider: "openai", "anthropic", "gemini", "groq", etc.
    /// * `api_key` - The API key for authentication
    /// * `base_url` - The base URL for the API (can be empty string for defaults)
    ///
    /// # Returns
    ///
    /// Returns a configured Provider instance or an error if the provider type is unknown.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ai_providers::Provider;
    ///
    /// let provider = Provider::new(
    ///     "openai",
    ///     "sk-...",
    ///     "https://api.openai.com/v1"
    /// )?;
    /// # Ok::<(), ai_providers::ProviderError>(())
    /// ```
    pub fn new(
        provider_type: impl Into<String>,
        api_key: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        let provider_type = provider_type.into();
        let api_key = api_key.into();
        let base_url = base_url.into();

        // Reject a base_url with a non-HTTP scheme (e.g. `file://`, `ftp://`).
        // Empty is allowed — each provider fills in its own default. http:// is
        // allowed for `local`/self-hosted endpoints (the key, if any, then goes
        // in cleartext — operator's choice).
        if !base_url.trim().is_empty() {
            let b = base_url.trim();
            if !(b.starts_with("http://") || b.starts_with("https://")) {
                return Err(ProviderError::InvalidRequest(format!(
                    "Invalid base_url '{}': must start with http:// or https://",
                    base_url
                )));
            }
        }

        let inner: Box<dyn AIProvider> = match provider_type.as_str() {
            "openai" | "groq" | "deepseek" | "mistral" | "huggingface" | "local" | "custom" => {
                Box::new(OpenAIProvider)
            }
            "anthropic" => Box::new(AnthropicProvider),
            "gemini" => Box::new(GeminiProvider),
            _ => {
                return Err(ProviderError::InvalidRequest(format!(
                    "Unknown provider type: '{}'. Supported: openai, anthropic, gemini, groq, deepseek, mistral",
                    provider_type
                )))
            }
        };

        Ok(Self {
            inner,
            api_key,
            base_url,
            provider_type,
        })
    }

    /// Returns the provider type string
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use ai_providers::Provider;
    /// let provider = Provider::new("openai", "sk-...", "")?;
    /// assert_eq!(provider.provider_type(), "openai");
    /// # Ok::<(), ai_providers::ProviderError>(())
    /// ```
    pub fn provider_type(&self) -> &str {
        &self.provider_type
    }

    /// Returns the human-readable provider name
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use ai_providers::Provider;
    /// let provider = Provider::new("openai", "sk-...", "")?;
    /// assert_eq!(provider.name(), "OpenAI");
    /// # Ok::<(), ai_providers::ProviderError>(())
    /// ```
    pub fn name(&self) -> &str {
        self.inner.name()
    }

    /// Streams a chat completion request
    ///
    /// Returns a stream of response chunks that can be consumed incrementally.
    ///
    /// # Arguments
    ///
    /// * `request` - The chat request containing messages, model, and parameters
    ///
    /// # Returns
    ///
    /// A stream of chat chunks that can be consumed as they arrive
    ///
    /// # Errors
    ///
    /// Returns `ProviderError` if the request fails to initiate or if there are
    /// errors during streaming.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ai_providers::{Provider, ChatRequest, ChatMessage};
    /// use futures_util::StreamExt;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let provider = Provider::new("openai", "sk-...", "https://api.openai.com/v1")?;
    ///
    /// let request = ChatRequest {
    ///     model: "gpt-4".to_string(),
    ///     messages: vec![ChatMessage::user("Hello!")],
    ///     ..Default::default()
    /// };
    ///
    /// let mut stream = provider.chat_stream(request).await?;
    ///
    /// while let Some(chunk) = stream.next().await {
    ///     let chunk = chunk?;
    ///     print!("{:?}", chunk.content);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        self.inner
            .stream_chat(&self.api_key, &self.base_url, request)
            .await
    }

    /// Generates embeddings for the given input
    ///
    /// # Arguments
    ///
    /// * `request` - The embeddings request containing input text and model
    ///
    /// # Returns
    ///
    /// The embeddings response containing vector representations
    ///
    /// # Errors
    ///
    /// Returns `ProviderError` if the request fails or if the provider doesn't
    /// support embeddings.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ai_providers::{Provider, EmbeddingsRequest};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let provider = Provider::new("openai", "sk-...", "https://api.openai.com/v1")?;
    ///
    /// let request = EmbeddingsRequest {
    ///     model: "text-embedding-3-small".to_string(),
    ///     input: vec!["Hello, world!".to_string()],
    /// };
    ///
    /// let response = provider.embeddings(request).await?;
    /// println!("Embedding dimensions: {}", response.embeddings[0].len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn embeddings(
        &self,
        request: EmbeddingsRequest,
    ) -> Result<EmbeddingsResponse, ProviderError> {
        self.inner
            .embeddings(&self.api_key, &self.base_url, request)
            .await
    }

    /// Uploads a file to the provider's Files API (if supported)
    ///
    /// Returns `None` if the provider doesn't support file uploads.
    ///
    /// # Arguments
    ///
    /// * `upload` - The file upload request containing filename, data, and MIME type
    ///
    /// # Returns
    ///
    /// - `Ok(Some(FileUploadResponse))` if upload succeeds
    /// - `Ok(None)` if provider doesn't support file uploads
    /// - `Err(ProviderError)` if upload fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ai_providers::{Provider, FileUpload};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let provider = Provider::new("anthropic", "sk-ant-...", "https://api.anthropic.com/v1")?;
    ///
    /// let upload = FileUpload {
    ///     filename: "document.pdf".to_string(),
    ///     file_data: vec![/* PDF bytes */],
    ///     mime_type: "application/pdf".to_string(),
    /// };
    ///
    /// if let Some(result) = provider.upload_file(upload).await? {
    ///     println!("Uploaded file ID: {}", result.provider_file_id);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn upload_file(
        &self,
        upload: FileUpload,
    ) -> Result<Option<FileUploadResponse>, ProviderError> {
        self.inner
            .upload_file(&self.api_key, &self.base_url, upload)
            .await
    }

    /// Checks if this provider supports file upload APIs
    ///
    /// # Returns
    ///
    /// - `true` if provider has a Files API (Anthropic, Gemini)
    /// - `false` otherwise (OpenAI, Groq, etc.)
    pub fn supports_file_api(&self) -> bool {
        self.inner.supports_file_api()
    }

    /// Deletes a file from the provider's storage (if supported)
    ///
    /// # Arguments
    ///
    /// * `provider_file_id` - The provider's file ID to delete
    ///
    /// # Errors
    ///
    /// Returns `ProviderError` if deletion fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ai_providers::Provider;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let provider = Provider::new("anthropic", "sk-ant-...", "https://api.anthropic.com/v1")?;
    /// provider.delete_file("file-abc123").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete_file(&self, provider_file_id: &str) -> Result<(), ProviderError> {
        self.inner
            .delete_file(&self.api_key, &self.base_url, provider_file_id)
            .await
    }
}

impl std::fmt::Debug for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Provider")
            .field("provider_type", &self.provider_type)
            .field("name", &self.inner.name())
            .field("base_url", &self.base_url)
            .field("api_key", &"***")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation_openai() {
        let provider = Provider::new("openai", "sk-test", "https://api.openai.com/v1").unwrap();
        assert_eq!(provider.provider_type(), "openai");
        assert_eq!(provider.name(), "OpenAI");
    }

    #[test]
    fn test_provider_creation_anthropic() {
        let provider =
            Provider::new("anthropic", "sk-ant-test", "https://api.anthropic.com/v1").unwrap();
        assert_eq!(provider.provider_type(), "anthropic");
        assert_eq!(provider.name(), "Anthropic");
    }

    #[test]
    fn test_provider_creation_gemini() {
        let provider = Provider::new("gemini", "test-key", "").unwrap();
        assert_eq!(provider.provider_type(), "gemini");
        assert_eq!(provider.name(), "Gemini");
    }

    #[test]
    fn test_provider_creation_groq() {
        let provider =
            Provider::new("groq", "gsk-test", "https://api.groq.com/openai/v1").unwrap();
        assert_eq!(provider.provider_type(), "groq");
        assert_eq!(provider.name(), "OpenAI"); // Uses OpenAI provider
    }

    #[test]
    fn test_provider_unknown_type() {
        let result = Provider::new("unknown", "test", "");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown provider type"));
    }

    #[test]
    fn test_provider_rejects_non_http_base_url() {
        // Non-HTTP schemes are refused (SSRF / scheme-confusion guard).
        assert!(Provider::new("openai", "k", "file:///etc/passwd").is_err());
        assert!(Provider::new("openai", "k", "ftp://example.com").is_err());
        // Empty (provider fills its default) and http(s) are accepted; http is
        // allowed for local/self-hosted endpoints.
        assert!(Provider::new("openai", "k", "").is_ok());
        assert!(Provider::new("openai", "k", "https://api.openai.com/v1").is_ok());
        assert!(Provider::new("local", "k", "http://localhost:8000/v1").is_ok());
    }

    #[test]
    fn test_provider_debug() {
        let provider = Provider::new("openai", "sk-secret", "https://api.openai.com/v1").unwrap();
        let debug_str = format!("{:?}", provider);
        assert!(debug_str.contains("openai"));
        assert!(debug_str.contains("OpenAI"));
        assert!(debug_str.contains("***")); // API key should be hidden
        assert!(!debug_str.contains("sk-secret")); // API key should not appear
    }
}
