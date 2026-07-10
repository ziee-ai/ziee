//! TEST-13 — response-side adapter over a deterministic raw-TCP mock.
//!
//! Drives the FULL `stream_chat` path (HTTP send → status check → generic SSE
//! driver → per-provider `map_event`) for ≥2 providers against a canned server,
//! asserting unified deltas + canonical finish reason on a 200 SSE response and a
//! typed `ProviderError` on a 4xx error body. No network, no API keys.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use ai_providers::{AIProvider, ChatMessage, ChatRequest, ContentBlockDelta, ProviderError, Role};
use futures_util::StreamExt;

/// Spawn a one-shot HTTP server that returns `response` verbatim for the first
/// connection, and return its `http://127.0.0.1:<port>` base URL.
fn spawn_once(response: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            // Drain the request headers (best-effort) so the client can finish
            // writing before we respond.
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    format!("http://{}", addr)
}

fn user_req(model: &str) -> ChatRequest {
    ChatRequest {
        model: model.to_string(),
        messages: vec![ChatMessage {
            role: Role::User,
            content: vec![],
        }],
        ..Default::default()
    }
}

const OPENAI_SSE_200: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n\
data: {\"id\":\"c\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n\
data: {\"id\":\"c\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n\
data: {\"id\":\"c\",\"choices\":[],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2,\"total_tokens\":5}}\n\n\
data: [DONE]\n\n";

const OPENAI_ERR_400: &str = "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: 118\r\n\r\n\
{\"error\":{\"message\":\"Unsupported parameter: temperature\",\"type\":\"invalid_request_error\",\"param\":\"temperature\",\"code\":\"x\"}}";

const ANTHROPIC_SSE_200: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n\
event: message_start\ndata: {\"type\":\"message_start\",\"usage\":{\"input_tokens\":4}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"message\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":2}}\n\n";

async fn collect(
    provider: &dyn AIProvider,
    base_url: &str,
    req: ChatRequest,
) -> Result<Vec<ai_providers::StreamChatChunk>, ProviderError> {
    let mut stream = provider.stream_chat("k", base_url, req).await?;
    let mut out = Vec::new();
    while let Some(item) = stream.next().await {
        out.push(item?);
    }
    Ok(out)
}

#[tokio::test]
async fn openai_200_yields_unified_deltas_and_canonical_finish() {
    let base = spawn_once(OPENAI_SSE_200);
    let chunks = collect(&ai_providers::OpenAIProvider, &base, user_req("gpt-4o"))
        .await
        .expect("stream ok");

    let text: String = chunks
        .iter()
        .flat_map(|c| c.content.iter())
        .filter_map(|d| match d {
            ContentBlockDelta::TextDelta { delta, .. } => Some(delta.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(text, "Hello");

    let finish = chunks.iter().find_map(|c| c.finish_reason.clone());
    assert_eq!(finish.as_deref(), Some("stop"));

    let usage = chunks.iter().find_map(|c| c.usage.as_ref());
    assert_eq!(usage.map(|u| u.total_tokens), Some(5));
}

#[tokio::test]
async fn openai_400_maps_to_typed_error() {
    let base = spawn_once(OPENAI_ERR_400);
    let err = collect(&ai_providers::OpenAIProvider, &base, user_req("gpt-4o"))
        .await
        .expect_err("must be an error");
    assert!(
        matches!(err, ProviderError::InvalidRequest(_)),
        "expected typed InvalidRequest, got {err:?}"
    );
}

#[tokio::test]
async fn anthropic_200_yields_deltas_and_canonical_finish() {
    let base = spawn_once(ANTHROPIC_SSE_200);
    let chunks = collect(&ai_providers::AnthropicProvider, &base, user_req("claude-sonnet-4-6"))
        .await
        .expect("stream ok");

    let text: String = chunks
        .iter()
        .flat_map(|c| c.content.iter())
        .filter_map(|d| match d {
            ContentBlockDelta::TextDelta { delta, .. } => Some(delta.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(text, "Hi");

    // Anthropic "end_turn" is canonicalized to "stop".
    let finish = chunks.iter().find_map(|c| c.finish_reason.clone());
    assert_eq!(finish.as_deref(), Some("stop"));
}
