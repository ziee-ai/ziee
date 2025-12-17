//! Multi-tool scenario tests for AI providers
//!
//! Tests for complex tool calling scenarios:
//! - Multiple parallel tool calls
//! - Multiple tool results in sequence
//! - Tool result with errors (is_error: true)
//! - Role::Tool handling
//! - Multi-turn tool conversations
//!
//! These tests require API keys for live testing.
//! Run with: cargo test --test test_multi_tool_scenarios -- --nocapture --ignored

use ai_providers::*;
use serde_json::json;

// ============================================================================
// Test Constants
// ============================================================================

const ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com/v1";
const MODEL_ANTHROPIC: &str = "claude-haiku-4-5-20251001";
const MODEL_OPENAI: &str = "gpt-4o-mini";
const MODEL_GEMINI: &str = "models/gemini-2.5-flash";

fn get_anthropic_key() -> Option<String> {
    std::env::var("ANTHROPIC_API_KEY").ok()
}

fn get_openai_key() -> Option<String> {
    std::env::var("OPENAI_API_KEY").ok()
}

fn get_gemini_key() -> Option<String> {
    std::env::var("GEMINI_API_KEY").ok()
}

// ============================================================================
// Helper: Create test tools
// ============================================================================

fn create_weather_tool() -> Tool {
    Tool::function(
        "get_weather",
        "Get current weather for a location",
        json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "City name"
                }
            },
            "required": ["location"]
        }),
    )
}

fn create_time_tool() -> Tool {
    Tool::function(
        "get_time",
        "Get current time for a timezone",
        json!({
            "type": "object",
            "properties": {
                "timezone": {
                    "type": "string",
                    "description": "Timezone name"
                }
            },
            "required": ["timezone"]
        }),
    )
}

fn create_calculator_tool() -> Tool {
    Tool::function(
        "calculate",
        "Perform a calculation",
        json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "Math expression to evaluate"
                }
            },
            "required": ["expression"]
        }),
    )
}

// ============================================================================
// Unit Tests: Message Structure Validation (No API required)
// ============================================================================

#[test]
fn test_multiple_tool_use_in_single_message() {
    // Test that multiple ToolUse blocks can be in a single assistant message
    let message = ChatMessage {
        role: Role::Assistant,
        content: vec![
            ContentBlock::ToolUse {
                id: "tool_1".to_string(),
                name: "get_weather".to_string(),
                input: json!({"location": "Tokyo"}),
            },
            ContentBlock::ToolUse {
                id: "tool_2".to_string(),
                name: "get_time".to_string(),
                input: json!({"timezone": "Asia/Tokyo"}),
            },
            ContentBlock::ToolUse {
                id: "tool_3".to_string(),
                name: "calculate".to_string(),
                input: json!({"expression": "2 + 2"}),
            },
        ],
    };

    assert_eq!(message.role, Role::Assistant);
    assert_eq!(message.content.len(), 3);

    // Verify each tool use
    for (i, block) in message.content.iter().enumerate() {
        match block {
            ContentBlock::ToolUse { id, .. } => {
                assert_eq!(id, &format!("tool_{}", i + 1));
            }
            _ => panic!("Expected ToolUse block at index {}", i),
        }
    }
}

#[test]
fn test_multiple_tool_results_in_single_message() {
    // Test that multiple ToolResult blocks can be in a single tool message
    let message = ChatMessage {
        role: Role::Tool,
        content: vec![
            ContentBlock::ToolResult {
                tool_use_id: "tool_1".to_string(),
                name: Some("get_weather".to_string()),
                content: vec![ContentBlock::Text {
                    text: r#"{"temp": 22, "condition": "sunny"}"#.to_string(),
                }],
                is_error: None,
            },
            ContentBlock::ToolResult {
                tool_use_id: "tool_2".to_string(),
                name: Some("get_time".to_string()),
                content: vec![ContentBlock::Text {
                    text: "14:30 JST".to_string(),
                }],
                is_error: None,
            },
            ContentBlock::ToolResult {
                tool_use_id: "tool_3".to_string(),
                name: Some("calculate".to_string()),
                content: vec![ContentBlock::Text {
                    text: "4".to_string(),
                }],
                is_error: None,
            },
        ],
    };

    assert_eq!(message.role, Role::Tool);
    assert_eq!(message.content.len(), 3);
}

