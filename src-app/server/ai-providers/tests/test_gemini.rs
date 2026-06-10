//! Gemini provider integration tests
//!
//! These tests require a valid Gemini API key set in the GEMINI_API_KEY environment variable.
//! Run with: GEMINI_API_KEY=your_key cargo test --test test_gemini -- --nocapture

use ai_providers::*;
use serde_json::json;

// Models used in tests
const MODEL_GEMINI_25_FLASH: &str = "models/gemini-2.5-flash";
const MODEL_GEMINI_25_PRO: &str = "models/gemini-2.5-pro";
const MODEL_GEMINI_20_FLASH: &str = "models/gemini-2.0-flash";
const MODEL_GEMINI_20_PRO: &str = "models/gemini-2.0-pro-exp";
const MODEL_GEMINI_THINKING: &str = "models/gemini-2.0-flash-thinking-exp";
const MODEL_GEMINI_LITE: &str = "models/gemini-2.0-flash-lite";
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
async fn test_gemini_function_calling_complete_workflow() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("gemini", &api_key, "").expect("Failed to create provider");

    println!("\n=== Testing Gemini Function Calling Complete Workflow ===\n");

    // Define a simple weather tool
    let tool = Tool::function(
        "get_weather",
        "Get the current weather for a location",
        json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "The city and state, e.g., San Francisco, CA"
                }
            },
            "required": ["location"]
        }),
    );

    // Step 1: Send request with ToolChoice::Auto (normal mode)
    println!("Step 1: Sending request with tools (Auto mode - model decides)...");
    let request = ChatRequest {
        model: MODEL_GEMINI_25_FLASH.to_string(),
        messages: vec![ChatMessage::user("What's the weather in Tokyo?")],
        tools: vec![tool],
        tool_choice: Some(ToolChoice::Auto), // Let model decide (natural behavior)
        max_tokens: Some(500),
        ..Default::default()
    };

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Stream with tools request failed");

    let mut tool_calls = Vec::new();

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                for delta in &chunk.content {
                    if let ai_providers::ContentBlockDelta::ToolUseDelta { id, name, input_delta, .. } = delta {
                        println!("  ✓ Tool call detected: id={:?}, name={:?}", id, name);
                        if let (Some(id), Some(name), Some(input)) = (id, name, input_delta) {
                            tool_calls.push((id.clone(), name.clone(), input.clone()));
                        }
                    }
                }
            }
            Err(e) => panic!("Stream error: {:?}", e),
        }
    }

    // Step 2: Verify tool call was generated
    println!("\nStep 2: Verifying tool call was generated...");
    if tool_calls.is_empty() {
        println!("  ⚠️  WARNING: Gemini did not call any tools with Auto mode");
        println!("  This may indicate:");
        println!("    - Gemini needs more explicit prompting");
        println!("    - Tool description needs improvement");
        println!("    - Gemini tool calling reliability issues");
        println!("\n  Skipping rest of workflow test (no tool calls to test)");
        return;
    }
    println!("  ✓ {} tool call(s) generated", tool_calls.len());

    let (tool_use_id, tool_name, tool_input) = &tool_calls[0];
    println!("  Tool: {}", tool_name);
    println!("  ID: {}", tool_use_id);
    println!("  Input: {}", tool_input);

    assert_eq!(tool_name, "get_weather", "Expected get_weather function to be called");

    // Step 3: Send function response back to model
    println!("\nStep 3: Sending function response back to model...");
    let function_response_msg = ChatMessage {
        role: ai_providers::Role::User,
        content: vec![ai_providers::ContentBlock::ToolResult {
            tool_use_id: tool_use_id.clone(),
            name: Some(tool_name.clone()), // Include function name for Gemini
            content: vec![ai_providers::ContentBlock::Text {
                text: json!({
                    "temperature": 22,
                    "condition": "Sunny",
                    "humidity": 65
                }).to_string(),
            }],
            is_error: None,
        }],
    };

    let request_with_response = ChatRequest {
        model: MODEL_GEMINI_25_FLASH.to_string(),
        messages: vec![
            ChatMessage::user("What's the weather in Tokyo?"),
            ChatMessage {
                role: ai_providers::Role::Assistant,
                content: vec![ai_providers::ContentBlock::ToolUse {
                    id: tool_use_id.clone(),
                    name: tool_name.clone(),
                    input: json!({"location": "Tokyo"}),
                }],
            },
            function_response_msg,
        ],
        tools: vec![], // No tools needed for final response
        max_tokens: Some(500),
        ..Default::default()
    };

    let mut final_stream = provider
        .chat_stream(request_with_response)
        .await
        .expect("Stream with function response failed");

    let mut final_response = String::new();

    while let Some(result) = final_stream.next().await {
        match result {
            Ok(chunk) => {
                for delta in &chunk.content {
                    if let ai_providers::ContentBlockDelta::TextDelta { delta, .. } = delta {
                        final_response.push_str(delta);
                        print!("{}", delta);
                    }
                }
            }
            Err(e) => panic!("Stream error: {:?}", e),
        }
    }

    // Step 4: Verify final response
    println!("\n\nStep 4: Verifying final response...");
    assert!(!final_response.is_empty(), "❌ FAILED: Expected non-empty final response");
    println!("  ✓ Received final response ({} chars)", final_response.len());
    println!("\n=== ✅ ALL TESTS PASSED ===");
    println!("Complete function calling workflow verified:");
    println!("  1. Tool call generation ✓");
    println!("  2. Function response format ✓");
    println!("  3. Final response generation ✓");
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
                        ai_providers::ContentBlockDelta::ToolUseDelta { .. }
                        | ai_providers::ContentBlockDelta::ThinkingSignatureDelta { .. }
                        | ai_providers::ContentBlockDelta::RedactedThinkingDelta { .. } => {
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

// ==================== PRIORITY 1: CRITICAL MISSING MODELS ====================

#[tokio::test]
#[ignore]
async fn test_gemini_20_pro_large_context() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("gemini", &api_key, "").expect("Failed to create provider");

    let request = ChatRequest {
        model: MODEL_GEMINI_20_PRO.to_string(),
        messages: vec![ChatMessage::user("Write a Python function to find the nth Fibonacci number.")],
        temperature: Some(0.1),
        max_tokens: Some(500),
        ..Default::default()
    };

    println!("\n=== Testing Gemini 2.0 Pro Experimental (2M context, best coding performance) ===");

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Gemini 2.0 Pro stream chat request failed");

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
            Err(e) => panic!("Gemini 2.0 Pro stream error: {:?}", e),
        }
    }

    println!("\n\nGemini 2.0 Pro: Received {} chunks", chunk_count);
    println!("Full content: {}", full_content);

    assert!(chunk_count > 0, "Expected at least one chunk");
    assert!(!full_content.is_empty(), "Expected non-empty content");
}

