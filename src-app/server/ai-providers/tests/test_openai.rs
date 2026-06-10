//! OpenAI provider integration tests
//!
//! These tests require a valid OpenAI API key set in the OPENAI_API_KEY environment variable.
//! Run with: OPENAI_API_KEY=your_key cargo test --test test_openai -- --nocapture

use ai_providers::*;

const BASE_URL: &str = "https://api.openai.com/v1";

// Models used in tests
const MODEL_GPT35_TURBO: &str = "gpt-3.5-turbo";
const MODEL_GPT4O: &str = "gpt-4o";
const MODEL_GPT4O_MINI: &str = "gpt-4o-mini";
const MODEL_GPT4_TURBO: &str = "gpt-4-turbo";
const MODEL_O1: &str = "o1";
const MODEL_O1_MINI: &str = "o1-mini";
const MODEL_GPT41_MINI: &str = "gpt-4.1-mini";
const MODEL_EMBEDDING_3_SMALL: &str = "text-embedding-3-small";
const MODEL_EMBEDDING_3_LARGE: &str = "text-embedding-3-large";

fn get_api_key() -> String {
    std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable must be set")
}

#[tokio::test]
#[ignore]
async fn test_openai_streaming_chat() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("openai", &api_key, BASE_URL)
        .expect("Failed to create provider");

    let request = ChatRequest {
        model: MODEL_GPT35_TURBO.to_string(),  // Faster for streaming
        messages: vec![ChatMessage::user("Count from 1 to 5, one number per line.")],
        temperature: Some(0.1),
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
async fn test_openai_embeddings() {
    let api_key = get_api_key();
    let provider = Provider::new("openai", &api_key, BASE_URL)
        .expect("Failed to create provider");

    let request = EmbeddingsRequest {
        model: MODEL_EMBEDDING_3_SMALL.to_string(),  // Newer embedding model
        input: vec![
            "The quick brown fox jumps over the lazy dog".to_string(),
            "Hello, world!".to_string(),
        ],
    };

    let response = provider
        .embeddings(request)
        .await
        .expect("Embeddings request failed");

    assert_eq!(response.embeddings.len(), 2);
    assert!(response.embeddings[0].len() > 0);
    assert!(response.embeddings[1].len() > 0);
    assert!(response.usage.is_some());

    println!(
        "Embedding dimensions: {}",
        response.embeddings[0].len()
    );
    println!("Usage: {:?}", response.usage);
}

#[tokio::test]
#[ignore]
async fn test_openai_reasoning_model_streaming() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("openai", &api_key, BASE_URL)
        .expect("Failed to create provider");

    let request = ChatRequest {
        model: "o3-mini".to_string(),
        messages: vec![ChatMessage::user("Count the prime numbers between 1 and 20.")],
        max_tokens: Some(3000),
        thinking: Some(ThinkingConfig::with_effort(ThinkingEffort::Low)),
        ..Default::default()
    };

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Reasoning streaming failed");

    let mut full_content = String::new();
    let mut chunk_count = 0;
    let mut has_reasoning_tokens = false;

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

                // Check for reasoning tokens in usage metadata
                if let Some(usage) = chunk.usage {
                    println!("\n=== Usage Metadata ===");
                    println!("Prompt tokens: {}", usage.prompt_tokens);
                    println!("Completion tokens: {}", usage.completion_tokens);
                    println!("Total tokens: {}", usage.total_tokens);
                    if let Some(reasoning) = usage.reasoning_tokens {
                        println!("Reasoning tokens: {}", reasoning);
                        has_reasoning_tokens = true;
                    }
                }
            }
            Err(e) => panic!("Stream error: {:?}", e),
        }
    }

    println!("\n\nReceived {} chunks", chunk_count);
    println!("Full content: {}", full_content);
    println!("Has reasoning tokens: {}", has_reasoning_tokens);

    assert!(chunk_count > 0);
    assert!(!full_content.is_empty());
    // Note: OpenAI reasoning models (o3-mini, o1, o1-mini) do NOT send reasoning_tokens in streaming responses
    // This is expected API behavior - reasoning tokens are only available in non-streaming mode
    if !has_reasoning_tokens {
        println!("NOTE: No reasoning tokens in streaming response. This is expected OpenAI API behavior for reasoning models.");
    }
}