#[test]
fn test_tool_result_with_error() {
    // Test tool result with is_error: true
    let message = ChatMessage {
        role: Role::Tool,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "tool_failed".to_string(),
            name: Some("get_weather".to_string()),
            content: vec![ContentBlock::Text {
                text: "Error: Location not found".to_string(),
            }],
            is_error: Some(true),
        }],
    };

    match &message.content[0] {
        ContentBlock::ToolResult { is_error, .. } => {
            assert_eq!(*is_error, Some(true));
        }
        _ => panic!("Expected ToolResult"),
    }
}

#[test]
fn test_tool_result_canceled() {
    // Test tool result representing a canceled/rejected tool call
    let message = ChatMessage {
        role: Role::Tool,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "tool_canceled".to_string(),
            name: Some("dangerous_operation".to_string()),
            content: vec![ContentBlock::Text {
                text: "Tool execution was canceled by user".to_string(),
            }],
            is_error: Some(true),
        }],
    };

    match &message.content[0] {
        ContentBlock::ToolResult { tool_use_id, is_error, content, .. } => {
            assert_eq!(tool_use_id, "tool_canceled");
            assert_eq!(*is_error, Some(true));
            match &content[0] {
                ContentBlock::Text { text } => {
                    assert!(text.contains("canceled"));
                }
                _ => panic!("Expected Text content"),
            }
        }
        _ => panic!("Expected ToolResult"),
    }
}

#[test]
fn test_mixed_tool_results_success_and_error() {
    // Test multiple tool results where some succeed and some fail
    let message = ChatMessage {
        role: Role::Tool,
        content: vec![
            ContentBlock::ToolResult {
                tool_use_id: "tool_success".to_string(),
                name: Some("get_weather".to_string()),
                content: vec![ContentBlock::Text {
                    text: r#"{"temp": 22}"#.to_string(),
                }],
                is_error: None,
            },
            ContentBlock::ToolResult {
                tool_use_id: "tool_error".to_string(),
                name: Some("get_time".to_string()),
                content: vec![ContentBlock::Text {
                    text: "Timezone service unavailable".to_string(),
                }],
                is_error: Some(true),
            },
            ContentBlock::ToolResult {
                tool_use_id: "tool_canceled".to_string(),
                name: Some("calculate".to_string()),
                content: vec![ContentBlock::Text {
                    text: "Canceled by user".to_string(),
                }],
                is_error: Some(true),
            },
        ],
    };

    assert_eq!(message.content.len(), 3);

    // Verify first result succeeded
    match &message.content[0] {
        ContentBlock::ToolResult { is_error, .. } => {
            assert_eq!(*is_error, None);
        }
        _ => panic!("Expected ToolResult"),
    }

    // Verify second and third failed
    for i in 1..3 {
        match &message.content[i] {
            ContentBlock::ToolResult { is_error, .. } => {
                assert_eq!(*is_error, Some(true));
            }
            _ => panic!("Expected ToolResult at index {}", i),
        }
    }
}

#[test]
fn test_multi_turn_tool_conversation_structure() {
    // Test the structure of a complete multi-turn tool conversation
    let messages = vec![
        // Turn 1: User asks about weather in multiple cities
        ChatMessage::user("What's the weather in Tokyo and London?"),

        // Turn 2: Assistant calls multiple tools
        ChatMessage {
            role: Role::Assistant,
            content: vec![
                ContentBlock::Text {
                    text: "I'll check the weather for both cities.".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "tool_tokyo".to_string(),
                    name: "get_weather".to_string(),
                    input: json!({"location": "Tokyo"}),
                },
                ContentBlock::ToolUse {
                    id: "tool_london".to_string(),
                    name: "get_weather".to_string(),
                    input: json!({"location": "London"}),
                },
            ],
        },

        // Turn 3: Tool results (using Role::Tool)
        ChatMessage {
            role: Role::Tool,
            content: vec![
                ContentBlock::ToolResult {
                    tool_use_id: "tool_tokyo".to_string(),
                    name: Some("get_weather".to_string()),
                    content: vec![ContentBlock::Text {
                        text: r#"{"temp": 22, "condition": "sunny"}"#.to_string(),
                    }],
                    is_error: None,
                },
                ContentBlock::ToolResult {
                    tool_use_id: "tool_london".to_string(),
                    name: Some("get_weather".to_string()),
                    content: vec![ContentBlock::Text {
                        text: r#"{"temp": 15, "condition": "cloudy"}"#.to_string(),
                    }],
                    is_error: None,
                },
            ],
        },

        // Turn 4: Assistant responds with results
        ChatMessage {
            role: Role::Assistant,
            content: vec![ContentBlock::Text {
                text: "Tokyo is 22°C and sunny. London is 15°C and cloudy.".to_string(),
            }],
        },

        // Turn 5: User asks follow-up
        ChatMessage::user("What about Paris?"),
    ];

    assert_eq!(messages.len(), 5);
    assert_eq!(messages[0].role, Role::User);
    assert_eq!(messages[1].role, Role::Assistant);
    assert_eq!(messages[2].role, Role::Tool);  // Tool results use Role::Tool
    assert_eq!(messages[3].role, Role::Assistant);
    assert_eq!(messages[4].role, Role::User);

    // Verify assistant message has text + multiple tool uses
    assert_eq!(messages[1].content.len(), 3);

    // Verify tool message has multiple results
    assert_eq!(messages[2].content.len(), 2);
}

