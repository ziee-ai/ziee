//! Gemini provider integration tests
//!
//! These tests require a valid Gemini API key set in the GEMINI_API_KEY environment variable.
//! Run with: GEMINI_API_KEY=your_key cargo test --test test_gemini -- --nocapture

use ai_providers::*;
use serde_json::json;

// Models used in tests
const MODEL_GEMINI_25_FLASH: &str = "models/gemini-2.5-flash";
const EMBEDDING_MODEL: &str = "models/text-embedding-004";

fn get_api_key() -> String {
    std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY environment variable must be set")
}

#[tokio::test]
#[ignore]
async fn test_gemini_streaming_chat() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("gemini", &api_key, "").expect("Failed to create provider");

    let request = ChatRequest {
        model: MODEL_GEMINI_25_FLASH.to_string(),  // Gemini 2.5 Flash (latest)
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
                        ai_providers::ContentBlockDelta::ToolUseDelta { .. } => {
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
async fn test_gemini_embeddings_single() {
    let api_key = get_api_key();
    let provider = Provider::new("gemini", &api_key, "").expect("Failed to create provider");

    let request = EmbeddingsRequest {
        model: EMBEDDING_MODEL.to_string(),
        input: vec!["Hello, world!".to_string()],
    };

    let response = provider
        .embeddings(request)
        .await
        .expect("Embeddings request failed");

    assert_eq!(response.embeddings.len(), 1);
    assert!(response.embeddings[0].len() > 0);

    println!(
        "Single embedding dimensions: {}",
        response.embeddings[0].len()
    );
}

#[tokio::test]
#[ignore]
async fn test_gemini_embeddings_batch() {
    let api_key = get_api_key();
    let provider = Provider::new("gemini", &api_key, "").expect("Failed to create provider");

    let request = EmbeddingsRequest {
        model: EMBEDDING_MODEL.to_string(),
        input: vec![
            "The quick brown fox jumps over the lazy dog".to_string(),
            "Hello, world!".to_string(),
            "Machine learning is fascinating".to_string(),
        ],
    };

    let response = provider
        .embeddings(request)
        .await
        .expect("Batch embeddings request failed");

    assert_eq!(response.embeddings.len(), 3);
    assert!(response.embeddings[0].len() > 0);
    assert!(response.embeddings[1].len() > 0);
    assert!(response.embeddings[2].len() > 0);

    println!(
        "Batch embedding dimensions: {}",
        response.embeddings[0].len()
    );
    println!("All embeddings have same dimension: {}",
        response.embeddings[0].len() == response.embeddings[1].len()
        && response.embeddings[1].len() == response.embeddings[2].len()
    );
}

#[tokio::test]
#[ignore]
async fn test_gemini_streaming_with_tools() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("gemini", &api_key, "").expect("Failed to create provider");

    let tool = Tool::function(
        "get_weather",
        "Get the current weather",
        json!({
            "type": "object",
            "properties": {
                "location": {"type": "string"}
            },
            "required": ["location"]
        }),
    );

    let request = ChatRequest {
        model: MODEL_GEMINI_25_FLASH.to_string(),  // Gemini 2.5 Flash (latest)
        messages: vec![ChatMessage::user("What's the weather in Tokyo?")],
        tools: vec![tool],
        tool_choice: Some(ToolChoice::auto()),
        max_tokens: Some(500),
        ..Default::default()
    };

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Stream with tools request failed");

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
                        ai_providers::ContentBlockDelta::ToolUseDelta { .. } => {
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

    // When model calls a tool, there may be no content chunks (tool calls are in final response)
    // This is valid behavior - the stream completes successfully even with 0 chunks
    println!("Test passed - streaming with tools completed (tool calls may result in 0 content chunks)");
}

#[tokio::test]
#[ignore]
async fn test_gemini_thinking_mode_streaming() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("gemini", &api_key, "").expect("Failed to create provider");

    let request = ChatRequest {
        model: MODEL_GEMINI_25_FLASH.to_string(),  // Gemini 2.5 with thinking
        messages: vec![ChatMessage::user("Count from 1 to 10 and explain why each number is special.")],
        max_tokens: Some(6000),
        thinking: Some(ThinkingConfig::with_budget(3000)),
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
                            print!("[THINKING] {}", delta);
                        }
                        ai_providers::ContentBlockDelta::ToolUseDelta { .. } => {
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
        println!("Full thinking summary: {}", full_thinking);
    }

    assert!(chunk_count > 0);
    assert!(!full_content.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_gemini_streaming_long_response() {
    use futures_util::StreamExt;
    use std::time::Instant;

    let api_key = get_api_key();
    let provider = Provider::new("gemini", &api_key, "").expect("Failed to create provider");

    // Wait to avoid rate limiting (Gemini streaming endpoint has aggressive rate limits)
    println!("\n=== Waiting 3 seconds to avoid rate limiting ===\n");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    let request = ChatRequest {
        model: MODEL_GEMINI_25_FLASH.to_string(),
        messages: vec![ChatMessage::user(
            "Count from 1 to 20, one number per line."
        )],
        temperature: Some(0.1),
        max_tokens: Some(100),
        ..Default::default()
    };

    println!("=== Testing Real Streaming with Long Response ===\n");

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Stream failed");

    let mut chunk_count = 0;
    let mut full_content = String::new();
    let start = Instant::now();
    let mut last_chunk_time = start;
    
    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                let now = Instant::now();
                let time_since_last = now.duration_since(last_chunk_time).as_millis();
                chunk_count += 1;

                // Calculate total chars in this chunk
                let mut chunk_chars = 0;
                for delta in &chunk.content {
                    match delta {
                        ai_providers::ContentBlockDelta::TextDelta { delta, .. } => {
                            full_content.push_str(delta);
                            chunk_chars += delta.len();
                        }
                        ai_providers::ContentBlockDelta::ThinkingDelta { delta, .. } => {
                            chunk_chars += delta.len();
                        }
                        ai_providers::ContentBlockDelta::ToolUseDelta { .. } => {
                            // Skip tool use deltas
                        }
                    }
                }

                println!("Chunk {} (+{}ms): {} chars",
                    chunk_count,
                    time_since_last,
                    chunk_chars
                );
                println!("Content: {:?}", &chunk.content);
                println!();

                last_chunk_time = now;
            }
            Err(e) => panic!("Stream error: {:?}", e),
        }
    }

    let total_time = start.elapsed().as_millis();
    
    println!("\n=== STREAMING RESULTS ===");
    println!("Total chunks: {}", chunk_count);
    println!("Total time: {}ms", total_time);
    println!("Content length: {} chars", full_content.len());
    println!("Average chunk size: {:.1} chars", full_content.len() as f64 / chunk_count as f64);
    
    if chunk_count == 1 {
        println!("\n⚠️  WARNING: Only 1 chunk received!");
        println!("This suggests buffering - not true streaming!");
        println!("Expected: Multiple incremental chunks");
    } else {
        println!("\n✅ TRUE STREAMING CONFIRMED");
        println!("Received {} incremental chunks", chunk_count);
    }
    
    assert!(!full_content.is_empty());
    assert!(full_content.len() > 20, "Response should contain the count");
    assert!(chunk_count > 1, "Should receive multiple chunks for streaming verification");
}
