// Minimal test to debug Gemini streaming in server context
use ai_providers::{models::*, Provider};
use futures::StreamExt;
use serde_json::json;

#[tokio::test]
#[ignore]
async fn test_gemini_minimal() {
    // Get API key from environment
    let api_key = std::env::var("GEMINI_API_KEY")
        .or_else(|_| std::env::var("GOOGLE_AI_API_KEY"))
        .expect("GEMINI_API_KEY or GOOGLE_AI_API_KEY must be set");

    eprintln!("===== CREATING PROVIDER =====");
    let provider = Provider::new("gemini", &api_key, "")
        .expect("Failed to create Gemini provider");

    eprintln!("===== BUILDING REQUEST =====");
    let request = ChatRequest {
        model: "gemini-2.0-flash-exp".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: vec![MessageContent::Text {
                text: "Say 'hello' and use the fetch tool to get https://httpbin.org/get".to_string(),
            }],
        }],
        temperature: Some(0.7),
        max_tokens: Some(1000),
        tools: vec![Tool::function(
            "fetch",
            "Fetches a URL",
            json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "URL to fetch"
                    }
                },
                "required": ["url"]
            }),
        )],
        tool_choice: Some(ToolChoice::Auto),
        ..Default::default()
    };

    eprintln!("===== CALLING CHAT_STREAM =====");
    let mut stream = provider.chat_stream(request).await
        .expect("Failed to create stream");

    eprintln!("===== ITERATING STREAM =====");
    let mut chunk_count = 0;
    while let Some(chunk_result) = stream.next().await {
        eprintln!("===== RECEIVED CHUNK #{} =====", chunk_count + 1);
        match chunk_result {
            Ok(chunk) => {
                chunk_count += 1;
                eprintln!("Chunk #{}: {} deltas, finish={:?}",
                    chunk_count, chunk.content.len(), chunk.finish_reason);
            }
            Err(e) => {
                eprintln!("ERROR: {}", e);
                panic!("Stream error: {}", e);
            }
        }
    }

    eprintln!("===== STREAM COMPLETE: {} chunks =====", chunk_count);
    assert!(chunk_count > 0, "Expected at least 1 chunk");
}