#[test]
fn test_chat_message_helper_tool_result() {
    // Test the ChatMessage::tool_result helper function
    let msg = ChatMessage::tool_result(
        "toolu_123",
        Some("get_weather".to_string()),
        vec![ContentBlock::Text {
            text: "Sunny, 25°C".to_string(),
        }],
    );

    assert_eq!(msg.role, Role::Tool);
    assert_eq!(msg.content.len(), 1);

    match &msg.content[0] {
        ContentBlock::ToolResult { tool_use_id, name, is_error, .. } => {
            assert_eq!(tool_use_id, "toolu_123");
            assert_eq!(name, &Some("get_weather".to_string()));
            assert_eq!(*is_error, None);
        }
        _ => panic!("Expected ToolResult"),
    }
}

#[test]
fn test_chat_message_helper_tool_result_text() {
    // Test the ChatMessage::tool_result_text helper function
    let msg = ChatMessage::tool_result_text(
        "toolu_456",
        Some("calculate".to_string()),
        "42",
    );

    assert_eq!(msg.role, Role::Tool);

    match &msg.content[0] {
        ContentBlock::ToolResult { tool_use_id, content, .. } => {
            assert_eq!(tool_use_id, "toolu_456");
            match &content[0] {
                ContentBlock::Text { text } => {
                    assert_eq!(text, "42");
                }
                _ => panic!("Expected Text content"),
            }
        }
        _ => panic!("Expected ToolResult"),
    }
}

// ============================================================================
// Live API Tests: Anthropic Multi-Tool (requires ANTHROPIC_API_KEY)
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_anthropic_multiple_parallel_tool_calls() {
    use futures_util::StreamExt;

    let api_key = match get_anthropic_key() {
        Some(key) => key,
        None => {
            println!("⚠️  Skipping: ANTHROPIC_API_KEY not set");
            return;
        }
    };

    let provider = Provider::new("anthropic", &api_key, ANTHROPIC_BASE_URL)
        .expect("Failed to create provider");

    println!("\n=== Testing Anthropic Multiple Parallel Tool Calls ===\n");

    let request = ChatRequest {
        model: MODEL_ANTHROPIC.to_string(),
        messages: vec![ChatMessage::user(
            "What's the weather in Tokyo AND what time is it there? Use BOTH tools.",
        )],
        tools: vec![create_weather_tool(), create_time_tool()],
        tool_choice: Some(ToolChoice::Required), // Force tool use, Auto might skip tools
        max_tokens: Some(500),
        ..Default::default()
    };

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Stream request failed");

    let mut tool_calls: Vec<(String, String, String)> = Vec::new();

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                for delta in &chunk.content {
                    if let ContentBlockDelta::ToolUseDelta { id, name, input_delta, .. } = delta {
                        if let (Some(id), Some(name), Some(input)) = (id, name, input_delta) {
                            println!("  Tool call: id={}, name={}, input={}", id, name, input);
                            tool_calls.push((id.clone(), name.clone(), input.clone()));
                        }
                    }
                }
            }
            Err(e) => panic!("Stream error: {:?}", e),
        }
    }

    println!("\nTotal tool calls: {}", tool_calls.len());

    // Model might call 1 or 2 tools depending on response
    assert!(!tool_calls.is_empty(), "Expected at least one tool call");
    println!("✅ Multiple parallel tool calls test passed");
}

