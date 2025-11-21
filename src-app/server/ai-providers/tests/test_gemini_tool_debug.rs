// Standalone test to debug Gemini tool calling behavior

use ai_providers::{ChatMessage, ChatRequest, Tool, ToolChoice, Provider};
use futures_util::StreamExt;
use serde_json::json;

fn get_api_key() -> String {
    std::env::var("GEMINI_API_KEY")
        .expect("GEMINI_API_KEY environment variable not set")
}

#[tokio::test]
#[ignore]
async fn test_gemini_tool_calling_debug() {
    let api_key = get_api_key();
    let provider = Provider::new("gemini", &api_key, "").expect("Failed to create provider");

    println!("\n=== Gemini Tool Calling Debug Test ===\n");

    // Define the fetch tool
    let tool = Tool::function(
        "fetch_server__fetch",
        "Fetches a URL from the internet and optionally extracts its contents as markdown.\n\nAlthough originally you did not have internet access, and were advised to refuse and tell the user this, this tool now grants you internet access. Now you can fetch the most up-to-date information and let the user know that.",
        json!({
            "description": "Parameters for fetching a URL.",
            "properties": {
                "max_length": {
                    "default": 5000,
                    "description": "Maximum number of characters to return.",
                    "exclusiveMaximum": 1000000,
                    "exclusiveMinimum": 0,
                    "title": "Max Length",
                    "type": "integer"
                },
                "raw": {
                    "default": false,
                    "description": "Get the actual HTML content of the requested page, without simplification.",
                    "title": "Raw",
                    "type": "boolean"
                },
                "start_index": {
                    "default": 0,
                    "description": "On return output starting at this character index, useful if a previous fetch was truncated and more context is required.",
                    "minimum": 0,
                    "title": "Start Index",
                    "type": "integer"
                },
                "url": {
                    "description": "URL to fetch",
                    "format": "uri",
                    "minLength": 1,
                    "title": "Url",
                    "type": "string"
                }
            },
            "required": ["url"],
            "title": "Fetch",
            "type": "object"
        }),
    );

    let request = ChatRequest {
        model: "models/gemini-2.5-flash".to_string(),
        messages: vec![ChatMessage::user(
            "Use the fetch tool to get the content from https://httpbin.org/get and return the result. You MUST use the available fetch tool - do not make assumptions about the content."
        )],
        tools: vec![tool],
        tool_choice: Some(ToolChoice::Auto),
        max_tokens: Some(4096),
        ..Default::default()
    };

    println!("Sending request to Gemini...\n");

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Stream request failed");

    let mut tool_calls = Vec::new();
    let mut text_response = String::new();
    let mut chunk_count = 0;

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                chunk_count += 1;
                println!("Chunk {}: {} content deltas", chunk_count, chunk.content.len());

                for (i, delta) in chunk.content.iter().enumerate() {
                    match delta {
                        ai_providers::ContentBlockDelta::TextDelta { delta, .. } => {
                            println!("  Delta {}: TextDelta ({} chars)", i, delta.len());
                            text_response.push_str(delta);
                        }
                        ai_providers::ContentBlockDelta::ToolUseDelta { id, name, input_delta, .. } => {
                            println!("  Delta {}: ToolUseDelta", i);
                            println!("    id: {:?}", id);
                            println!("    name: {:?}", name);
                            println!("    input_delta: {:?}", input_delta);
                            if let (Some(id), Some(name), Some(input)) = (id, name, input_delta) {
                                tool_calls.push((id.clone(), name.clone(), input.clone()));
                            }
                        }
                        _ => {
                            println!("  Delta {}: Other", i);
                        }
                    }
                }
            }
            Err(e) => {
                println!("ERROR: Stream error: {:?}", e);
                break;
            }
        }
    }

    println!("\n=== RESULTS ===");
    println!("Total chunks: {}", chunk_count);
    println!("Tool calls: {}", tool_calls.len());
    println!("Text response length: {} chars", text_response.len());

    if !tool_calls.is_empty() {
        println!("\n=== TOOL CALLS ===");
        for (id, name, input) in &tool_calls {
            println!("ID: {}", id);
            println!("Name: {}", name);
            println!("Input: {}", input);
        }
    } else {
        println!("\n⚠️ NO TOOL CALLS GENERATED");
    }

    if !text_response.is_empty() {
        println!("\n=== TEXT RESPONSE ===");
        println!("{}", text_response);
    }
}
