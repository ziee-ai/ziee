//! Anthropic provider integration tests
//!
//! These tests require a valid Anthropic API key set in the ANTHROPIC_API_KEY environment variable.
//! Run with: ANTHROPIC_API_KEY=your_key cargo test --test test_anthropic -- --nocapture

use ai_providers::*;

const BASE_URL: &str = "https://api.anthropic.com/v1";

// Models used in tests
const MODEL_OPUS_41: &str = "claude-opus-4-1-20250805";
const MODEL_SONNET_45: &str = "claude-sonnet-4-5-20250929";
const MODEL_HAIKU_45: &str = "claude-haiku-4-5-20251001";
const MODEL_HAIKU_35: &str = "claude-3-5-haiku-20241022";

fn get_api_key() -> String {
    std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY environment variable must be set")
}

#[tokio::test]
#[ignore]
async fn test_anthropic_streaming_chat() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("anthropic", &api_key, BASE_URL).expect("Failed to create provider");

    let request = ChatRequest {
        model: MODEL_HAIKU_45.to_string(),  // Claude Haiku 4.5 (fastest)
        messages: vec![ChatMessage::user("Count from 1 to 5, one number per line.")],
        temperature: Some(0.1),
        max_tokens: Some(100),
        ..Default::default()
    };

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Stream chat request failed");

    let mut full_content = String::new();
    let mut chunk_count = 0;

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                // Process content deltas
                for delta in &chunk.content {
                    match delta {
                        ai_providers::ContentBlockDelta::TextDelta { delta, .. } => {
                            full_content.push_str(delta);
                            print!("{}", delta);
                        }
                        ai_providers::ContentBlockDelta::ThinkingDelta { delta, .. } => {
                            full_content.push_str(&format!("[THINKING: {}]", delta));
                            print!("[THINKING: {}]", delta);
                        }
                        ai_providers::ContentBlockDelta::ToolUseDelta { .. }
                        | ai_providers::ContentBlockDelta::ThinkingSignatureDelta { .. }
                        | ai_providers::ContentBlockDelta::RedactedThinkingDelta { .. } => {
                            // Skip tool use deltas
                        }
                    }
                }
                chunk_count += 1;
            }
            Err(e) => panic!("Stream error: {:?}", e),
        }
    }

    println!("\n\nReceived {} chunks", chunk_count);
    println!("Full content: {}", full_content);

    assert!(chunk_count > 0);
    assert!(!full_content.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_anthropic_extended_thinking_streaming() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("anthropic", &api_key, BASE_URL).expect("Failed to create provider");

    let request = ChatRequest {
        model: MODEL_SONNET_45.to_string(),  // Claude Sonnet 4.5 with extended thinking
        messages: vec![ChatMessage::user("List all prime numbers between 1 and 50 with explanation.")],
        max_tokens: Some(12000),
        thinking: Some(ThinkingConfig::with_budget(8000)),
        ..Default::default()
    };

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Thinking streaming failed");

    let mut full_content = String::new();
    let mut full_thinking = String::new();
    let mut chunk_count = 0;

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                // Process content deltas
                for delta in &chunk.content {
                    match delta {
                        ai_providers::ContentBlockDelta::TextDelta { delta, .. } => {
                            full_content.push_str(delta);
                            print!("{}", delta);
                        }
                        ai_providers::ContentBlockDelta::ThinkingDelta { delta, .. } => {
                            full_thinking.push_str(delta);
                            print!("[THINKING: {}]", delta);
                        }
                        ai_providers::ContentBlockDelta::ToolUseDelta { .. }
                        | ai_providers::ContentBlockDelta::ThinkingSignatureDelta { .. }
                        | ai_providers::ContentBlockDelta::RedactedThinkingDelta { .. } => {
                            // Skip tool use deltas
                        }
                    }
                }
                chunk_count += 1;
            }
            Err(e) => panic!("Stream error: {:?}", e),
        }
    }

    println!("\n\nReceived {} chunks", chunk_count);
    println!("Full content: {}", full_content);
    if !full_thinking.is_empty() {
        println!("Full thinking (first 200 chars): {}...", &full_thinking.chars().take(200).collect::<String>());
    }

    assert!(chunk_count > 0);
    assert!(!full_content.is_empty());
}

