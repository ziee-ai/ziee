//! AI Providers - Unified interface for multiple AI provider APIs
//!
//! This library provides a stateless, trait-based abstraction over different AI provider APIs
//! including OpenAI, Anthropic, and Gemini. All configuration (API keys, base URLs, etc.) is
//! passed as function parameters, making the library simple, flexible, and easy to test.
//!
//! # Architecture
//!
//! - **Stateless Design**: All providers are zero-sized structs with no stored state
//! - **Trait-Based**: Common `AIProvider` trait for uniform interface
//! - **Three Implementations**:
//!   - `OpenAIProvider`: Wraps the `openai` library (used for 7 provider types)
//!   - `GeminiProvider`: Wraps the `gemini-rust` library
//!   - `AnthropicProvider`: Custom implementation based on Anthropic API
//! - **Provider Registry**: Maps provider type strings to implementations
//!
//! # Example
//!
//! ```no_run
//! use ai_providers::{ProviderRegistry, ChatRequest, ChatMessage};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let registry = ProviderRegistry::new();
//!
//!     // Get OpenAI provider
//!     let provider = registry.get("openai").unwrap();
//!
//!     let request = ChatRequest {
//!         model: "gpt-4".to_string(),
//!         messages: vec![
//!             ChatMessage::user("Hello, how are you?"),
//!         ],
//!         ..Default::default()
//!     };
//!
//!     let response = provider.chat(
//!         "your-api-key",
//!         "https://api.openai.com/v1",
//!         request,
//!     ).await?;
//!
//!     println!("Response: {}", response.choices[0].message.content);
//!     Ok(())
//! }
//! ```
//!
//! # Supported Providers
//!
//! | Provider Type | Implementation | Base URL |
//! |--------------|----------------|----------|
//! | `openai` | OpenAIProvider | https://api.openai.com/v1 |
//! | `groq` | OpenAIProvider | https://api.groq.com/openai/v1 |
//! | `deepseek` | OpenAIProvider | https://api.deepseek.com/v1 |
//! | `mistral` | OpenAIProvider | https://api.mistral.ai/v1 |
//! | `huggingface` | OpenAIProvider | (various) |
//! | `local` | OpenAIProvider | http://localhost:8000/v1 |
//! | `custom` | OpenAIProvider | (custom) |
//! | `anthropic` | AnthropicProvider | https://api.anthropic.com/v1 |
//! | `gemini` | GeminiProvider | https://generativelanguage.googleapis.com/v1beta |

pub mod conversion;
pub mod error;
pub mod models;
pub mod providers;
pub mod registry;
pub mod traits;

// Re-export commonly used types
pub use error::ProviderError;
pub use models::*;
pub use providers::{AnthropicProvider, GeminiProvider, OpenAIProvider};
pub use registry::ProviderRegistry;
pub use traits::AIProvider;