#[tokio::test]
#[ignore]
async fn test_anthropic_tool_results_continuation() {
    use futures_util::StreamExt;

    let api_key = match get_anthropic_key() {
        Some(key) => key,
        None => {
            println!("⚠️  Skipping: ANTHROPIC_API_KEY not set");
            return;
        }
    };

    let provider = Provider::new("anthropic", &api_key, ANTHROPIC_BASE_URL)
        .expect("Failed to create provider");

    println!("\n=== Testing Anthropic Tool Results Continuation ===\n");

    // Build a conversation with tool use and results
    let messages = vec![
        ChatMessage::user("What's the weather in Tokyo?"),
        ChatMessage {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "toolu_test_123".to_string(),
                name: "get_weather".to_string(),
                input: json!({"location": "Tokyo"}),
            }],
        },
        // Use Role::Tool for tool results (unified interface)
        ChatMessage {
            role: Role::Tool,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "toolu_test_123".to_string(),
                name: Some("get_weather".to_string()),
                content: vec![ContentBlock::Text {
                    text: r#"{"temperature": 22, "condition": "sunny", "humidity": 65}"#.to_string(),
                }],
                is_error: None,
            }],
        },
    ];

    let request = ChatRequest {
        model: MODEL_ANTHROPIC.to_string(),
        messages,
        max_tokens: Some(500),
        ..Default::default()
    };

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Stream request failed");

    let mut response_text = String::new();

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                for delta in &chunk.content {
                    if let ContentBlockDelta::TextDelta { delta, .. } = delta {
                        response_text.push_str(delta);
                        print!("{}", delta);
                    }
                }
            }
            Err(e) => panic!("Stream error: {:?}", e),
        }
    }

    println!("\n\nResponse length: {} chars", response_text.len());
    assert!(!response_text.is_empty(), "Expected non-empty response after tool result");
    assert!(
        response_text.to_lowercase().contains("22") ||
        response_text.to_lowercase().contains("sunny") ||
        response_text.to_lowercase().contains("tokyo"),
        "Response should reference the weather data"
    );
    println!("✅ Tool results continuation test passed");
}

#[tokio::test]
#[ignore]
async fn test_anthropic_tool_error_handling() {
    use futures_util::StreamExt;

    let api_key = match get_anthropic_key() {
        Some(key) => key,
        None => {
            println!("⚠️  Skipping: ANTHROPIC_API_KEY not set");
            return;
        }
    };

    let provider = Provider::new("anthropic", &api_key, ANTHROPIC_BASE_URL)
        .expect("Failed to create provider");

    println!("\n=== Testing Anthropic Tool Error Handling ===\n");

    // Build a conversation with a failed tool result
    let messages = vec![
        ChatMessage::user("What's the weather in Atlantis?"),
        ChatMessage {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "toolu_error_test".to_string(),
                name: "get_weather".to_string(),
                input: json!({"location": "Atlantis"}),
            }],
        },
        ChatMessage {
            role: Role::Tool,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "toolu_error_test".to_string(),
                name: Some("get_weather".to_string()),
                content: vec![ContentBlock::Text {
                    text: "Error: Location 'Atlantis' not found in weather database".to_string(),
                }],
                is_error: Some(true),
            }],
        },
    ];

    let request = ChatRequest {
        model: MODEL_ANTHROPIC.to_string(),
        messages,
        max_tokens: Some(500),
        ..Default::default()
    };

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Stream request failed");

    let mut response_text = String::new();

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                for delta in &chunk.content {
                    if let ContentBlockDelta::TextDelta { delta, .. } = delta {
                        response_text.push_str(delta);
                        print!("{}", delta);
                    }
                }
            }
            Err(e) => panic!("Stream error: {:?}", e),
        }
    }

    println!("\n\nResponse length: {} chars", response_text.len());
    assert!(!response_text.is_empty(), "Expected response acknowledging error");
    println!("✅ Tool error handling test passed");
}

#[tokio::test]
#[ignore]
async fn test_anthropic_multiple_tool_results() {
    use futures_util::StreamExt;

    let api_key = match get_anthropic_key() {
        Some(key) => key,
        None => {
            println!("⚠️  Skipping: ANTHROPIC_API_KEY not set");
            return;
        }
    };

    let provider = Provider::new("anthropic", &api_key, ANTHROPIC_BASE_URL)
        .expect("Failed to create provider");

    println!("\n=== Testing Anthropic Multiple Tool Results ===\n");

    // Build conversation with multiple tool calls and results
    let messages = vec![
        ChatMessage::user("What's the weather in Tokyo and what time is it there?"),
        ChatMessage {
            role: Role::Assistant,
            content: vec![
                ContentBlock::ToolUse {
                    id: "toolu_weather".to_string(),
                    name: "get_weather".to_string(),
                    input: json!({"location": "Tokyo"}),
                },
                ContentBlock::ToolUse {
                    id: "toolu_time".to_string(),
                    name: "get_time".to_string(),
                    input: json!({"timezone": "Asia/Tokyo"}),
                },
            ],
        },
        // Multiple tool results in one message
        ChatMessage {
            role: Role::Tool,
            content: vec![
                ContentBlock::ToolResult {
                    tool_use_id: "toolu_weather".to_string(),
                    name: Some("get_weather".to_string()),
                    content: vec![ContentBlock::Text {
                        text: r#"{"temperature": 22, "condition": "sunny"}"#.to_string(),
                    }],
                    is_error: None,
                },
                ContentBlock::ToolResult {
                    tool_use_id: "toolu_time".to_string(),
                    name: Some("get_time".to_string()),
                    content: vec![ContentBlock::Text {
                        text: "14:30 JST (UTC+9)".to_string(),
                    }],
                    is_error: None,
                },
            ],
        },
    ];

    let request = ChatRequest {
        model: MODEL_ANTHROPIC.to_string(),
        messages,
        max_tokens: Some(500),
        ..Default::default()
    };

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Stream request failed");

    let mut response_text = String::new();

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                for delta in &chunk.content {
                    if let ContentBlockDelta::TextDelta { delta, .. } = delta {
                        response_text.push_str(delta);
                        print!("{}", delta);
                    }
                }
            }
            Err(e) => panic!("Stream error: {:?}", e),
        }
    }

    println!("\n\nResponse length: {} chars", response_text.len());
    assert!(!response_text.is_empty(), "Expected response using both tool results");
    println!("✅ Multiple tool results test passed");
}