// ==================== PRIORITY 1: CRITICAL MISSING MODELS ====================

#[tokio::test]
#[ignore]
async fn test_anthropic_opus_41_streaming() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("anthropic", &api_key, BASE_URL).expect("Failed to create provider");

    let request = ChatRequest {
        model: MODEL_OPUS_41.to_string(),
        messages: vec![ChatMessage::user("Write a Python function to calculate factorial. Include docstring.")],
        temperature: Some(0.1),
        max_tokens: Some(500),
        ..Default::default()
    };

    println!("\n=== Testing Claude Opus 4.1 (most powerful Claude model, best code generation) ===");

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Opus 4.1 stream chat request failed");

    let mut full_content = String::new();
    let mut chunk_count = 0;

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                for delta in &chunk.content {
                    match delta {
                        ai_providers::ContentBlockDelta::TextDelta { delta, .. } => {
                            full_content.push_str(delta);
                            print!("{}", delta);
                        }
                        ai_providers::ContentBlockDelta::ThinkingDelta { delta, .. } => {
                            full_content.push_str(&format!("[THINKING: {}]", delta));
                            print!("[THINKING: {}]", delta);
                        }
                        ai_providers::ContentBlockDelta::ToolUseDelta { .. }
                        | ai_providers::ContentBlockDelta::ThinkingSignatureDelta { .. }
                        | ai_providers::ContentBlockDelta::RedactedThinkingDelta { .. } => {}
                    }
                }
                chunk_count += 1;
            }
            Err(e) => panic!("Opus 4.1 stream error: {:?}", e),
        }
    }

    println!("\n\nOpus 4.1: Received {} chunks", chunk_count);
    println!("Full content: {}", full_content);

    assert!(chunk_count > 0, "Expected at least one chunk");
    assert!(!full_content.is_empty(), "Expected non-empty content");
}

// ==================== PRIORITY 2: LATEST GENERATION MODELS ====================

#[tokio::test]
#[ignore]
async fn test_anthropic_haiku_35_streaming() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("anthropic", &api_key, BASE_URL).expect("Failed to create provider");

    let request = ChatRequest {
        model: MODEL_HAIKU_35.to_string(),
        messages: vec![ChatMessage::user("Count from 1 to 5, one number per line.")],
        temperature: Some(0.1),
        max_tokens: Some(100),
        ..Default::default()
    };

    println!("\n=== Testing Claude 3.5 Haiku (matches Claude 3 Opus performance at Haiku speed) ===");

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Haiku 3.5 stream chat request failed");

    let mut full_content = String::new();
    let mut chunk_count = 0;

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                for delta in &chunk.content {
                    match delta {
                        ai_providers::ContentBlockDelta::TextDelta { delta, .. } => {
                            full_content.push_str(delta);
                            print!("{}", delta);
                        }
                        ai_providers::ContentBlockDelta::ThinkingDelta { delta, .. } => {
                            full_content.push_str(&format!("[THINKING: {}]", delta));
                            print!("[THINKING: {}]", delta);
                        }
                        ai_providers::ContentBlockDelta::ToolUseDelta { .. }
                        | ai_providers::ContentBlockDelta::ThinkingSignatureDelta { .. }
                        | ai_providers::ContentBlockDelta::RedactedThinkingDelta { .. } => {}
                    }
                }
                chunk_count += 1;
            }
            Err(e) => panic!("Haiku 3.5 stream error: {:?}", e),
        }
    }

    println!("\n\nHaiku 3.5: Received {} chunks", chunk_count);
    println!("Full content: {}", full_content);

    assert!(chunk_count > 0, "Expected at least one chunk");
    assert!(!full_content.is_empty(), "Expected non-empty content");
}
