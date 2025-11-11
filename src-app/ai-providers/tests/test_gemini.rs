//! Gemini provider integration tests
//!
//! These tests require a valid Gemini API key set in the GEMINI_API_KEY environment variable.
//! Run with: GEMINI_API_KEY=your_key cargo test --test test_gemini -- --nocapture

use ai_providers::*;
use serde_json::json;

// Chat models (2.5 series has thinking mode)
const MODEL_GEMINI_25_PRO: &str = "models/gemini-2.5-pro";      // Latest with thinking
const MODEL_GEMINI_25_FLASH: &str = "models/gemini-2.5-flash";  // Fast with thinking
const MODEL_GEMINI_15_PRO: &str = "models/gemini-1.5-pro";      // Previous generation
const MODEL_GEMINI_15_FLASH: &str = "models/gemini-1.5-flash";  // Faster previous gen
const EMBEDDING_MODEL: &str = "models/text-embedding-004";

fn get_api_key() -> String {
    std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY environment variable must be set")
}

#[tokio::test]
#[ignore]
async fn test_gemini_simple_chat() {
    let api_key = get_api_key();
    let provider = GeminiProvider;

    let request = ChatRequest {
        model: MODEL_GEMINI_15_FLASH.to_string(),  // Standard Gemini 1.5
        messages: vec![
            ChatMessage::system("You are a helpful assistant."),
            ChatMessage::user("Say 'Hello, World!' and nothing else."),
        ],
        temperature: Some(0.7),
        max_tokens: Some(50),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, "", request)
        .await
        .expect("Chat request failed");

    assert!(!response.id.is_empty());
    assert!(!response.choices.is_empty());
    assert!(response.choices[0]
        .message
        .content
        .to_lowercase()
        .contains("hello"));
    assert!(response.usage.is_some());

    println!("Response: {:?}", response);
}

#[tokio::test]
#[ignore]
async fn test_gemini_streaming_chat() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = GeminiProvider;

    let request = ChatRequest {
        model: MODEL_GEMINI_15_FLASH.to_string(),  // Standard Gemini 1.5
        messages: vec![ChatMessage::user("Count from 1 to 5, one number per line.")],
        temperature: Some(0.1),
        max_tokens: Some(100),
        ..Default::default()
    };

    let mut stream = provider
        .stream_chat(&api_key, "", request)
        .await
        .expect("Stream chat request failed");

    let mut full_content = String::new();
    let mut chunk_count = 0;

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                full_content.push_str(&chunk.content);
                chunk_count += 1;
                print!("{}", chunk.content);
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
async fn test_gemini_tool_calling() {
    let api_key = get_api_key();
    let provider = GeminiProvider;

    // Define a weather tool
    let weather_tool = Tool::function(
        "get_weather",
        "Get the current weather in a location",
        json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "The city and state, e.g. San Francisco, CA"
                },
                "unit": {
                    "type": "string",
                    "enum": ["celsius", "fahrenheit"],
                    "description": "The temperature unit"
                }
            },
            "required": ["location"]
        }),
    );

    let request = ChatRequest {
        model: MODEL_GEMINI_15_FLASH.to_string(),  // Standard Gemini 1.5
        messages: vec![ChatMessage::user(
            "What's the weather like in San Francisco?",
        )],
        tools: vec![weather_tool],
        tool_choice: Some(ToolChoice::auto()),
        max_tokens: Some(1000),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, "", request)
        .await
        .expect("Tool calling request failed");

    println!("Response: {:?}", response);

    assert!(!response.choices.is_empty());
    // Gemini may include tool calls in the response
    let message = &response.choices[0].message;
    if !message.tool_calls.is_empty() {
        println!("Tool calls: {:?}", message.tool_calls);
    }
}