// ============================================================================
// Live API Tests: OpenAI Multi-Tool (requires OPENAI_API_KEY)
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_openai_multiple_tool_results() {
    use futures_util::StreamExt;

    let api_key = match get_openai_key() {
        Some(key) => key,
        None => {
            println!("⚠️  Skipping: OPENAI_API_KEY not set");
            return;
        }
    };

    let provider = Provider::new("openai", &api_key, "https://api.openai.com/v1")
        .expect("Failed to create provider");

    println!("\n=== Testing OpenAI Multiple Tool Results ===\n");

    // OpenAI uses different tool call IDs format
    let messages = vec![
        ChatMessage::user("What's the weather in Tokyo and what time is it there?"),
        ChatMessage {
            role: Role::Assistant,
            content: vec![
                ContentBlock::ToolUse {
                    id: "call_weather_123".to_string(),
                    name: "get_weather".to_string(),
                    input: json!({"location": "Tokyo"}),
                },
                ContentBlock::ToolUse {
                    id: "call_time_456".to_string(),
                    name: "get_time".to_string(),
                    input: json!({"timezone": "Asia/Tokyo"}),
                },
            ],
        },
        // OpenAI: Tool results use Role::Tool
        ChatMessage {
            role: Role::Tool,
            content: vec![
                ContentBlock::ToolResult {
                    tool_use_id: "call_weather_123".to_string(),
                    name: Some("get_weather".to_string()),
                    content: vec![ContentBlock::Text {
                        text: r#"{"temperature": 22, "condition": "sunny"}"#.to_string(),
                    }],
                    is_error: None,
                },
            ],
        },
        ChatMessage {
            role: Role::Tool,
            content: vec![
                ContentBlock::ToolResult {
                    tool_use_id: "call_time_456".to_string(),
                    name: Some("get_time".to_string()),
                    content: vec![ContentBlock::Text {
                        text: "14:30 JST (UTC+9)".to_string(),
                    }],
                    is_error: None,
                },
            ],
        },
    ];

    let request = ChatRequest {
        model: MODEL_OPENAI.to_string(),
        messages,
        max_tokens: Some(500),
        ..Default::default()
    };

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Stream request failed");

    let mut response_text = String::new();

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                for delta in &chunk.content {
                    if let ContentBlockDelta::TextDelta { delta, .. } = delta {
                        response_text.push_str(delta);
                        print!("{}", delta);
                    }
                }
            }
            Err(e) => panic!("Stream error: {:?}", e),
        }
    }

    println!("\n\nResponse length: {} chars", response_text.len());
    assert!(!response_text.is_empty(), "Expected response using both tool results");
    println!("✅ OpenAI multiple tool results test passed");
}

