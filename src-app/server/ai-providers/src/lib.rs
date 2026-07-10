//! AI Providers - Unified interface for multiple AI provider APIs
//!
//! This library provides a simple, ergonomic API for working with different AI providers
//! including OpenAI, Anthropic, Gemini, and others. The library supports streaming-only
//! chat completions and embeddings.
//!
//! # Quick Start
//!
//! ```no_run
//! use ai_providers::{Provider, ChatRequest, ChatMessage};
//! use futures_util::StreamExt;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a provider with credentials
//!     let provider = Provider::new(
//!         "openai",
//!         "sk-...",
//!         "https://api.openai.com/v1"
//!     )?;
//!
//!     // Stream chat completion
//!     let request = ChatRequest {
//!         model: "gpt-4".to_string(),
//!         messages: vec![ChatMessage::user("Hello!")],
//!         ..Default::default()
//!     };
//!
//!     let mut stream = provider.chat_stream(request).await?;
//!
//!     while let Some(chunk) = stream.next().await {
//!         print!("{:?}", chunk?.content);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! # Architecture
//!
//! - **Simple API**: Create a `Provider` with credentials, call methods without passing them again
//! - **Streaming Only**: All chat completions use streaming for optimal user experience
//! - **Single Interface**: Only the unified `Provider` is exported - no individual provider types
//!
//! # Supported Providers
//!
//! | Provider Type | Default Base URL |
//! |--------------|------------------|
//! | `openai` | https://api.openai.com/v1 |
//! | `groq` | https://api.groq.com/openai/v1 |
//! | `deepseek` | https://api.deepseek.com/v1 |
//! | `mistral` | https://api.mistral.ai/v1 |
//! | `huggingface` | (various) |
//! | `local` | http://localhost:8000/v1 |
//! | `custom` | (custom) |
//! | `anthropic` | https://api.anthropic.com/v1 |
//! | `gemini` | https://generativelanguage.googleapis.com/v1beta |

mod error;
pub mod model_registry;
mod models;
pub mod param_policy;
mod provider;
mod providers;
mod traits;

// Re-export the unified Provider API
pub use provider::Provider;

// Re-export commonly used types
pub use error::ProviderError;
pub use models::*;

// Re-export individual providers for testing
pub use providers::{AnthropicProvider, GeminiProvider, OpenAIProvider};

// The Anthropic REST API version, shared with the model-discovery probe in the
// server crate so it does not keep a divergent copy of the header value.
pub use providers::anthropic::ANTHROPIC_VERSION;

// Re-export the curated catalog (P1.j).
pub use model_registry::{lookup as registry_lookup, known_ids_for as registry_known_ids};

// Re-export the declarative parameter-contract layer so the server can build a
// `ModelParamContract` from a DB model row and resolve thinking/param policy.
pub use param_policy::{
    ModelParamContract, ProviderFamily, ResolvedParams, UnifiedParam, MaxTokensField,
    resolved_thinking_style,
};

// Re-export AIProvider trait for testing
pub use traits::AIProvider;