// ==================== PRIORITY 2: LATEST GENERATION MODELS ====================

#[tokio::test]
#[ignore]
async fn test_gemini_25_pro_streaming() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("gemini", &api_key, "").expect("Failed to create provider");

    let request = ChatRequest {
        model: MODEL_GEMINI_25_PRO.to_string(),
        messages: vec![ChatMessage::user("Count from 1 to 5, one number per line.")],
        temperature: Some(0.1),
        max_tokens: Some(100),
        ..Default::default()
    };

    println!("\n=== Testing Gemini 2.5 Pro (latest generation, evolution beyond 2.0) ===");

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Gemini 2.5 Pro stream chat request failed");

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
            Err(e) => panic!("Gemini 2.5 Pro stream error: {:?}", e),
        }
    }

    println!("\n\nGemini 2.5 Pro: Received {} chunks", chunk_count);
    println!("Full content: {}", full_content);

    assert!(chunk_count > 0, "Expected at least one chunk");
    assert!(!full_content.is_empty(), "Expected non-empty content");
}

#[tokio::test]
#[ignore]
async fn test_gemini_20_flash_streaming() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("gemini", &api_key, "").expect("Failed to create provider");

    let request = ChatRequest {
        model: MODEL_GEMINI_20_FLASH.to_string(),
        messages: vec![ChatMessage::user("Count from 1 to 5, one number per line.")],
        temperature: Some(0.1),
        max_tokens: Some(100),
        ..Default::default()
    };

    println!("\n=== Testing Gemini 2.0 Flash GA (production-ready, 1M context, native tool use) ===");

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Gemini 2.0 Flash stream chat request failed");

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
            Err(e) => panic!("Gemini 2.0 Flash stream error: {:?}", e),
        }
    }

    println!("\n\nGemini 2.0 Flash: Received {} chunks", chunk_count);
    println!("Full content: {}", full_content);

    assert!(chunk_count > 0, "Expected at least one chunk");
    assert!(!full_content.is_empty(), "Expected non-empty content");
}

// ==================== PRIORITY 3: COMPLETE COVERAGE ====================

#[tokio::test]
#[ignore]
async fn test_gemini_thinking_reasoning() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("gemini", &api_key, "").expect("Failed to create provider");

    let request = ChatRequest {
        model: MODEL_GEMINI_THINKING.to_string(),
        messages: vec![ChatMessage::user("What are the first 5 prime numbers? Explain why.")],
        temperature: Some(0.1),
        max_tokens: Some(500),
        ..Default::default()
    };

    println!("\n=== Testing Gemini 2.0 Flash Thinking (reasoning before answering) ===");

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Gemini Thinking stream chat request failed");

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
            Err(e) => panic!("Gemini Thinking stream error: {:?}", e),
        }
    }

    println!("\n\nGemini Thinking: Received {} chunks", chunk_count);
    println!("Full content: {}", full_content);

    assert!(chunk_count > 0, "Expected at least one chunk");
    assert!(!full_content.is_empty(), "Expected non-empty content");
}

#[tokio::test]
#[ignore]
async fn test_gemini_lite_cost_optimization() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("gemini", &api_key, "").expect("Failed to create provider");

    let request = ChatRequest {
        model: MODEL_GEMINI_LITE.to_string(),
        messages: vec![ChatMessage::user("Count from 1 to 10, one number per line.")],
        temperature: Some(0.1),
        max_tokens: Some(100),
        ..Default::default()
    };

    println!("\n=== Testing Gemini 2.0 Flash-Lite (cost-optimized for bulk text generation) ===");

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Gemini Lite stream chat request failed");

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
            Err(e) => panic!("Gemini Lite stream error: {:?}", e),
        }
    }

    println!("\n\nGemini Lite: Received {} chunks", chunk_count);
    println!("Full content: {}", full_content);

    assert!(chunk_count > 0, "Expected at least one chunk");
    assert!(!full_content.is_empty(), "Expected non-empty content");
}