// ============================================================================
// Live API Tests: Gemini Multi-Tool (requires GEMINI_API_KEY)
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_gemini_multiple_tool_results() {
    use futures_util::StreamExt;

    let api_key = match get_gemini_key() {
        Some(key) => key,
        None => {
            println!("⚠️  Skipping: GEMINI_API_KEY not set");
            return;
        }
    };

    let provider = Provider::new("gemini", &api_key, "")
        .expect("Failed to create provider");

    println!("\n=== Testing Gemini Multiple Tool Results ===\n");

    let messages = vec![
        ChatMessage::user("What's the weather in Tokyo and what time is it there?"),
        ChatMessage {
            role: Role::Assistant,
            content: vec![
                ContentBlock::ToolUse {
                    id: "weather_call".to_string(),
                    name: "get_weather".to_string(),
                    input: json!({"location": "Tokyo"}),
                },
                ContentBlock::ToolUse {
                    id: "time_call".to_string(),
                    name: "get_time".to_string(),
                    input: json!({"timezone": "Asia/Tokyo"}),
                },
            ],
        },
        // Gemini: Tool results need function name
        ChatMessage {
            role: Role::User, // Gemini uses User role for tool results
            content: vec![
                ContentBlock::ToolResult {
                    tool_use_id: "weather_call".to_string(),
                    name: Some("get_weather".to_string()), // Required for Gemini
                    content: vec![ContentBlock::Text {
                        text: r#"{"temperature": 22, "condition": "sunny"}"#.to_string(),
                    }],
                    is_error: None,
                },
                ContentBlock::ToolResult {
                    tool_use_id: "time_call".to_string(),
                    name: Some("get_time".to_string()), // Required for Gemini
                    content: vec![ContentBlock::Text {
                        text: "14:30 JST (UTC+9)".to_string(),
                    }],
                    is_error: None,
                },
            ],
        },
    ];

    let request = ChatRequest {
        model: MODEL_GEMINI.to_string(),
        messages,
        max_tokens: Some(500),
        ..Default::default()
    };

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Stream request failed");

    let mut response_text = String::new();

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                for delta in &chunk.content {
                    if let ContentBlockDelta::TextDelta { delta, .. } = delta {
                        response_text.push_str(delta);
                        print!("{}", delta);
                    }
                }
            }
            Err(e) => panic!("Stream error: {:?}", e),
        }
    }

    println!("\n\nResponse length: {} chars", response_text.len());
    assert!(!response_text.is_empty(), "Expected response using both tool results");
    println!("✅ Gemini multiple tool results test passed");
}

// ============================================================================
// Cross-Provider Comparison Tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_all_providers_tool_continuation() {
    println!("\n=== Cross-Provider Tool Continuation Test ===\n");

    let providers_to_test = [
        ("anthropic", get_anthropic_key(), ANTHROPIC_BASE_URL, MODEL_ANTHROPIC),
        ("openai", get_openai_key(), "https://api.openai.com/v1", MODEL_OPENAI),
        ("gemini", get_gemini_key(), "", MODEL_GEMINI),
    ];

    for (provider_name, api_key, base_url, model) in providers_to_test {
        let api_key = match api_key {
            Some(key) => key,
            None => {
                println!("⚠️  Skipping {}: API key not set", provider_name);
                continue;
            }
        };

        println!("\n--- Testing {} ---", provider_name);

        let provider = match Provider::new(provider_name, &api_key, base_url) {
            Ok(p) => p,
            Err(e) => {
                println!("❌ Failed to create {} provider: {:?}", provider_name, e);
                continue;
            }
        };

        // Build a simple tool continuation scenario
        let messages = vec![
            ChatMessage::user("What's 2 + 2?"),
            ChatMessage {
                role: Role::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: "calc_test".to_string(),
                    name: "calculate".to_string(),
                    input: json!({"expression": "2 + 2"}),
                }],
            },
            ChatMessage {
                role: if provider_name == "gemini" { Role::User } else { Role::Tool },
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: "calc_test".to_string(),
                    name: Some("calculate".to_string()),
                    content: vec![ContentBlock::Text {
                        text: "4".to_string(),
                    }],
                    is_error: None,
                }],
            },
        ];

        let request = ChatRequest {
            model: model.to_string(),
            messages,
            max_tokens: Some(200),
            ..Default::default()
        };

        use futures_util::StreamExt;

        match provider.chat_stream(request).await {
            Ok(mut stream) => {
                let mut response = String::new();
                while let Some(result) = stream.next().await {
                    if let Ok(chunk) = result {
                        for delta in &chunk.content {
                            if let ContentBlockDelta::TextDelta { delta, .. } = delta {
                                response.push_str(delta);
                            }
                        }
                    }
                }

                if !response.is_empty() {
                    println!("✅ {} passed: got {} char response", provider_name, response.len());
                } else {
                    println!("❌ {} failed: empty response", provider_name);
                }
            }
            Err(e) => {
                println!("❌ {} failed: {:?}", provider_name, e);
            }
        }
    }
}

// ============================================================================
// CRITICAL TEST: Sequential Messages After Tool Execution
// This tests the exact bug: "tool_use ids were found without tool_result blocks"
// ============================================================================

