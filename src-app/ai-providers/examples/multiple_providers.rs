//! Example demonstrating multiple providers with the unified API
//!
//! This example shows how easy it is to switch between different AI providers
//! using the same code structure.
//!
//! Run with:
//! ```bash
//! OPENAI_API_KEY=sk-... ANTHROPIC_API_KEY=sk-ant-... cargo run --example multiple_providers
//! ```

use ai_providers::{ChatMessage, ChatRequest, Provider};
use futures_util::StreamExt;

async fn test_provider(
    name: &str,
    provider: &Provider,
    prompt: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    println!("\n=== Testing {} ===", name);

    let request = ChatRequest {
        model: get_model_for_provider(provider.provider_type()),
        messages: vec![ChatMessage::user(prompt)],
        temperature: Some(0.1),
        max_tokens: Some(50),
        ..Default::default()
    };

    let mut stream = provider.chat_stream(request).await?;
    let mut full_response = String::new();
    let mut chunk_count = 0;

    print!("Response: ");
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        // Process content deltas
        for delta in &chunk.content {
            match delta {
                ai_providers::ContentBlockDelta::TextDelta { delta, .. } => {
                    print!("{}", delta);
                    full_response.push_str(delta);
                }
                ai_providers::ContentBlockDelta::ThinkingDelta { delta, .. } => {
                    print!("[THINKING: {}]", delta);
                    full_response.push_str(&format!("[THINKING: {}]", delta));
                }
                ai_providers::ContentBlockDelta::ToolUseDelta { .. } => {
                    // Skip tool use deltas in this example
                }
            }
        }
        chunk_count += 1;
    }

    println!("\n(Received {} chunks)", chunk_count);

    Ok(full_response)
}

fn get_model_for_provider(provider_type: &str) -> String {
    match provider_type {
        "openai" => "gpt-4".to_string(),
        "anthropic" => "claude-3-5-sonnet-20241022".to_string(),
        "gemini" => "gemini-2.5-flash".to_string(),
        "groq" => "llama-3.3-70b-versatile".to_string(),
        _ => "gpt-4".to_string(),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Multiple Providers Example ===");

    let prompt = "What is 2 + 2? Answer with just the number.";

    // OpenAI
    if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
        let provider = Provider::new("openai", api_key, "https://api.openai.com/v1")?;
        test_provider("OpenAI", &provider, prompt).await?;
    } else {
        println!("\n⚠️  Skipping OpenAI (OPENAI_API_KEY not set)");
    }

    // Anthropic
    if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
        let provider = Provider::new("anthropic", api_key, "https://api.anthropic.com/v1")?;
        test_provider("Anthropic", &provider, prompt).await?;
    } else {
        println!("\n⚠️  Skipping Anthropic (ANTHROPIC_API_KEY not set)");
    }

    // Gemini
    if let Ok(api_key) = std::env::var("GEMINI_API_KEY") {
        let provider = Provider::new("gemini", api_key, "")?;
        test_provider("Gemini", &provider, prompt).await?;
    } else {
        println!("\n⚠️  Skipping Gemini (GEMINI_API_KEY not set)");
    }

    // Groq (uses OpenAI-compatible API)
    if let Ok(api_key) = std::env::var("GROQ_API_KEY") {
        let provider = Provider::new("groq", api_key, "https://api.groq.com/openai/v1")?;
        test_provider("Groq", &provider, prompt).await?;
    } else {
        println!("\n⚠️  Skipping Groq (GROQ_API_KEY not set)");
    }

    println!("\n✅ All tests complete!");

    Ok(())
}
