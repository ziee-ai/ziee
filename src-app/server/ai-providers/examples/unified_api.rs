//! Example demonstrating the unified Provider API
//!
//! This example shows how to use the new simplified API for streaming chat completions.
//!
//! Run with:
//! ```bash
//! OPENAI_API_KEY=sk-... cargo run --example unified_api
//! ```

use ai_providers::{ChatMessage, ChatRequest, Provider};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get API key from environment
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");

    println!("=== Unified Provider API Example ===\n");

    // Create a provider with credentials stored
    let provider = Provider::new("openai", api_key, "https://api.openai.com/v1")?;

    println!("Provider: {} ({})", provider.name(), provider.provider_type());
    println!();

    // Create a chat request
    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![
            ChatMessage::system("You are a helpful assistant."),
            ChatMessage::user("Count from 1 to 5, one number per line."),
        ],
        temperature: Some(0.1),
        ..Default::default()
    };

    println!("Streaming response:");
    println!("---");

    // Stream chat - no need to pass credentials again!
    let mut stream = provider.chat_stream(request).await?;

    let mut chunk_count = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        // Print text and thinking deltas
        for delta in &chunk.content {
            match delta {
                ai_providers::ContentBlockDelta::TextDelta { delta, .. } => {
                    print!("{}", delta);
                }
                ai_providers::ContentBlockDelta::ThinkingDelta { delta, .. } => {
                    print!("[THINKING: {}]", delta);
                }
                ai_providers::ContentBlockDelta::ToolUseDelta { .. } => {
                    // Skip tool use deltas in this example
                }
                _ => {
                    // Skip thinking-signature / redacted-thinking deltas in this example
                }
            }
        }
        chunk_count += 1;
    }

    println!();
    println!("---");
    println!("\nReceived {} chunks", chunk_count);
    println!("✅ Streaming complete!");

    Ok(())
}