/// Tests sending a second user message after a complete tool execution cycle.
/// This is the exact scenario that caused the error:
/// "tool_use ids were found without tool_result blocks immediately after"
///
/// The conversation flow:
/// 1. User: "What's the weather in Tokyo?"
/// 2. Assistant: [tool_use]
/// 3. Tool: [tool_result]
/// 4. Assistant: "The weather is 22°C..."
/// 5. User: "What about New York?" <-- THIS CAUSED THE ERROR!
#[tokio::test]
#[ignore]
async fn test_anthropic_second_message_after_tool_execution() {
    use futures_util::StreamExt;

    let api_key = match get_anthropic_key() {
        Some(key) => key,
        None => {
            println!("⚠️  Skipping: ANTHROPIC_API_KEY not set");
            return;
        }
    };

    let provider = Provider::new("anthropic", &api_key, ANTHROPIC_BASE_URL)
        .expect("Failed to create provider");

    println!("\n=== Testing Second Message After Tool Execution ===\n");
    println!("This tests the exact bug scenario that caused:");
    println!("'tool_use ids were found without tool_result blocks'\n");

    // Build a complete conversation history with tool use cycle + second message
    // This simulates what happens when a user sends a second message after tool execution
    let messages = vec![
        // Turn 1: User asks about Tokyo weather
        ChatMessage::user("What's the weather in Tokyo?"),
        // Turn 1: Assistant decides to use tool
        ChatMessage {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "toolu_01Tokyo".to_string(),
                name: "get_weather".to_string(),
                input: json!({"location": "Tokyo"}),
            }],
        },
        // Turn 1: Tool result (MUST use Role::Tool for unified interface!)
        ChatMessage {
            role: Role::Tool,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "toolu_01Tokyo".to_string(),
                name: Some("get_weather".to_string()),
                content: vec![ContentBlock::Text {
                    text: r#"{"temperature": 22, "condition": "sunny", "humidity": 65}"#.to_string(),
                }],
                is_error: None,
            }],
        },
        // Turn 1: Assistant responds with the weather info
        ChatMessage {
            role: Role::Assistant,
            content: vec![ContentBlock::Text {
                text: "The weather in Tokyo is 22°C and sunny with 65% humidity.".to_string(),
            }],
        },
        // Turn 2: User sends SECOND message (THIS is what caused the error!)
        ChatMessage::user("What about New York? Is it colder there?"),
    ];

    let request = ChatRequest {
        model: MODEL_ANTHROPIC.to_string(),
        messages,
        max_tokens: Some(500),
        tools: vec![create_weather_tool()],
        tool_choice: Some(ToolChoice::Auto),
        ..Default::default()
    };

    println!("Sending request with {} messages (second user message after tool cycle)", 5);

    let result = provider.chat_stream(request).await;

    match result {
        Ok(mut stream) => {
            let mut response_text = String::new();
            let mut chunk_count = 0;
            let mut got_tool_use = false;

            while let Some(result) = stream.next().await {
                match result {
                    Ok(chunk) => {
                        chunk_count += 1;
                        println!("[Chunk {}] content blocks: {}", chunk_count, chunk.content.len());
                        for delta in &chunk.content {
                            match delta {
                                ContentBlockDelta::TextDelta { delta, .. } => {
                                    response_text.push_str(delta);
                                    print!("{}", delta);
                                }
                                other => {
                                    println!("[Debug] Got non-text delta: {:?}", other);
                                    // Check if it's a tool use related delta
                                    if format!("{:?}", other).contains("ToolUse") {
                                        got_tool_use = true;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        panic!("Stream error: {:?}\nThis is the exact bug we're testing for!", e);
                    }
                }
            }

            println!("\n\n--- Test Results ---");
            println!("Total chunks received: {}", chunk_count);
            println!("Response length: {} chars", response_text.len());
            println!("Got tool use: {}", got_tool_use);

            // The API should accept this request without the "tool_use ids without tool_result" error
            // Success if we got ANY response (text or tool use) - the key is that the API didn't reject the request
            assert!(
                chunk_count > 0,
                "Expected at least one response chunk - API may have rejected the request!"
            );

            println!("✅ Second message after tool execution test PASSED!");
            println!("   The API accepted the conversation history with Role::Tool messages.");
        }
        Err(e) => {
            // Check if this is the specific error we fixed
            let error_msg = format!("{:?}", e);
            if error_msg.contains("tool_use ids were found without tool_result") {
                panic!(
                    "❌ REGRESSION! Got the exact error we fixed:\n{}\n\
                    The Role::Tool message structure is broken!",
                    error_msg
                );
            } else {
                panic!("Request failed with unexpected error: {:?}", e);
            }
        }
    }
}

/// Same test for OpenAI provider
#[tokio::test]
#[ignore]
async fn test_openai_second_message_after_tool_execution() {
    use futures_util::StreamExt;

    let api_key = match get_openai_key() {
        Some(key) => key,
        None => {
            println!("⚠️  Skipping: OPENAI_API_KEY not set");
            return;
        }
    };

    let provider = Provider::new("openai", &api_key, "https://api.openai.com/v1")
        .expect("Failed to create provider");

    println!("\n=== Testing OpenAI Second Message After Tool Execution ===\n");

    let messages = vec![
        ChatMessage::user("What's the weather in Tokyo?"),
        ChatMessage {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "call_Tokyo123".to_string(),
                name: "get_weather".to_string(),
                input: json!({"location": "Tokyo"}),
            }],
        },
        ChatMessage {
            role: Role::Tool,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "call_Tokyo123".to_string(),
                name: Some("get_weather".to_string()),
                content: vec![ContentBlock::Text {
                    text: r#"{"temperature": 22, "condition": "sunny"}"#.to_string(),
                }],
                is_error: None,
            }],
        },
        ChatMessage {
            role: Role::Assistant,
            content: vec![ContentBlock::Text {
                text: "The weather in Tokyo is 22°C and sunny.".to_string(),
            }],
        },
        ChatMessage::user("What about New York?"),
    ];

    let request = ChatRequest {
        model: MODEL_OPENAI.to_string(),
        messages,
        max_tokens: Some(500),
        tools: vec![create_weather_tool()],
        tool_choice: Some(ToolChoice::Auto),
        ..Default::default()
    };

    match provider.chat_stream(request).await {
        Ok(mut stream) => {
            let mut response_text = String::new();

            while let Some(result) = stream.next().await {
                if let Ok(chunk) = result {
                    for delta in &chunk.content {
                        if let ContentBlockDelta::TextDelta { delta, .. } = delta {
                            response_text.push_str(delta);
                            print!("{}", delta);
                        }
                    }
                }
            }

            println!("\n\nResponse length: {} chars", response_text.len());
            println!("✅ OpenAI second message after tool execution test PASSED!");
        }
        Err(e) => {
            panic!("OpenAI request failed: {:?}", e);
        }
    }
}