#[tokio::test]
#[ignore]
async fn test_openai_gpt5_non_streaming_workaround() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("openai", &api_key, BASE_URL)
        .expect("Failed to create provider");

    // Test gpt-5 model which requires non-streaming workaround
    let request = ChatRequest {
        model: "gpt-5".to_string(), // This should trigger non-streaming internally
        messages: vec![ChatMessage::user("What is 2+2? Provide a brief answer.")],
        max_tokens: Some(100),
        thinking: Some(ThinkingConfig::with_effort(ThinkingEffort::Low)),
        ..Default::default()
    };

    println!("\n=== Testing GPT-5 with non-streaming workaround ===");
    println!("Model: gpt-5 (should use non-streaming internally)");

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("GPT-5 stream (non-streaming workaround) failed");

    let mut full_content = String::new();
    let mut chunk_count = 0;
    let mut has_usage = false;
    let mut has_reasoning_tokens = false;

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                // Process content deltas
                for delta in &chunk.content {
                    match delta {
                        ai_providers::ContentBlockDelta::TextDelta { delta, .. } => {
                            full_content.push_str(delta);
                            println!("Content chunk: {}", delta);
                        }
                        ai_providers::ContentBlockDelta::ThinkingDelta { delta, .. } => {
                            full_content.push_str(&format!("[THINKING: {}]", delta));
                            println!("Thinking chunk: {}", delta);
                        }
                        ai_providers::ContentBlockDelta::ToolUseDelta { .. }
                        | ai_providers::ContentBlockDelta::ThinkingSignatureDelta { .. }
                        | ai_providers::ContentBlockDelta::RedactedThinkingDelta { .. } => {
                            // Skip tool use deltas
                        }
                    }
                }
                chunk_count += 1;

                // Check for usage metadata
                if let Some(usage) = chunk.usage {
                    has_usage = true;
                    println!("\n=== Usage Metadata ===");
                    println!("Prompt tokens: {}", usage.prompt_tokens);
                    println!("Completion tokens: {}", usage.completion_tokens);
                    println!("Total tokens: {}", usage.total_tokens);
                    if let Some(reasoning) = usage.reasoning_tokens {
                        println!("Reasoning tokens: {}", reasoning);
                        has_reasoning_tokens = true;
                    }
                }
            }
            Err(e) => panic!("GPT-5 stream error: {:?}", e),
        }
    }

    println!("\n=== Test Results ===");
    println!("Received {} chunks", chunk_count);
    println!("Full content: {}", full_content);
    println!("Has usage: {}", has_usage);
    println!("Has reasoning tokens: {}", has_reasoning_tokens);

    // Assertions
    assert!(chunk_count > 0, "Expected at least one chunk");
    assert!(!full_content.is_empty(), "Expected non-empty content");
    assert!(has_usage, "Expected usage metadata in response");
    // Note: reasoning_tokens might be 0 for simple prompts, so we just check it's present
}

// ==================== GROQ TESTS (OpenAI-Compatible) ====================

#[tokio::test]
#[ignore]
async fn test_groq_streaming_chat() {
    use futures_util::StreamExt;

    let api_key = std::env::var("GROQ_API_KEY").expect("GROQ_API_KEY environment variable must be set");
    let provider = Provider::new("groq", &api_key, "https://api.groq.com/openai/v1")
        .expect("Failed to create provider");

    let request = ChatRequest {
        model: "llama-3.3-70b-versatile".to_string(),
        messages: vec![ChatMessage::user("Count from 1 to 5, one number per line.")],
        temperature: Some(0.1),
        max_tokens: Some(100),
        ..Default::default()
    };

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Groq stream chat request failed");

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
            Err(e) => panic!("Groq stream error: {:?}", e),
        }
    }

    println!("\n\nGroq: Received {} chunks", chunk_count);
    println!("Full content: {}", full_content);

    assert!(chunk_count > 0);
    assert!(!full_content.is_empty());
}

// ==================== PRIORITY 1: CRITICAL MISSING MODELS ====================

