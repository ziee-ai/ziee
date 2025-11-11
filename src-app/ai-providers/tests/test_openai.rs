//! OpenAI provider integration tests
//!
//! These tests require a valid OpenAI API key set in the OPENAI_API_KEY environment variable.
//! Run with: OPENAI_API_KEY=your_key cargo test --test test_openai -- --nocapture

use ai_providers::*;
use serde_json::json;

const BASE_URL: &str = "https://api.openai.com/v1";

// Standard chat models (support temperature, top_p, tools)
const MODEL_GPT4O: &str = "gpt-4o";                    // Latest, fastest, multimodal
const MODEL_GPT4_TURBO: &str = "gpt-4-turbo";         // Previous generation
const MODEL_GPT35_TURBO: &str = "gpt-3.5-turbo";      // Older, faster, cheaper

// Reasoning models (NO temperature/top_p, use reasoning_effort)
const MODEL_O1: &str = "o1";                           // Original reasoning model
const MODEL_O3_MINI: &str = "o3-mini";                 // Fast reasoning
const MODEL_O4_MINI: &str = "o4-mini";                 // Latest reasoning
const MODEL_GPT5: &str = "gpt-5";                      // GPT-5 with reasoning

// Vision models
const MODEL_GPT4_VISION: &str = "gpt-4-vision-preview"; // Vision support
const MODEL_GPT4O_VISION: &str = "gpt-4o";              // Also has vision

// Embedding models
const MODEL_EMBEDDING_ADA: &str = "text-embedding-ada-002";
const MODEL_EMBEDDING_3_SMALL: &str = "text-embedding-3-small";
const MODEL_EMBEDDING_3_LARGE: &str = "text-embedding-3-large";

fn get_api_key() -> String {
    std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable must be set")
}

#[tokio::test]
#[ignore] // Remove this attribute to run the test
async fn test_openai_simple_chat() {
    let api_key = get_api_key();
    let provider = OpenAIProvider;

    let request = ChatRequest {
        model: MODEL_GPT4O.to_string(),  // Use latest GPT-4o
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
async fn test_openai_streaming_chat() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = OpenAIProvider;

    let request = ChatRequest {
        model: MODEL_GPT35_TURBO.to_string(),  // Faster for streaming
        messages: vec![ChatMessage::user("Count from 1 to 5, one number per line.")],
        temperature: Some(0.1),
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
async fn test_openai_tool_calling() {
    let api_key = get_api_key();
    let provider = OpenAIProvider;

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
        model: MODEL_GPT4O.to_string(),  // Latest standard model
        messages: vec![ChatMessage::user(
            "What's the weather like in San Francisco?",
        )],
        tools: vec![weather_tool],
        tool_choice: Some(ToolChoice::auto()),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, BASE_URL, request)
        .await
        .expect("Tool calling request failed");

    println!("Response: {:?}", response);

    assert!(!response.choices.is_empty());
    let message = &response.choices[0].message;

    // Should have tool calls
    assert!(
        !message.tool_calls.is_empty(),
        "Expected tool calls in response"
    );

    let tool_call = &message.tool_calls[0];
    assert_eq!(tool_call.tool_type, "function");
    assert_eq!(tool_call.function.name, "get_weather");
    assert!(!tool_call.function.arguments.is_empty());
    assert!(!tool_call.id.is_empty());

    println!("Tool call: {:?}", tool_call);
}

#[tokio::test]
#[ignore]
async fn test_openai_tool_calling_required() {
    let api_key = get_api_key();
    let provider = OpenAIProvider;

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
        model: MODEL_GPT4O.to_string(),  // Latest standard model
        messages: vec![ChatMessage::user("What is 25 * 4?")],
        tools: vec![calculator_tool],
        tool_choice: Some(ToolChoice::required()),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, BASE_URL, request)
        .await
        .expect("Required tool calling failed");

    assert!(!response.choices.is_empty());
    assert!(!response.choices[0].message.tool_calls.is_empty());

    println!("Tool calls: {:?}", response.choices[0].message.tool_calls);
}

#[tokio::test]
#[ignore]
async fn test_openai_tool_calling_specific() {
    let api_key = get_api_key();
    let provider = OpenAIProvider;

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
        model: MODEL_GPT4O.to_string(),  // Latest standard model
        messages: vec![ChatMessage::user("What is the capital of France?")],
        tools: vec![search_tool],
        tool_choice: Some(ToolChoice::function("web_search")),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, BASE_URL, request)
        .await
        .expect("Specific tool calling failed");

    assert!(!response.choices.is_empty());
    assert!(!response.choices[0].message.tool_calls.is_empty());
    assert_eq!(
        response.choices[0].message.tool_calls[0].function.name,
        "web_search"
    );

    println!("Response: {:?}", response);
}

