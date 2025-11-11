//! Anthropic provider integration tests
//!
//! These tests require a valid Anthropic API key set in the ANTHROPIC_API_KEY environment variable.
//! Run with: ANTHROPIC_API_KEY=your_key cargo test --test test_anthropic -- --nocapture

use ai_providers::*;
use serde_json::json;

const BASE_URL: &str = "https://api.anthropic.com/v1";
// Chat models
const MODEL_CLAUDE_4: &str = "claude-sonnet-4-5";           // Latest with extended thinking
const MODEL_CLAUDE_35_SONNET: &str = "claude-3-5-sonnet-20241022";  // Previous generation
const MODEL_CLAUDE_3_OPUS: &str = "claude-3-opus-20240229";     // Most capable Claude 3
const MODEL_CLAUDE_3_HAIKU: &str = "claude-3-haiku-20240307";   // Fastest Claude 3

fn get_api_key() -> String {
    std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY environment variable must be set")
}

#[tokio::test]
#[ignore]
async fn test_anthropic_simple_chat() {
    let api_key = get_api_key();
    let provider = AnthropicProvider;

    let request = ChatRequest {
        model: MODEL_CLAUDE_35_SONNET.to_string(),  // Standard Claude 3.5
        messages: vec![
            ChatMessage::system("You are a helpful assistant."),
            ChatMessage::user("Say 'Hello, World!' and nothing else."),
        ],
        temperature: Some(0.7),
        max_tokens: Some(50),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, BASE_URL, request)
        .await
        .expect("Chat request failed");

    assert!(!response.id.is_empty());
    // Model may vary
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
async fn test_anthropic_streaming_chat() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = AnthropicProvider;

    let request = ChatRequest {
        model: MODEL_CLAUDE_35_SONNET.to_string(),  // Standard Claude 3.5
        messages: vec![ChatMessage::user("Count from 1 to 5, one number per line.")],
        temperature: Some(0.1),
        max_tokens: Some(100),
        ..Default::default()
    };

    let mut stream = provider
        .stream_chat(&api_key, BASE_URL, request)
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
async fn test_anthropic_tool_calling() {
    let api_key = get_api_key();
    let provider = AnthropicProvider;

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
        model: MODEL_CLAUDE_35_SONNET.to_string(),  // Standard Claude 3.5
        messages: vec![ChatMessage::user(
            "What's the weather like in San Francisco?",
        )],
        tools: vec![weather_tool],
        tool_choice: Some(ToolChoice::auto()),
        max_tokens: Some(1000),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, BASE_URL, request)
        .await
        .expect("Tool calling request failed");

    println!("Response: {:?}", response);

    // Note: Anthropic tool calls need to be extracted from response content
    // This is a TODO in the implementation
    assert!(!response.choices.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_anthropic_tool_calling_required() {
    let api_key = get_api_key();
    let provider = AnthropicProvider;

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
        model: MODEL_CLAUDE_35_SONNET.to_string(),  // Standard Claude 3.5
        messages: vec![ChatMessage::user("What is 25 * 4?")],
        tools: vec![calculator_tool],
        tool_choice: Some(ToolChoice::required()),
        max_tokens: Some(1000),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, BASE_URL, request)
        .await
        .expect("Required tool calling failed");

    println!("Response: {:?}", response);
    assert!(!response.choices.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_anthropic_tool_calling_specific() {
    let api_key = get_api_key();
    let provider = AnthropicProvider;

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
        model: MODEL_CLAUDE_35_SONNET.to_string(),  // Standard Claude 3.5
        messages: vec![ChatMessage::user("What is the capital of France?")],
        tools: vec![search_tool],
        tool_choice: Some(ToolChoice::function("web_search")),
        max_tokens: Some(1000),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, BASE_URL, request)
        .await
        .expect("Specific tool calling failed");

    println!("Response: {:?}", response);
    assert!(!response.choices.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_anthropic_multimodal_image() {
    let api_key = get_api_key();
    let provider = AnthropicProvider;

    // Create a small test image (1x1 red pixel PNG)
    let image_data = base64::decode(
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==",
    )
    .expect("Failed to decode test image");

    let request = ChatRequest {
        model: MODEL_CLAUDE_35_SONNET.to_string(),  // Standard Claude 3.5
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
        .chat(&api_key, BASE_URL, request)
        .await
        .expect("Multimodal request failed");

    println!("Response: {:?}", response);
    assert!(!response.choices.is_empty());
    assert!(!response.choices[0].message.content.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_anthropic_multimodal_multiple_images() {
    let api_key = get_api_key();
    let provider = AnthropicProvider;

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
        model: MODEL_CLAUDE_35_SONNET.to_string(),  // Standard Claude 3.5
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
        .chat(&api_key, BASE_URL, request)
        .await
        .expect("Multiple images request failed");

    println!("Response: {:?}", response);
    assert!(!response.choices.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_anthropic_no_embeddings() {
    let api_key = get_api_key();
    let provider = AnthropicProvider;

    let request = EmbeddingsRequest {
        model: MODEL_CLAUDE_35_SONNET.to_string(),  // Standard Claude 3.5
        input: vec!["Hello, world!".to_string()],
    };

    let result = provider.embeddings(&api_key, BASE_URL, request).await;

    // Anthropic doesn't support embeddings
    assert!(result.is_err());
    println!("Expected error: {:?}", result.unwrap_err());
}

#[tokio::test]
#[ignore]
async fn test_anthropic_multiple_messages() {
    let api_key = get_api_key();
    let provider = AnthropicProvider;

    let request = ChatRequest {
        model: MODEL_CLAUDE_35_SONNET.to_string(),  // Standard Claude 3.5
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
        .chat(&api_key, BASE_URL, request)
        .await
        .expect("Multi-message request failed");

    assert!(!response.choices.is_empty());
    let content = &response.choices[0].message.content;
    assert!(content.contains("6") || content.contains("six"));

    println!("Response: {}", content);
}

#[tokio::test]
#[ignore]
async fn test_anthropic_long_system_message() {
    let api_key = get_api_key();
    let provider = AnthropicProvider;

    let long_system = "You are an expert chef with 30 years of experience. \
                      You specialize in French cuisine and have worked in \
                      Michelin-starred restaurants. You are patient, detailed, \
                      and always provide step-by-step instructions.";

    let request = ChatRequest {
        model: MODEL_CLAUDE_35_SONNET.to_string(),  // Standard Claude 3.5
        messages: vec![
            ChatMessage::system(long_system),
            ChatMessage::user("How do I make an omelette?"),
        ],
        max_tokens: Some(500),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, BASE_URL, request)
        .await
        .expect("Long system message request failed");

    println!("Response: {:?}", response);
    assert!(!response.choices.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_anthropic_temperature_variations() {
    let api_key = get_api_key();
    let provider = AnthropicProvider;

    for temp in [0.0, 0.5, 1.0] {
        let request = ChatRequest {
            model: MODEL_CLAUDE_35_SONNET.to_string(),  // Standard Claude 3.5
            messages: vec![ChatMessage::user("Say hello in a creative way.")],
            temperature: Some(temp),
            max_tokens: Some(50),
            ..Default::default()
        };

        let response = provider
            .chat(&api_key, BASE_URL, request.clone())
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
async fn test_anthropic_max_tokens_limit() {
    let api_key = get_api_key();
    let provider = AnthropicProvider;

    let request = ChatRequest {
        model: MODEL_CLAUDE_35_SONNET.to_string(),  // Standard Claude 3.5
        messages: vec![ChatMessage::user("Write a long story about a cat.")],
        max_tokens: Some(10),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, BASE_URL, request)
        .await
        .expect("Max tokens test failed");

    assert!(!response.choices.is_empty());
    assert_eq!(response.choices[0].finish_reason, "max_tokens");

    println!("Response: {:?}", response);
}

#[tokio::test]
#[ignore]
async fn test_anthropic_error_invalid_model() {
    let api_key = get_api_key();
    let provider = AnthropicProvider;

    let request = ChatRequest {
        model: "invalid-model-name-12345".to_string(),
        messages: vec![ChatMessage::user("Hello")],
        max_tokens: Some(100),
        ..Default::default()
    };

    let result = provider.chat(&api_key, BASE_URL, request).await;

    assert!(result.is_err());
    println!("Expected error: {:?}", result.unwrap_err());
}

#[tokio::test]
#[ignore]
async fn test_anthropic_error_invalid_api_key() {
    let provider = AnthropicProvider;

    let request = ChatRequest {
        model: MODEL_CLAUDE_35_SONNET.to_string(),  // Standard Claude 3.5
        messages: vec![ChatMessage::user("Hello")],
        max_tokens: Some(100),
        ..Default::default()
    };

    let result = provider
        .chat("invalid_api_key_12345", BASE_URL, request)
        .await;

    assert!(result.is_err());
    println!("Expected error: {:?}", result.unwrap_err());
}

#[tokio::test]
#[ignore]
async fn test_anthropic_top_p_parameter() {
    let api_key = get_api_key();
    let provider = AnthropicProvider;

    let request = ChatRequest {
        model: MODEL_CLAUDE_35_SONNET.to_string(),  // Standard Claude 3.5
        messages: vec![ChatMessage::user("Complete this: The sky is")],
        top_p: Some(0.1), // Very focused sampling
        max_tokens: Some(20),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, BASE_URL, request)
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
async fn test_anthropic_empty_content_with_tools() {
    let api_key = get_api_key();
    let provider = AnthropicProvider;

    let tool = Tool::function(
        "get_time",
        "Get the current time",
        json!({
            "type": "object",
            "properties": {}
        }),
    );

    let request = ChatRequest {
        model: MODEL_CLAUDE_35_SONNET.to_string(),  // Standard Claude 3.5
        messages: vec![ChatMessage::user("What time is it?")],
        tools: vec![tool],
        tool_choice: Some(ToolChoice::required()),
        max_tokens: Some(500),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, BASE_URL, request)
        .await
        .expect("Empty content with tools failed");

    println!("Response: {:?}", response);
    assert!(!response.choices.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_anthropic_extended_thinking_basic() {
    let api_key = get_api_key();
    let provider = AnthropicProvider;

    let request = ChatRequest {
        model: MODEL_CLAUDE_4.to_string(),  // Claude 4 with extended thinking
        messages: vec![ChatMessage::user("What is 456 * 789? Explain your thought process.")],
        max_tokens: Some(16000),
        thinking: Some(ThinkingConfig::with_budget(10000)),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, BASE_URL, request)
        .await
        .expect("Extended thinking request failed");

    println!("Response: {:?}", response);
    assert!(!response.choices.is_empty());
    
    // Check if thinking was included
    if let Some(ref thinking) = response.choices[0].message.thinking {
        println!("Thinking process: {}", thinking);
        assert!(!thinking.is_empty());
    }
    
    assert!(response.choices[0].message.content.contains("359784"));
}

#[tokio::test]
#[ignore]
async fn test_anthropic_extended_thinking_large_budget() {
    let api_key = get_api_key();
    let provider = AnthropicProvider;

    let request = ChatRequest {
        model: MODEL_CLAUDE_4.to_string(),  // Claude 4 with extended thinking
        messages: vec![ChatMessage::user("Analyze the pros and cons of using recursion vs iteration in programming. Think deeply about this.")],
        max_tokens: Some(20000),
        thinking: Some(ThinkingConfig::with_budget(15000)),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, BASE_URL, request)
        .await
        .expect("Large budget thinking failed");

    println!("Response: {:?}", response);
    assert!(!response.choices.is_empty());
    
    // Should have thinking content
    if let Some(ref thinking) = response.choices[0].message.thinking {
        println!("Extended thinking (first 200 chars): {}...", &thinking.chars().take(200).collect::<String>());
    }
}

#[tokio::test]
#[ignore]
async fn test_anthropic_extended_thinking_streaming() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = AnthropicProvider;

    let request = ChatRequest {
        model: MODEL_CLAUDE_4.to_string(),  // Claude 4 with extended thinking
        messages: vec![ChatMessage::user("List all prime numbers between 1 and 50 with explanation.")],
        max_tokens: Some(12000),
        thinking: Some(ThinkingConfig::with_budget(8000)),
        ..Default::default()
    };

    let mut stream = provider
        .stream_chat(&api_key, BASE_URL, request)
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
        println!("Full thinking (first 200 chars): {}...", &full_thinking.chars().take(200).collect::<String>());
    }

    assert!(chunk_count > 0);
    assert!(!full_content.is_empty());
}