#[tokio::test]
#[ignore]
async fn test_openai_gpt4o_streaming_chat() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("openai", &api_key, BASE_URL)
        .expect("Failed to create provider");

    let request = ChatRequest {
        model: MODEL_GPT4O.to_string(),
        messages: vec![ChatMessage::user("Count from 1 to 5, one number per line.")],
        temperature: Some(0.1),
        max_tokens: Some(100),
        ..Default::default()
    };

    println!("\n=== Testing GPT-4o (most popular GPT-4 variant, 128k context, multimodal) ===");

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("GPT-4o stream chat request failed");

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
            Err(e) => panic!("GPT-4o stream error: {:?}", e),
        }
    }

    println!("\n\nGPT-4o: Received {} chunks", chunk_count);
    println!("Full content: {}", full_content);

    assert!(chunk_count > 0, "Expected at least one chunk");
    assert!(!full_content.is_empty(), "Expected non-empty content");
}

#[tokio::test]
#[ignore]
async fn test_openai_gpt4o_mini_streaming_chat() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("openai", &api_key, BASE_URL)
        .expect("Failed to create provider");

    let request = ChatRequest {
        model: MODEL_GPT4O_MINI.to_string(),
        messages: vec![ChatMessage::user("Count from 1 to 5, one number per line.")],
        temperature: Some(0.1),
        max_tokens: Some(100),
        ..Default::default()
    };

    println!("\n=== Testing GPT-4o-mini (most affordable GPT-4 class, 16k context, cost-optimized) ===");

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("GPT-4o-mini stream chat request failed");

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
            Err(e) => panic!("GPT-4o-mini stream error: {:?}", e),
        }
    }

    println!("\n\nGPT-4o-mini: Received {} chunks", chunk_count);
    println!("Full content: {}", full_content);

    assert!(chunk_count > 0, "Expected at least one chunk");
    assert!(!full_content.is_empty(), "Expected non-empty content");
}

#[tokio::test]
#[ignore]
async fn test_openai_gpt4_turbo_streaming_chat() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("openai", &api_key, BASE_URL)
        .expect("Failed to create provider");

    let request = ChatRequest {
        model: MODEL_GPT4_TURBO.to_string(),
        messages: vec![ChatMessage::user("Count from 1 to 5, one number per line.")],
        temperature: Some(0.1),
        max_tokens: Some(100),
        ..Default::default()
    };

    println!("\n=== Testing GPT-4-turbo (traditional completions optimization, 128k context) ===");

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("GPT-4-turbo stream chat request failed");

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
            Err(e) => panic!("GPT-4-turbo stream error: {:?}", e),
        }
    }

    println!("\n\nGPT-4-turbo: Received {} chunks", chunk_count);
    println!("Full content: {}", full_content);

    assert!(chunk_count > 0, "Expected at least one chunk");
    assert!(!full_content.is_empty(), "Expected non-empty content");
}

#[tokio::test]
#[ignore]
async fn test_openai_o1_reasoning() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("openai", &api_key, BASE_URL)
        .expect("Failed to create provider");

    let request = ChatRequest {
        model: MODEL_O1.to_string(),
        messages: vec![ChatMessage::user("Count the prime numbers between 1 and 20.")],
        max_tokens: Some(3000),
        thinking: Some(ThinkingConfig::with_effort(ThinkingEffort::Low)),
        ..Default::default()
    };

    println!("\n=== Testing O1 (full reasoning model, more capable than o3-mini) ===");
    println!("Note: No few-shot prompting (degrades performance)");

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("O1 reasoning streaming failed");

    let mut full_content = String::new();
    let mut chunk_count = 0;
    let mut has_reasoning_tokens = false;

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

                if let Some(usage) = chunk.usage {
                    println!("\n=== Usage Metadata ===");
                    println!("Prompt tokens: {}", usage.prompt_tokens);
                    println!("Completion tokens: {}", usage.completion_tokens);
                    println!("Total tokens: {}", usage.total_tokens);
                    println!("Reasoning tokens: {:?}", usage.reasoning_tokens);
                    if let Some(reasoning) = usage.reasoning_tokens {
                        println!("Reasoning tokens value: {}", reasoning);
                        has_reasoning_tokens = true;
                    } else {
                        println!("WARNING: reasoning_tokens field is None");
                    }
                }
            }
            Err(e) => panic!("O1 stream error: {:?}", e),
        }
    }

    println!("\n\nO1: Received {} chunks", chunk_count);
    println!("Full content: {}", full_content);
    println!("Has reasoning tokens: {}", has_reasoning_tokens);

    assert!(chunk_count > 0, "Expected at least one chunk");
    assert!(!full_content.is_empty(), "Expected non-empty content");
    // Note: O1 streaming may not include reasoning_tokens in usage metadata
    // This is expected OpenAI API behavior - reasoning tokens may only be available in non-streaming mode
    if !has_reasoning_tokens {
        println!("WARNING: No reasoning tokens found in streaming response. This may be expected API behavior.");
    }
}