#[tokio::test]
#[ignore]
async fn test_openai_tool_response_conversation() {
    let api_key = get_api_key();
    let provider = OpenAIProvider;

    // First request with tool
    let weather_tool = Tool::function(
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

    let request1 = ChatRequest {
        model: MODEL_GPT4O.to_string(),  // Latest standard model
        messages: vec![ChatMessage::user("What's the weather in Tokyo?")],
        tools: vec![weather_tool.clone()],
        tool_choice: Some(ToolChoice::auto()),
        ..Default::default()
    };

    let response1 = provider
        .chat(&api_key, BASE_URL, request1)
        .await
        .expect("First request failed");

    assert!(!response1.choices[0].message.tool_calls.is_empty());
    let tool_call = &response1.choices[0].message.tool_calls[0];

    // Second request with tool response
    let request2 = ChatRequest {
        model: MODEL_GPT4O.to_string(),  // Latest standard model
        messages: vec![
            ChatMessage::user("What's the weather in Tokyo?"),
            ChatMessage::assistant_with_tools(
                None,
                vec![ToolCall::function(
                    tool_call.id.clone(),
                    "get_weather",
                    r#"{"location": "Tokyo"}"#,
                )],
            ),
            ChatMessage::tool(
                tool_call.id.clone(),
                r#"{"temperature": 22, "condition": "sunny", "unit": "celsius"}"#,
            ),
        ],
        tools: vec![weather_tool],
        ..Default::default()
    };

    let response2 = provider
        .chat(&api_key, BASE_URL, request2)
        .await
        .expect("Second request failed");

    println!("Final response: {:?}", response2);
    assert!(!response2.choices[0].message.content.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_openai_multimodal_image() {
    let api_key = get_api_key();
    let provider = OpenAIProvider;

    // Create a small test image (1x1 red pixel PNG)
    let image_data = base64::decode(
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==",
    )
    .expect("Failed to decode test image");

    let request = ChatRequest {
        model: "gpt-4o".to_string(),  // GPT-4o has built-in vision
        messages: vec![ChatMessage::user("What color is this image?")],
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
async fn test_openai_embeddings() {
    let api_key = get_api_key();
    let provider = OpenAIProvider;

    let request = EmbeddingsRequest {
        model: MODEL_EMBEDDING_3_SMALL.to_string(),  // Newer embedding model
        input: vec![
            "The quick brown fox jumps over the lazy dog".to_string(),
            "Hello, world!".to_string(),
        ],
    };

    let response = provider
        .embeddings(&api_key, BASE_URL, request)
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
async fn test_openai_multiple_messages() {
    let api_key = get_api_key();
    let provider = OpenAIProvider;

    let request = ChatRequest {
        model: MODEL_GPT4O.to_string(),  // Latest standard model
        messages: vec![
            ChatMessage::system("You are a helpful math tutor."),
            ChatMessage::user("What is 2 + 2?"),
            ChatMessage::assistant("2 + 2 equals 4."),
            ChatMessage::user("What about 3 + 3?"),
        ],
        temperature: Some(0.1),
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
async fn test_openai_temperature_variations() {
    let api_key = get_api_key();
    let provider = OpenAIProvider;

    for temp in [0.0, 0.5, 1.0] {
        let request = ChatRequest {
            model: MODEL_GPT4O.to_string(),  // Latest standard model
            messages: vec![ChatMessage::user("Say hello in a creative way.")],
            temperature: Some(temp),
            max_tokens: Some(30),
            ..Default::default()
        };

        let response = provider
            .chat(&api_key, BASE_URL, request.clone())
            .await
            .expect("Temperature test failed");

        println!("Temperature {}: {}", temp, response.choices[0].message.content);
    }
}

#[tokio::test]
#[ignore]
async fn test_openai_max_tokens_limit() {
    let api_key = get_api_key();
    let provider = OpenAIProvider;

    let request = ChatRequest {
        model: MODEL_GPT4O.to_string(),  // Latest standard model
        messages: vec![ChatMessage::user("Write a long story about a cat.")],
        max_tokens: Some(10),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, BASE_URL, request)
        .await
        .expect("Max tokens test failed");

    assert!(!response.choices.is_empty());
    assert_eq!(response.choices[0].finish_reason, "length");

    println!("Response: {:?}", response);
}

#[tokio::test]
#[ignore]
async fn test_openai_error_invalid_model() {
    let api_key = get_api_key();
    let provider = OpenAIProvider;

    let request = ChatRequest {
        model: "invalid-model-name-12345".to_string(),
        messages: vec![ChatMessage::user("Hello")],
        ..Default::default()
    };

    let result = provider.chat(&api_key, BASE_URL, request).await;

    assert!(result.is_err());
    println!("Expected error: {:?}", result.unwrap_err());
}

#[tokio::test]
#[ignore]
async fn test_openai_error_invalid_api_key() {
    let provider = OpenAIProvider;

    let request = ChatRequest {
        model: MODEL_GPT4O.to_string(),  // Latest standard model
        messages: vec![ChatMessage::user("Hello")],
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
async fn test_openai_groq_compatibility() {
    // Test that the provider works with Groq (OpenAI-compatible)
    let api_key =
        std::env::var("GROQ_API_KEY").unwrap_or_else(|_| "test_key".to_string());
    let provider = OpenAIProvider;

    let request = ChatRequest {
        model: "llama3-8b-8192".to_string(),
        messages: vec![ChatMessage::user("Say 'Hello from Groq!'")],
        temperature: Some(0.7),
        ..Default::default()
    };

    // This test will be skipped if GROQ_API_KEY is not set
    if std::env::var("GROQ_API_KEY").is_ok() {
        let response = provider
            .chat(&api_key, "https://api.groq.com/openai/v1", request)
            .await
            .expect("Groq request failed");

        println!("Groq response: {:?}", response);
        assert!(!response.choices.is_empty());
    } else {
        println!("Skipping Groq test - GROQ_API_KEY not set");
    }
}

#[tokio::test]
#[ignore]
async fn test_openai_top_p_parameter() {
    let api_key = get_api_key();
    let provider = OpenAIProvider;

    let request = ChatRequest {
        model: MODEL_GPT4O.to_string(),  // Latest standard model
        messages: vec![ChatMessage::user("Complete this: The sky is")],
        top_p: Some(0.1), // Very focused sampling
        max_tokens: Some(10),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, BASE_URL, request)
        .await
        .expect("Top-p test failed");

    println!("Response with top_p=0.1: {}", response.choices[0].message.content);
    assert!(!response.choices.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_openai_reasoning_model_medium() {
    let api_key = get_api_key();
    let provider = OpenAIProvider;

    let request = ChatRequest {
        model: "o3-mini".to_string(),
        messages: vec![ChatMessage::user("What is 127 * 893? Show your reasoning.")],
        max_tokens: Some(5000),
        thinking: Some(ThinkingConfig::with_effort(ThinkingEffort::Medium)),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, BASE_URL, request)
        .await
        .expect("Reasoning model test failed");

    println!("Response: {:?}", response);
    assert!(!response.choices.is_empty());
    assert!(response.choices[0].message.content.contains("113511"));

    // Check if reasoning tokens were tracked
    if let Some(ref usage) = response.usage {
        println!("Reasoning tokens: {:?}", usage.reasoning_tokens);
    }
}

#[tokio::test]
#[ignore]
async fn test_openai_reasoning_model_high_effort() {
    let api_key = get_api_key();
    let provider = OpenAIProvider;

    let request = ChatRequest {
        model: "o4-mini".to_string(),
        messages: vec![ChatMessage::user("Solve this logic puzzle: If all roses are flowers and some flowers fade quickly, what can we conclude about roses?")],
        max_tokens: Some(8000),
        thinking: Some(ThinkingConfig::with_effort(ThinkingEffort::High)),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, BASE_URL, request)
        .await
        .expect("High effort reasoning failed");

    println!("Response: {:?}", response);
    assert!(!response.choices.is_empty());
    assert!(!response.choices[0].message.content.is_empty());

    // High effort should use reasoning tokens
    if let Some(ref usage) = response.usage {
        println!("Reasoning tokens (high effort): {:?}", usage.reasoning_tokens);
    }
}

#[tokio::test]
#[ignore]
async fn test_openai_reasoning_model_streaming() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = OpenAIProvider;

    let request = ChatRequest {
        model: "o3-mini".to_string(),
        messages: vec![ChatMessage::user("Count the prime numbers between 1 and 20.")],
        max_tokens: Some(3000),
        thinking: Some(ThinkingConfig::with_effort(ThinkingEffort::Low)),
        ..Default::default()
    };

    let mut stream = provider
        .stream_chat(&api_key, BASE_URL, request)
        .await
        .expect("Reasoning streaming failed");

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

// ==================== GROQ TESTS (OpenAI-Compatible) ====================

#[tokio::test]
#[ignore]
async fn test_groq_simple_chat() {
    let api_key = std::env::var("GROQ_API_KEY").expect("GROQ_API_KEY environment variable must be set");
    let provider = OpenAIProvider;

    let request = ChatRequest {
        model: "llama-3.3-70b-versatile".to_string(),
        messages: vec![
            ChatMessage::system("You are a helpful assistant."),
            ChatMessage::user("Say 'Hello from Groq!' and nothing else."),
        ],
        temperature: Some(0.7),
        max_tokens: Some(50),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, "https://api.groq.com/openai/v1", request)
        .await
        .expect("Groq chat request failed");

    assert!(!response.id.is_empty());
    assert!(!response.choices.is_empty());
    assert!(response.choices[0]
        .message
        .content
        .to_lowercase()
        .contains("groq"));
    assert!(response.usage.is_some());

    println!("Groq Response: {:?}", response);
}

#[tokio::test]
#[ignore]
async fn test_groq_streaming_chat() {
    use futures_util::StreamExt;

    let api_key = std::env::var("GROQ_API_KEY").expect("GROQ_API_KEY environment variable must be set");
    let provider = OpenAIProvider;

    let request = ChatRequest {
        model: "llama-3.3-70b-versatile".to_string(),
        messages: vec![ChatMessage::user("Count from 1 to 5, one number per line.")],
        temperature: Some(0.1),
        max_tokens: Some(100),
        ..Default::default()
    };

    let mut stream = provider
        .stream_chat(&api_key, "https://api.groq.com/openai/v1", request)
        .await
        .expect("Groq stream chat request failed");

    let mut full_content = String::new();
    let mut chunk_count = 0;

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                full_content.push_str(&chunk.content);
                chunk_count += 1;
                print!("{}", chunk.content);
            }
            Err(e) => panic!("Groq stream error: {:?}", e),
        }
    }

    println!("\n\nGroq: Received {} chunks", chunk_count);
    println!("Full content: {}", full_content);

    assert!(chunk_count > 0);
    assert!(!full_content.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_groq_tool_calling() {
    let api_key = std::env::var("GROQ_API_KEY").expect("GROQ_API_KEY environment variable must be set");
    let provider = OpenAIProvider;

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
                    "enum": ["celsius", "fahrenheit"]
                }
            },
            "required": ["location"]
        }),
    );

    let request = ChatRequest {
        model: "llama-3.3-70b-versatile".to_string(),
        messages: vec![ChatMessage::user(
            "What's the weather like in San Francisco?",
        )],
        tools: vec![weather_tool],
        tool_choice: Some(ToolChoice::auto()),
        max_tokens: Some(1000),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, "https://api.groq.com/openai/v1", request)
        .await
        .expect("Groq tool calling request failed");

    println!("Groq tool response: {:?}", response);
    assert!(!response.choices.is_empty());
    
    if !response.choices[0].message.tool_calls.is_empty() {
        println!("Groq tool calls: {:?}", response.choices[0].message.tool_calls);
    }
}

#[tokio::test]
#[ignore]
async fn test_groq_multimodal_with_llama_vision() {
    let api_key = std::env::var("GROQ_API_KEY").expect("GROQ_API_KEY environment variable must be set");
    let provider = OpenAIProvider;

    // Create a small test image (1x1 red pixel PNG)
    let image_data = base64::decode(
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==",
    )
    .expect("Failed to decode test image");

    let request = ChatRequest {
        model: "llama-3.2-90b-vision-preview".to_string(),
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
        .chat(&api_key, "https://api.groq.com/openai/v1", request)
        .await
        .expect("Groq multimodal request failed");

    println!("Groq vision response: {:?}", response);
    assert!(!response.choices.is_empty());
    assert!(!response.choices[0].message.content.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_groq_multiple_models() {
    let api_key = std::env::var("GROQ_API_KEY").expect("GROQ_API_KEY environment variable must be set");
    let provider = OpenAIProvider;

    let models = vec![
        "llama-3.3-70b-versatile",
        "llama-3.1-8b-instant",
        "mixtral-8x7b-32768",
    ];

    for model in models {
        println!("\nTesting Groq model: {}", model);
        
        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![ChatMessage::user("Say hello!")],
            max_tokens: Some(50),
            ..Default::default()
        };

        let response = provider
            .chat(&api_key, "https://api.groq.com/openai/v1", request)
            .await
            .expect(&format!("Groq {} request failed", model));

        println!("{} response: {}", model, response.choices[0].message.content);
        assert!(!response.choices.is_empty());
    }
}

#[tokio::test]
#[ignore]
async fn test_groq_fast_inference() {
    use std::time::Instant;

    let api_key = std::env::var("GROQ_API_KEY").expect("GROQ_API_KEY environment variable must be set");
    let provider = OpenAIProvider;

    let start = Instant::now();

    let request = ChatRequest {
        model: "llama-3.1-8b-instant".to_string(),
        messages: vec![ChatMessage::user("Count from 1 to 10")],
        max_tokens: Some(100),
        ..Default::default()
    };

    let response = provider
        .chat(&api_key, "https://api.groq.com/openai/v1", request)
        .await
        .expect("Groq fast inference failed");

    let duration = start.elapsed();

    println!("Groq response time: {:?}", duration);
    println!("Response: {}", response.choices[0].message.content);
    
    assert!(!response.choices.is_empty());
    // Groq is known for being very fast
    println!("Note: Groq inference completed in {:?}", duration);
}