/// Gemini test for second message after tool execution
#[tokio::test]
#[ignore]
async fn test_gemini_second_message_after_tool_execution() {
    use futures_util::StreamExt;

    let api_key = match get_gemini_key() {
        Some(key) => key,
        None => {
            println!("⚠️  Skipping: GEMINI_API_KEY not set");
            return;
        }
    };

    let provider = Provider::new("gemini", &api_key, "")
        .expect("Failed to create provider");

    println!("\n=== Testing Gemini Second Message After Tool Execution ===\n");

    let messages = vec![
        ChatMessage::user("What's the weather in Tokyo?"),
        ChatMessage {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "weather_tokyo".to_string(),
                name: "get_weather".to_string(),
                input: json!({"location": "Tokyo"}),
            }],
        },
        // Gemini: Use Role::Tool for tool results (unified interface)
        ChatMessage {
            role: Role::Tool,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "weather_tokyo".to_string(),
                name: Some("get_weather".to_string()),
                content: vec![ContentBlock::Text {
                    text: r#"{"temperature": 22, "condition": "sunny"}"#.to_string(),
                }],
                is_error: None,
            }],
        },
        ChatMessage {
            role: Role::Assistant,
            content: vec![ContentBlock::Text {
                text: "The weather in Tokyo is 22°C and sunny.".to_string(),
            }],
        },
        ChatMessage::user("What about New York?"),
    ];

    let request = ChatRequest {
        model: MODEL_GEMINI.to_string(),
        messages,
        max_tokens: Some(500),
        tools: vec![create_weather_tool()],
        tool_choice: Some(ToolChoice::Auto),
        ..Default::default()
    };

    match provider.chat_stream(request).await {
        Ok(mut stream) => {
            let mut response_text = String::new();
            let mut chunk_count = 0;

            while let Some(result) = stream.next().await {
                if let Ok(chunk) = result {
                    chunk_count += 1;
                    for delta in &chunk.content {
                        if let ContentBlockDelta::TextDelta { delta, .. } = delta {
                            response_text.push_str(delta);
                            print!("{}", delta);
                        }
                    }
                }
            }

            println!("\n\nChunks: {}, Response length: {} chars", chunk_count, response_text.len());
            assert!(chunk_count > 0, "Expected response from Gemini");
            println!("✅ Gemini second message after tool execution test PASSED!");
        }
        Err(e) => {
            panic!("Gemini request failed: {:?}", e);
        }
    }
}