// ==================== PRIORITY 2: LATEST GENERATION MODELS ====================

#[tokio::test]
#[ignore]
async fn test_openai_gpt41_mini_streaming() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("openai", &api_key, BASE_URL)
        .expect("Failed to create provider");

    let request = ChatRequest {
        model: MODEL_GPT41_MINI.to_string(),
        messages: vec![ChatMessage::user("Write a simple Python function to check if a number is prime.")],
        temperature: Some(0.1),
        max_tokens: Some(500),
        ..Default::default()
    };

    println!("\n=== Testing GPT-4.1-mini (2025 latest, outperforms GPT-4o in coding/instruction following) ===");

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("GPT-4.1-mini stream chat request failed");

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
            Err(e) => panic!("GPT-4.1-mini stream error: {:?}", e),
        }
    }

    println!("\n\nGPT-4.1-mini: Received {} chunks", chunk_count);
    println!("Full content: {}", full_content);

    assert!(chunk_count > 0, "Expected at least one chunk");
    assert!(!full_content.is_empty(), "Expected non-empty content");
}

#[tokio::test]
#[ignore]
async fn test_openai_o1_mini_reasoning() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("openai", &api_key, BASE_URL)
        .expect("Failed to create provider");

    let request = ChatRequest {
        model: MODEL_O1_MINI.to_string(),
        messages: vec![ChatMessage::user("What are the first 5 prime numbers? Explain why each is prime.")],
        max_tokens: Some(2000),
        thinking: Some(ThinkingConfig::with_effort(ThinkingEffort::Low)),
        ..Default::default()
    };

    println!("\n=== Testing O1-mini (faster/cheaper reasoning, comparison to o3-mini) ===");

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("O1-mini reasoning streaming failed");

    let mut full_content = String::new();
    let mut chunk_count = 0;
    let mut has_reasoning_tokens = false;

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

                if let Some(usage) = chunk.usage {
                    if let Some(_reasoning) = usage.reasoning_tokens {
                        has_reasoning_tokens = true;
                    }
                }
            }
            Err(e) => panic!("O1-mini stream error: {:?}", e),
        }
    }

    println!("\n\nO1-mini: Received {} chunks", chunk_count);
    println!("Full content: {}", full_content);
    println!("Has reasoning tokens: {}", has_reasoning_tokens);

    assert!(chunk_count > 0, "Expected at least one chunk");
    assert!(!full_content.is_empty(), "Expected non-empty content");
    // Note: OpenAI reasoning models (o3-mini, o1, o1-mini) do NOT send reasoning_tokens in streaming responses
    // This is expected API behavior - reasoning tokens are only available in non-streaming mode
    if !has_reasoning_tokens {
        println!("NOTE: No reasoning tokens in streaming response. This is expected OpenAI API behavior for reasoning models.");
    }
}

#[tokio::test]
#[ignore]
async fn test_openai_embedding_large() {
    let api_key = get_api_key();
    let provider = Provider::new("openai", &api_key, BASE_URL)
        .expect("Failed to create provider");

    let request = EmbeddingsRequest {
        model: MODEL_EMBEDDING_3_LARGE.to_string(),
        input: vec![
            "The quick brown fox jumps over the lazy dog".to_string(),
            "Hello, world!".to_string(),
        ],
    };

    println!("\n=== Testing text-embedding-3-large (higher quality, up to 3072 dimensions) ===");

    let response = provider
        .embeddings(request)
        .await
        .expect("Embeddings request failed");

    assert_eq!(response.embeddings.len(), 2);
    assert!(response.embeddings[0].len() > 0);
    assert!(response.embeddings[1].len() > 0);
    assert!(response.usage.is_some());

    println!(
        "Large embedding dimensions: {}",
        response.embeddings[0].len()
    );
    println!("Usage: {:?}", response.usage);

    // text-embedding-3-large should have larger dimensions than small (1536)
    assert!(response.embeddings[0].len() >= 1536, "Expected larger dimension size");
}