#[tokio::test]
#[ignore]
async fn test_gemini_tool_calling_required() {
    let api_key = get_api_key();
    let provider = GeminiProvider;

    let calculator_tool = Tool::function(
        "calculate",
        "Perform a mathematical calculation",
        json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "The mathematical expression to evaluate"
                }
            },
            "required": ["expression"]
        }),
    );

    let request = ChatRequest {
        model: MODEL_GEMINI_15_FLASH.to_string(),  // Standard Gemini 1.5
        messages: vec![ChatMessage::user("What is 25 * 4?")],
        tools: vec![calculator_tool],
        tool_choice: Some(ToolChoice::required()),
        max_tokens: Some(1000),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, "", request)
        .await
        .expect("Required tool calling failed");

    println!("Response: {:?}", response);
    assert!(!response.choices.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_gemini_tool_calling_specific() {
    let api_key = get_api_key();
    let provider = GeminiProvider;

    let search_tool = Tool::function(
        "web_search",
        "Search the web for information",
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                }
            },
            "required": ["query"]
        }),
    );

    let request = ChatRequest {
        model: MODEL_GEMINI_15_FLASH.to_string(),  // Standard Gemini 1.5
        messages: vec![ChatMessage::user("What is the capital of France?")],
        tools: vec![search_tool],
        tool_choice: Some(ToolChoice::function("web_search")),
        max_tokens: Some(1000),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, "", request)
        .await
        .expect("Specific tool calling failed");

    println!("Response: {:?}", response);
    assert!(!response.choices.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_gemini_multimodal_image() {
    let api_key = get_api_key();
    let provider = GeminiProvider;

    // Create a small test image (1x1 red pixel PNG)
    let image_data = base64::decode(
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==",
    )
    .expect("Failed to decode test image");

    let request = ChatRequest {
        model: MODEL_GEMINI_15_FLASH.to_string(),  // Standard Gemini 1.5
        messages: vec![ChatMessage::user("What color is this image? Be brief.")],
        attachments: vec![FileAttachment {
            filename: "test.png".to_string(),
            content: image_data,
            mime_type: "image/png".to_string(),
        }],
        max_tokens: Some(100),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, "", request)
        .await
        .expect("Multimodal request failed");

    println!("Response: {:?}", response);
    assert!(!response.choices.is_empty());
    assert!(!response.choices[0].message.content.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_gemini_multimodal_multiple_images() {
    let api_key = get_api_key();
    let provider = GeminiProvider;

    // Red pixel
    let red_image = base64::decode(
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==",
    )
    .expect("Failed to decode red image");

    // Blue pixel
    let blue_image = base64::decode(
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M/wHwAEBgIApD5fRAAAAABJRU5ErkJggg==",
    )
    .expect("Failed to decode blue image");

    let request = ChatRequest {
        model: MODEL_GEMINI_15_FLASH.to_string(),  // Standard Gemini 1.5
        messages: vec![ChatMessage::user("How many images do you see?")],
        attachments: vec![
            FileAttachment {
                filename: "red.png".to_string(),
                content: red_image,
                mime_type: "image/png".to_string(),
            },
            FileAttachment {
                filename: "blue.png".to_string(),
                content: blue_image,
                mime_type: "image/png".to_string(),
            },
        ],
        max_tokens: Some(100),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, "", request)
        .await
        .expect("Multiple images request failed");

    println!("Response: {:?}", response);
    assert!(!response.choices.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_gemini_embeddings_single() {
    let api_key = get_api_key();
    let provider = GeminiProvider;

    let request = EmbeddingsRequest {
        model: EMBEDDING_MODEL.to_string(),
        input: vec!["Hello, world!".to_string()],
    };

    let response = provider
        .embeddings(&api_key, "", request)
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
    let provider = GeminiProvider;

    let request = EmbeddingsRequest {
        model: EMBEDDING_MODEL.to_string(),
        input: vec![
            "The quick brown fox jumps over the lazy dog".to_string(),
            "Hello, world!".to_string(),
            "Machine learning is fascinating".to_string(),
        ],
    };

    let response = provider
        .embeddings(&api_key, "", request)
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
async fn test_gemini_multiple_messages() {
    let api_key = get_api_key();
    let provider = GeminiProvider;

    let request = ChatRequest {
        model: MODEL_GEMINI_15_FLASH.to_string(),  // Standard Gemini 1.5
        messages: vec![
            ChatMessage::system("You are a helpful math tutor."),
            ChatMessage::user("What is 2 + 2?"),
            ChatMessage::assistant("2 + 2 equals 4."),
            ChatMessage::user("What about 3 + 3?"),
        ],
        temperature: Some(0.1),
        max_tokens: Some(100),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, "", request)
        .await
        .expect("Multi-message request failed");

    assert!(!response.choices.is_empty());
    let content = &response.choices[0].message.content;
    assert!(content.contains("6") || content.contains("six"));

    println!("Response: {}", content);
}

#[tokio::test]
#[ignore]
async fn test_gemini_long_system_instruction() {
    let api_key = get_api_key();
    let provider = GeminiProvider;

    let long_system = "You are an expert chef with 30 years of experience. \
                      You specialize in French cuisine and have worked in \
                      Michelin-starred restaurants. You are patient, detailed, \
                      and always provide step-by-step instructions.";

    let request = ChatRequest {
        model: MODEL_GEMINI_15_FLASH.to_string(),  // Standard Gemini 1.5
        messages: vec![
            ChatMessage::system(long_system),
            ChatMessage::user("How do I make an omelette?"),
        ],
        max_tokens: Some(500),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, "", request)
        .await
        .expect("Long system instruction request failed");

    println!("Response: {:?}", response);
    assert!(!response.choices.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_gemini_temperature_variations() {
    let api_key = get_api_key();
    let provider = GeminiProvider;

    for temp in [0.0, 0.5, 1.0] {
        let request = ChatRequest {
            model: MODEL_GEMINI_15_FLASH.to_string(),  // Standard Gemini 1.5
            messages: vec![ChatMessage::user("Say hello in a creative way.")],
            temperature: Some(temp),
            max_tokens: Some(50),
            ..Default::default()
        };

        let response = provider
            .chat(&api_key, "", request.clone())
            .await
            .expect("Temperature test failed");

        println!(
            "Temperature {}: {}",
            temp, response.choices[0].message.content
        );
    }
}

#[tokio::test]
#[ignore]
async fn test_gemini_max_tokens_limit() {
    let api_key = get_api_key();
    let provider = GeminiProvider;

    let request = ChatRequest {
        model: MODEL_GEMINI_15_FLASH.to_string(),  // Standard Gemini 1.5
        messages: vec![ChatMessage::user("Write a long story about a cat.")],
        max_tokens: Some(10),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, "", request)
        .await
        .expect("Max tokens test failed");

    assert!(!response.choices.is_empty());
    // Gemini uses different finish reason strings
    println!("Finish reason: {:?}", response.choices[0].finish_reason);
    println!("Response: {:?}", response);
}

#[tokio::test]
#[ignore]
async fn test_gemini_error_invalid_model() {
    let api_key = get_api_key();
    let provider = GeminiProvider;

    let request = ChatRequest {
        model: "invalid-model-name-12345".to_string(),
        messages: vec![ChatMessage::user("Hello")],
        max_tokens: Some(100),
        ..Default::default()
    };

    let result = provider.chat(&api_key, "", request).await;

    assert!(result.is_err());
    println!("Expected error: {:?}", result.unwrap_err());
}

#[tokio::test]
#[ignore]
async fn test_gemini_error_invalid_api_key() {
    let provider = GeminiProvider;

    let request = ChatRequest {
        model: MODEL_GEMINI_15_FLASH.to_string(),  // Standard Gemini 1.5
        messages: vec![ChatMessage::user("Hello")],
        max_tokens: Some(100),
        ..Default::default()
    };

    let result = provider
        .chat("invalid_api_key_12345", "", request)
        .await;

    assert!(result.is_err());
    println!("Expected error: {:?}", result.unwrap_err());
}

#[tokio::test]
#[ignore]
async fn test_gemini_top_p_parameter() {
    let api_key = get_api_key();
    let provider = GeminiProvider;

    let request = ChatRequest {
        model: MODEL_GEMINI_15_FLASH.to_string(),  // Standard Gemini 1.5
        messages: vec![ChatMessage::user("Complete this: The sky is")],
        top_p: Some(0.1), // Very focused sampling
        max_tokens: Some(20),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, "", request)
        .await
        .expect("Top-p test failed");

    println!(
        "Response with top_p=0.1: {}",
        response.choices[0].message.content
    );
    assert!(!response.choices.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_gemini_empty_content_with_tools() {
    let api_key = get_api_key();
    let provider = GeminiProvider;

    let tool = Tool::function(
        "get_time",
        "Get the current time",
        json!({
            "type": "object",
            "properties": {}
        }),
    );

    let request = ChatRequest {
        model: MODEL_GEMINI_15_FLASH.to_string(),  // Standard Gemini 1.5
        messages: vec![ChatMessage::user("What time is it?")],
        tools: vec![tool],
        tool_choice: Some(ToolChoice::required()),
        max_tokens: Some(500),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, "", request)
        .await
        .expect("Empty content with tools failed");

    println!("Response: {:?}", response);
    assert!(!response.choices.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_gemini_streaming_with_tools() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = GeminiProvider;

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
        model: MODEL_GEMINI_15_FLASH.to_string(),  // Standard Gemini 1.5
        messages: vec![ChatMessage::user("What's the weather in Tokyo?")],
        tools: vec![tool],
        tool_choice: Some(ToolChoice::auto()),
        max_tokens: Some(500),
        ..Default::default()
    };

    let mut stream = provider
        .stream_chat(&api_key, "", request)
        .await
        .expect("Stream with tools request failed");

    let mut full_content = String::new();
    let mut chunk_count = 0;

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                full_content.push_str(&chunk.content);
                chunk_count += 1;
                print!("{}", chunk.content);
            }
            Err(e) => panic!("Stream error: {:?}", e),
        }
    }

    println!("\n\nReceived {} chunks", chunk_count);
    println!("Full content: {}", full_content);

    assert!(chunk_count > 0);
}

#[tokio::test]
#[ignore]
async fn test_gemini_thinking_mode_basic() {
    let api_key = get_api_key();
    let provider = GeminiProvider;

    let request = ChatRequest {
        model: MODEL_GEMINI_25_FLASH.to_string(),  // Gemini 2.5 with thinking
        messages: vec![ChatMessage::user("What is 234 * 567? Show your thinking.")],
        max_tokens: Some(5000),
        thinking: Some(ThinkingConfig::with_budget(2048)),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, "", request)
        .await
        .expect("Thinking mode request failed");

    println!("Response: {:?}", response);
    assert!(!response.choices.is_empty());
    
    // Check if thinking was included
    if let Some(ref thinking) = response.choices[0].message.thinking {
        println!("Thought summary: {}", thinking);
        assert!(!thinking.is_empty());
    }
    
    assert!(response.choices[0].message.content.contains("132678"));
}

#[tokio::test]
#[ignore]
async fn test_gemini_thinking_mode_dynamic() {
    let api_key = get_api_key();
    let provider = GeminiProvider;

    let request = ChatRequest {
        model: MODEL_GEMINI_25_PRO.to_string(),  // Gemini 2.5 Pro with thinking
        messages: vec![ChatMessage::user("Explain the concept of recursion with examples. Think carefully about this.")],
        max_tokens: Some(8000),
        thinking: Some(ThinkingConfig::with_effort(ThinkingEffort::Dynamic)),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, "", request)
        .await
        .expect("Dynamic thinking failed");

    println!("Response: {:?}", response);
    assert!(!response.choices.is_empty());
    
    // Dynamic thinking should produce thoughts based on complexity
    if let Some(ref thinking) = response.choices[0].message.thinking {
        println!("Dynamic thinking (first 200 chars): {}...", &thinking.chars().take(200).collect::<String>());
    }
}

#[tokio::test]
#[ignore]
async fn test_gemini_thinking_mode_high_budget() {
    let api_key = get_api_key();
    let provider = GeminiProvider;

    let request = ChatRequest {
        model: MODEL_GEMINI_25_FLASH.to_string(),  // Gemini 2.5 with thinking
        messages: vec![ChatMessage::user("Write a short algorithm to find all palindromes in a string. Explain your reasoning.")],
        max_tokens: Some(10000),
        thinking: Some(ThinkingConfig::with_budget(8192)),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, "", request)
        .await
        .expect("High budget thinking failed");

    println!("Response: {:?}", response);
    assert!(!response.choices.is_empty());
    
    // Higher budget should allow more extensive thinking
    if let Some(ref thinking) = response.choices[0].message.thinking {
        println!("Extensive thinking (length): {} chars", thinking.len());
    }
}

#[tokio::test]
#[ignore]
async fn test_gemini_thinking_mode_streaming() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = GeminiProvider;

    let request = ChatRequest {
        model: MODEL_GEMINI_25_FLASH.to_string(),  // Gemini 2.5 with thinking
        messages: vec![ChatMessage::user("Count from 1 to 10 and explain why each number is special.")],
        max_tokens: Some(6000),
        thinking: Some(ThinkingConfig::with_budget(3000)),
        ..Default::default()
    };

    let mut stream = provider
        .stream_chat(&api_key, "", request)
        .await
        .expect("Thinking streaming failed");

    let mut full_content = String::new();
    let mut full_thinking = String::new();
    let mut chunk_count = 0;

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                full_content.push_str(&chunk.content);
                if let Some(thinking) = chunk.thinking {
                    full_thinking.push_str(&thinking);
                    print!("[THINKING] {}", thinking);
                }
                chunk_count += 1;
                print!("{}", chunk.content);
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
