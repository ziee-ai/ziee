//! Anthropic provider implementation (custom implementation based on anthropic-sdk reference)

use crate::{
    error::ProviderError,
    models::{
        ChatMessage, ChatRequest, EmbeddingsRequest, EmbeddingsResponse,
        Role, StreamChatChunk, Tool, ToolChoice, FileUpload, FileUploadResponse,
        DocumentSource,
    },
    traits::AIProvider,
};
use async_stream::stream;
use async_trait::async_trait;
use futures_core::Stream;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::pin::Pin;

/// Anthropic REST API version, sent as the required `anthropic-version` header
/// on every request (chat, file upload/delete, and model discovery). Anthropic
/// returns 400 when it is missing. Single source of truth so no call site keeps
/// a divergent copy of the value.
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic provider (zero-sized, stateless)
pub struct AnthropicProvider;

/// Anthropic API message format
#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: AnthropicMessageContent,
}

/// Anthropic message content (can be string or array for multimodal)
#[derive(Serialize)]
#[serde(untagged)]
enum AnthropicMessageContent {
    Text(String),
    Multimodal(Vec<AnthropicContentBlock>),
}

/// Anthropic content block for multimodal messages
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicContentBlock {
    Text {
        text: String,
    },
    /// Extended-thinking block replayed on a later turn — `signature` is required
    /// (Anthropic rejects a thinking block without it).
    Thinking {
        thinking: String,
        signature: String,
    },
    RedactedThinking {
        data: String,
    },
    Image {
        source: AnthropicImageSource,
    },
    Document {
        source: AnthropicDocumentSource,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: AnthropicToolResultContent,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

/// `tool_result.content` is a string-or-array union in the Anthropic API.
/// We emit the array form when the result carries non-text blocks (images).
#[derive(Serialize)]
#[serde(untagged)]
enum AnthropicToolResultContent {
    Text(String),
    Blocks(Vec<AnthropicToolResultBlock>),
}

/// Blocks allowed inside a `tool_result.content` array (text + image).
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicToolResultBlock {
    Text { text: String },
    Image { source: AnthropicImageSource },
}

#[derive(Serialize)]
struct AnthropicImageSource {
    #[serde(rename = "type")]
    source_type: String, // "base64", "url", or "file"
    #[serde(skip_serializing_if = "Option::is_none")]
    media_type: Option<String>,  // "image/jpeg", "image/png", etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<String>,        // base64 encoded image data or URL
    #[serde(skip_serializing_if = "Option::is_none")]
    file_id: Option<String>,     // Anthropic file ID (from Files API)
}

#[derive(Serialize)]
struct AnthropicDocumentSource {
    #[serde(rename = "type")]
    source_type: String, // "base64", "url", or "file"
    #[serde(skip_serializing_if = "Option::is_none")]
    media_type: Option<String>,  // "application/pdf", etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<String>,        // base64 encoded document data
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,         // URL to document
    #[serde(skip_serializing_if = "Option::is_none")]
    file_id: Option<String>,     // Anthropic file ID (from Files API)
}

/// Anthropic streaming chunk
#[derive(Deserialize)]
struct AnthropicStreamChunk {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    delta: Option<AnthropicDelta>,
    #[serde(default)]
    error: Option<AnthropicStreamError>,
    #[serde(default)]
    message: Option<AnthropicMessageDelta>,
    #[serde(default)]
    usage: Option<AnthropicStreamUsage>,
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    content_block: Option<AnthropicContentBlockStart>,
}

/// Content block for content_block_start event
#[derive(Deserialize)]
struct AnthropicContentBlockStart {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    /// Present on a `redacted_thinking` block start.
    #[serde(default)]
    data: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicDelta {
    #[serde(rename = "type")]
    delta_type: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    thinking: Option<String>,
    #[serde(default)]
    partial_json: Option<String>,
    /// Present on a `signature_delta`.
    #[serde(default)]
    signature: Option<String>,
}

/// Error in streaming response
#[derive(Deserialize)]
struct AnthropicStreamError {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

/// Message delta for stop reason and usage
#[derive(Deserialize)]
struct AnthropicMessageDelta {
    #[serde(default)]
    stop_reason: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    stop_sequence: Option<String>,
}

/// Usage metadata in streaming response
#[derive(Deserialize)]
struct AnthropicStreamUsage {
    #[serde(default)]
    input_tokens: Option<u32>,
    #[serde(default)]
    output_tokens: Option<u32>,
    #[serde(default)]
    cache_read_input_tokens: Option<u32>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u32>,
}

impl AnthropicProvider {
    /// Map a unified image source to the Anthropic shape (base64 / url / file).
    fn convert_image_source(source: &crate::models::ImageSource) -> AnthropicImageSource {
        use crate::models::ImageSource;
        match source {
            ImageSource::Base64 { media_type, data } => AnthropicImageSource {
                source_type: "base64".to_string(),
                media_type: Some(media_type.clone()),
                data: Some(data.clone()),
                file_id: None,
            },
            ImageSource::Url { url, .. } => AnthropicImageSource {
                source_type: "url".to_string(),
                media_type: None,
                data: Some(url.clone()),
                file_id: None,
            },
            ImageSource::File { file_id, media_type } => AnthropicImageSource {
                source_type: "file".to_string(),
                media_type: media_type.clone(),
                data: None,
                file_id: Some(file_id.clone()),
            },
        }
    }

    /// Build a `tool_result.content` value from nested blocks. Text-only results
    /// use the string form (back-compat); results carrying images use the array
    /// form (Anthropic `tool_result` natively accepts `[text, image]`).
    fn convert_tool_result_content(
        content: &[crate::models::ContentBlock],
    ) -> AnthropicToolResultContent {
        use crate::models::ContentBlock;
        let mut blocks: Vec<AnthropicToolResultBlock> = Vec::new();
        let mut has_non_text = false;
        for b in content {
            match b {
                ContentBlock::Text { text } => {
                    blocks.push(AnthropicToolResultBlock::Text { text: text.clone() })
                }
                ContentBlock::Image { source } => {
                    has_non_text = true;
                    blocks.push(AnthropicToolResultBlock::Image {
                        source: Self::convert_image_source(source),
                    });
                }
                _ => {} // other nested block types are not representable in a tool_result
            }
        }
        if has_non_text {
            AnthropicToolResultContent::Blocks(blocks)
        } else {
            let text = blocks
                .into_iter()
                .filter_map(|b| match b {
                    AnthropicToolResultBlock::Text { text } => Some(text),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            AnthropicToolResultContent::Text(text)
        }
    }

    /// Converts our messages to Anthropic format
    fn convert_messages(msgs: &[ChatMessage]) -> (Option<String>, Vec<AnthropicMessage>) {
        use crate::models::ContentBlock;

        let mut system_message = None;
        let mut messages = Vec::new();

        for msg in msgs.iter() {
            match msg.role {
                Role::System => {
                    // Extract text from content blocks for system message
                    let text = msg
                        .content
                        .iter()
                        .filter_map(|block| match block {
                            ContentBlock::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !text.is_empty() {
                        // CONCATENATE multiple Role::System messages
                        // instead of overwriting. Anthropic's API has
                        // a single `system` field, so when the caller
                        // supplies multiple system messages (e.g. the
                        // assistant extension + the project extension
                        // both inject — Plan 5 §4 stacking) we need
                        // to merge them. Without this, the LAST system
                        // message would silently clobber earlier ones,
                        // and the assistant's persona instructions
                        // would never reach the model.
                        system_message = Some(match system_message {
                            Some(prev) => format!("{prev}\n\n{text}"),
                            None => text,
                        });
                    }
                }
                Role::User | Role::Tool => {
                    // Convert content blocks to Anthropic format
                    let mut anthropic_blocks = Vec::new();

                    for block in &msg.content {
                        match block {
                            ContentBlock::Text { text } => {
                                anthropic_blocks.push(AnthropicContentBlock::Text {
                                    text: text.clone(),
                                });
                            }
                            ContentBlock::Image { source } => {
                                anthropic_blocks.push(AnthropicContentBlock::Image {
                                    source: Self::convert_image_source(source),
                                });
                            }
                            ContentBlock::Thinking { thinking, signature } => {
                                // Replay a thinking block only when we have its
                                // signature — a signature-less thinking block is
                                // rejected by Anthropic, so omit it.
                                if let Some(sig) = signature {
                                    anthropic_blocks.push(AnthropicContentBlock::Thinking {
                                        thinking: thinking.clone(),
                                        signature: sig.clone(),
                                    });
                                }
                            }
                            ContentBlock::RedactedThinking { data } => {
                                anthropic_blocks.push(AnthropicContentBlock::RedactedThinking {
                                    data: data.clone(),
                                });
                            }
                            ContentBlock::ToolUse { id, name, input } => {
                                // Anthropic uses tool_use content blocks
                                anthropic_blocks.push(AnthropicContentBlock::ToolUse {
                                    id: id.clone(),
                                    name: name.clone(),
                                    input: input.clone(),
                                });
                            }
                            ContentBlock::ToolResult {
                                tool_use_id,
                                name: _,
                                content,
                                is_error,
                            } => {
                                anthropic_blocks.push(AnthropicContentBlock::ToolResult {
                                    tool_use_id: tool_use_id.clone(),
                                    content: Self::convert_tool_result_content(content),
                                    is_error: *is_error,
                                });
                            }
                            ContentBlock::Document { source } => {
                                let anthropic_source = match source {
                                    DocumentSource::Base64 { media_type, data } => {
                                        AnthropicDocumentSource {
                                            source_type: "base64".to_string(),
                                            media_type: Some(media_type.clone()),
                                            data: Some(data.clone()),
                                            url: None,
                                            file_id: None,
                                        }
                                    }
                                    DocumentSource::Url { url } => {
                                        AnthropicDocumentSource {
                                            source_type: "url".to_string(),
                                            media_type: None,
                                            data: None,
                                            url: Some(url.clone()),
                                            file_id: None,
                                        }
                                    }
                                    DocumentSource::File { file_id, media_type } => {
                                        AnthropicDocumentSource {
                                            source_type: "file".to_string(),
                                            media_type: media_type.clone(),
                                            data: None,
                                            url: None,
                                            file_id: Some(file_id.clone()),
                                        }
                                    }
                                };
                                anthropic_blocks.push(AnthropicContentBlock::Document {
                                    source: anthropic_source,
                                });
                            }
                        }
                    }

                    // Skip a message that produced no content blocks — Anthropic
                    // rejects empty content (e.g. a turn whose only block was an
                    // unsigned thinking block, which we omit).
                    if anthropic_blocks.is_empty() {
                        continue;
                    }
                    // Build message content
                    let content = if anthropic_blocks.len() == 1
                        && matches!(anthropic_blocks[0], AnthropicContentBlock::Text { .. })
                    {
                        // Single text block - use string format
                        if let Some(AnthropicContentBlock::Text { text }) =
                            anthropic_blocks.into_iter().next()
                        {
                            AnthropicMessageContent::Text(text)
                        } else {
                            AnthropicMessageContent::Text(String::new())
                        }
                    } else {
                        // Multiple blocks or non-text - use array format
                        AnthropicMessageContent::Multimodal(anthropic_blocks)
                    };

                    messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content,
                    });
                }
                Role::Assistant => {
                    // Convert content blocks
                    let mut anthropic_blocks = Vec::new();

                    for block in &msg.content {
                        match block {
                            ContentBlock::Text { text } => {
                                anthropic_blocks.push(AnthropicContentBlock::Text {
                                    text: text.clone(),
                                });
                            }
                            ContentBlock::Thinking { thinking, signature } => {
                                // Thinking blocks must precede tool_use in the
                                // assistant turn, and need their signature to
                                // replay. Omit when the signature is absent.
                                if let Some(sig) = signature {
                                    anthropic_blocks.push(AnthropicContentBlock::Thinking {
                                        thinking: thinking.clone(),
                                        signature: sig.clone(),
                                    });
                                }
                            }
                            ContentBlock::RedactedThinking { data } => {
                                anthropic_blocks.push(AnthropicContentBlock::RedactedThinking {
                                    data: data.clone(),
                                });
                            }
                            ContentBlock::ToolUse { id, name, input } => {
                                anthropic_blocks.push(AnthropicContentBlock::ToolUse {
                                    id: id.clone(),
                                    name: name.clone(),
                                    input: input.clone(),
                                });
                            }
                            _ => {} // Skip other types for assistant messages
                        }
                    }

                    // Skip an assistant turn that produced no content blocks.
                    if anthropic_blocks.is_empty() {
                        continue;
                    }
                    let content = if anthropic_blocks.len() == 1
                        && matches!(anthropic_blocks[0], AnthropicContentBlock::Text { .. })
                    {
                        if let Some(AnthropicContentBlock::Text { text }) =
                            anthropic_blocks.into_iter().next()
                        {
                            AnthropicMessageContent::Text(text)
                        } else {
                            AnthropicMessageContent::Text(String::new())
                        }
                    } else {
                        AnthropicMessageContent::Multimodal(anthropic_blocks)
                    };

                    messages.push(AnthropicMessage {
                        role: "assistant".to_string(),
                        content,
                    });
                }
            }
        }

        (system_message, messages)
    }

    /// Converts our tools to Anthropic format
    fn convert_tools(tools: &[Tool]) -> Vec<serde_json::Value> {
        tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.function.name,
                    "description": t.function.description,
                    "input_schema": t.function.parameters,
                })
            })
            .collect()
    }

    /// Converts our tool choice to Anthropic format
    fn convert_tool_choice(choice: &ToolChoice) -> serde_json::Value {
        match choice {
            ToolChoice::Auto => json!({"type": "auto"}),
            ToolChoice::Required => json!({"type": "any"}),
            ToolChoice::Specific { function, .. } => {
                json!({"type": "tool", "name": function.name})
            }
        }
    }

    /// True when the model rejects sampling params (`temperature`/`top_p`/`top_k`)
    /// — Opus 4.7/4.8 family, per the model registry. Unknown models default to
    /// allowed (back-compat); a brand-new restricted model 400s until registered.
    fn sampling_restricted(model: &str) -> bool {
        crate::model_registry::lookup("anthropic", model)
            .and_then(|c| c.supports_sampling_params)
            .map(|ok| !ok)
            .unwrap_or(false)
    }

    /// Map a unified effort level to the Anthropic `output_config.effort` value.
    /// `Dynamic` → `None` (let adaptive thinking decide).
    fn effort_str(effort: Option<crate::models::ThinkingEffort>) -> Option<&'static str> {
        use crate::models::ThinkingEffort;
        match effort? {
            ThinkingEffort::Minimal | ThinkingEffort::Low => Some("low"),
            ThinkingEffort::Medium => Some("medium"),
            ThinkingEffort::High => Some("high"),
            ThinkingEffort::XHigh => Some("xhigh"),
            ThinkingEffort::Max => Some("max"),
            ThinkingEffort::Dynamic => None,
        }
    }

    /// Put a cache breakpoint on the last content block of the last message so the
    /// growing conversation prefix is cached for the next request. Converts a
    /// string-content message to a single text block to carry `cache_control`.
    fn apply_message_cache_breakpoint(body: &mut serde_json::Value) {
        let Some(messages) = body.get_mut("messages").and_then(|m| m.as_array_mut()) else {
            return;
        };
        let Some(last_msg) = messages.last_mut() else { return };
        let Some(content) = last_msg.get_mut("content") else { return };
        match content {
            serde_json::Value::String(s) => {
                if s.is_empty() {
                    return; // don't synthesize an empty cached text block
                }
                let text = std::mem::take(s);
                *content = json!([{
                    "type": "text",
                    "text": text,
                    "cache_control": { "type": "ephemeral" }
                }]);
            }
            serde_json::Value::Array(arr) => {
                if let Some(obj) = arr.last_mut().and_then(|v| v.as_object_mut()) {
                    obj.insert("cache_control".to_string(), json!({ "type": "ephemeral" }));
                }
            }
            _ => {}
        }
    }

    /// Pure request-body assembly (no HTTP), so the wire shape is unit-testable.
    /// `stream` toggles `"stream": true`.
    fn build_request_body(request: &ChatRequest, stream: bool) -> serde_json::Value {
        use crate::models::ThinkingMode;

        let (system, messages) = Self::convert_messages(&request.messages);

        let mut body = json!({
            "model": request.model,
            "max_tokens": request.max_tokens.unwrap_or(1024),
            "messages": messages,
        });
        if stream {
            body["stream"] = json!(true);
        }

        let sampling_ok = !Self::sampling_restricted(&request.model);
        let cache_on = !request.disable_prompt_cache;

        // System prompt. Cache the stable tools+system prefix by rendering system
        // as a text-block array with a breakpoint on the last block.
        if let Some(system_msg) = system {
            if cache_on {
                body["system"] = json!([{
                    "type": "text",
                    "text": system_msg,
                    "cache_control": { "type": "ephemeral" }
                }]);
            } else {
                body["system"] = json!(system_msg);
            }
        }

        // Sampling params (gated for restricted models). Anthropic accepts at most
        // one of temperature / top_p — prefer temperature.
        if sampling_ok {
            match (request.temperature, request.top_p) {
                (Some(temp), _) => body["temperature"] = json!(temp),
                (None, Some(top_p)) => body["top_p"] = json!(top_p),
                (None, None) => {}
            }
            if let Some(top_k) = request.top_k {
                body["top_k"] = json!(top_k);
            }
        }

        // stop_sequences are accepted on all Anthropic models.
        if let Some(stop) = &request.stop {
            if !stop.is_empty() {
                body["stop_sequences"] = json!(stop);
            }
        }

        // End-user id -> metadata.user_id.
        if let Some(user) = &request.user {
            body["metadata"] = json!({ "user_id": user });
        }

        if !request.tools.is_empty() {
            body["tools"] = json!(Self::convert_tools(&request.tools));
        }
        if let Some(tool_choice) = &request.tool_choice {
            body["tool_choice"] = Self::convert_tool_choice(tool_choice);
        }

        // Thinking.
        if let Some(thinking) = &request.thinking {
            match thinking.mode {
                ThinkingMode::Disabled => {}
                ThinkingMode::Adaptive => {
                    let display = if thinking.include_thinking {
                        "summarized"
                    } else {
                        "omitted"
                    };
                    body["thinking"] = json!({ "type": "adaptive", "display": display });
                    if let Some(effort) = Self::effort_str(thinking.effort) {
                        body["output_config"] = json!({ "effort": effort });
                    }
                }
                ThinkingMode::Enabled => {
                    let budget = thinking.budget_tokens.unwrap_or(10000).max(1024);
                    body["thinking"] = json!({ "type": "enabled", "budget_tokens": budget });
                }
            }
        }

        // History cache breakpoint (≤2 of 4 breakpoints total).
        if cache_on {
            Self::apply_message_cache_breakpoint(&mut body);
        }

        body
    }

}

#[async_trait]
impl AIProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "Anthropic"
    }

    async fn stream_chat(
        &self,
        api_key: &str,
        base_url: &str,
        request: ChatRequest,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<StreamChatChunk, ProviderError>> + Send>>,
        ProviderError,
    > {
        let client = super::http_client();

        // Build the request body (pure, unit-testable).
        let body = Self::build_request_body(&request, true);

        // Make streaming request
        let response = client
            .post(format!("{}/messages", base_url))
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("anthropic-beta", "files-api-2025-04-14")  // Enable Files API beta
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        // Check status
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ProviderError::from_status_code(status.as_u16(), error_text));
        }

        // Get byte stream
        let byte_stream = response.bytes_stream();

        // Create SSE parser stream
        let output_stream = stream! {
            let mut buffer = String::new();
            let mut decoder = super::Utf8StreamDecoder::default();
            let mut byte_stream = Box::pin(byte_stream);

            // Usage accumulators: Anthropic reports input + cache tokens on
            // `message_start` and the final output tokens on `message_delta`.
            let mut usage_input: u32 = 0;
            let mut usage_cache_read: Option<u32> = None;
            let mut usage_cache_creation: Option<u32> = None;

            while let Some(chunk_result) = byte_stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        // Decode incrementally so a multi-byte UTF-8 char split
                        // across chunk boundaries doesn't abort the stream.
                        buffer.push_str(&decoder.decode(&chunk));

                        // Process complete SSE events (Anthropic format: "event: ...\ndata: {...}\n\n")
                        while let Some(index) = buffer.find("\n\n") {
                            let event_block = buffer[..index].to_string();
                            buffer.drain(..=index + 1);

                            // Extract data line from event block
                            for line in event_block.lines() {
                                if line.starts_with("data: ") {
                                    let data = &line[6..]; // Skip "data: "

                                    if data == "[DONE]" {
                                        break;
                                    }

                                    // Truncate at a char boundary for trace logging.
                                    let truncated = data.char_indices().nth(200).map(|(i, _)| &data[..i]).unwrap_or(data);
                                    tracing::trace!("Anthropic SSE event: {}", truncated);

                                    // Try to parse as JSON
                                    if let Ok(chunk_data) = serde_json::from_str::<AnthropicStreamChunk>(data) {
                                        tracing::trace!("Anthropic: Parsed event type: {}", chunk_data.event_type);
                                        // Handle error events
                                        if chunk_data.event_type == "error" {
                                            if let Some(error) = chunk_data.error {
                                                yield Err(ProviderError::from_anthropic_error(
                                                    &error.error_type,
                                                    &error.message
                                                ));
                                                break;
                                            }
                                        }

                                        // Capture input + cache tokens from message_start.
                                        if chunk_data.event_type == "message_start" {
                                            if let Some(stream_usage) = chunk_data.usage.as_ref() {
                                                usage_input = stream_usage.input_tokens.unwrap_or(0);
                                                usage_cache_read = stream_usage.cache_read_input_tokens;
                                                usage_cache_creation = stream_usage.cache_creation_input_tokens;
                                            }
                                        }

                                        // Handle message_delta for usage and finish reason
                                        if chunk_data.event_type == "message_delta" {
                                            let mut finish_reason = None;
                                            let mut usage = None;

                                            if let Some(message_delta) = chunk_data.message {
                                                finish_reason = message_delta.stop_reason;
                                            }

                                            if let Some(stream_usage) = chunk_data.usage {
                                                let output = stream_usage.output_tokens.unwrap_or(0);
                                                // Prefer the delta's input if present, else the value
                                                // captured at message_start.
                                                let delta_input = stream_usage.input_tokens.unwrap_or(0);
                                                let input = if delta_input > 0 { delta_input } else { usage_input };
                                                usage = Some(crate::models::StreamUsage {
                                                    prompt_tokens: input,
                                                    completion_tokens: output,
                                                    total_tokens: input + output,
                                                    reasoning_tokens: None,
                                                    cache_read_input_tokens: usage_cache_read
                                                        .or(stream_usage.cache_read_input_tokens),
                                                    cache_creation_input_tokens: usage_cache_creation
                                                        .or(stream_usage.cache_creation_input_tokens),
                                                });
                                            }

                                            if finish_reason.is_some() || usage.is_some() {
                                                yield Ok(StreamChatChunk {
                                                    content: Vec::new(),
                                                    finish_reason,
                                                    usage,
                                                    refusal: None,
                                                    safety_ratings: Vec::new(),
                                                    safety_blocked: false,
                                                });
                                            }
                                        }

                                        // Handle content_block_start for tool_use
                                        if chunk_data.event_type == "content_block_start" {
                                            tracing::trace!("Anthropic: Processing content_block_start, has content_block: {}, block_type: {:?}",
                                                chunk_data.content_block.is_some(),
                                                chunk_data.content_block.as_ref().map(|b| b.block_type.as_str()));

                                            if let Some(content_block) = chunk_data.content_block {
                                                if content_block.block_type == "tool_use" {
                                                    let index = chunk_data.index.unwrap_or(0);
                                                    tracing::trace!(
                                                        "Anthropic: Tool use start at index {}: id={:?}, name={:?}",
                                                        index,
                                                        content_block.id,
                                                        content_block.name
                                                    );

                                                    yield Ok(StreamChatChunk {
                                                        content: vec![crate::models::ContentBlockDelta::ToolUseDelta {
                                                            index,
                                                            id: content_block.id,
                                                            name: content_block.name,
                                                            input_delta: None,
                                                        }],
                                                        finish_reason: None,
                                                        usage: None,
                                                        refusal: None,
                                                        safety_ratings: Vec::new(),
                                                        safety_blocked: false,
                                                    });
                                                } else if content_block.block_type == "redacted_thinking" {
                                                    if let Some(data) = content_block.data {
                                                        let index = chunk_data.index.unwrap_or(0);
                                                        yield Ok(StreamChatChunk {
                                                            content: vec![crate::models::ContentBlockDelta::RedactedThinkingDelta {
                                                                index,
                                                                data,
                                                            }],
                                                            finish_reason: None,
                                                            usage: None,
                                                            refusal: None,
                                                            safety_ratings: Vec::new(),
                                                            safety_blocked: false,
                                                        });
                                                    }
                                                }
                                            }
                                        }

                                        // Handle content_block_delta
                                        if chunk_data.event_type == "content_block_delta" {
                                            if let Some(delta) = chunk_data.delta {
                                                let index = chunk_data.index.unwrap_or(0);
                                                let content_delta = match delta.delta_type.as_str() {
                                                    "text_delta" => {
                                                        delta.text.map(|text| {
                                                            crate::models::ContentBlockDelta::TextDelta {
                                                                index,
                                                                delta: text,
                                                            }
                                                        })
                                                    }
                                                    "thinking_delta" => {
                                                        delta.thinking.map(|thinking| {
                                                            crate::models::ContentBlockDelta::ThinkingDelta {
                                                                index,
                                                                delta: thinking,
                                                            }
                                                        })
                                                    }
                                                    "signature_delta" => {
                                                        delta.signature.map(|signature| {
                                                            crate::models::ContentBlockDelta::ThinkingSignatureDelta {
                                                                index,
                                                                signature,
                                                            }
                                                        })
                                                    }
                                                    "input_json_delta" => {
                                                        delta.partial_json.map(|partial_json| {
                                                            crate::models::ContentBlockDelta::ToolUseDelta {
                                                                index,
                                                                id: None,
                                                                name: None,
                                                                input_delta: Some(partial_json),
                                                            }
                                                        })
                                                    }
                                                    _ => None, // Ignore unknown delta types
                                                };

                                                if let Some(delta) = content_delta {
                                                    yield Ok(StreamChatChunk {
                                                        content: vec![delta],
                                                        finish_reason: None,
                                                        usage: None,
                                                        refusal: None,
                                                        safety_ratings: Vec::new(),
                                                        safety_blocked: false,
                                                    });
                                                }
                                            }
                                        }
                                    }
                                    break; // Only process first data line in block
                                }
                            }
                        }

                        // Guard against an upstream that never emits an event
                        // delimiter (would otherwise grow `buffer` until OOM).
                        if buffer.len() > super::MAX_SSE_BUFFER_BYTES {
                            yield Err(ProviderError::streaming(
                                "Anthropic: SSE buffer exceeded maximum size",
                            ));
                            break;
                        }
                    }
                    Err(e) => {
                        yield Err(ProviderError::Network(e));
                        break;
                    }
                }
            }
        };

        Ok(Box::pin(output_stream))
    }

    async fn embeddings(
        &self,
        _api_key: &str,
        _base_url: &str,
        _request: EmbeddingsRequest,
    ) -> Result<EmbeddingsResponse, ProviderError> {
        // Anthropic doesn't support embeddings API
        Err(ProviderError::not_supported(
            "Anthropic does not support embeddings",
        ))
    }

    async fn upload_file(
        &self,
        api_key: &str,
        base_url: &str,
        upload: FileUpload,
    ) -> Result<Option<FileUploadResponse>, ProviderError> {
        // Dedicated SSRF-guarded client (connect-time DNS-rebind guard +
        // no_proxy): the request carries the provider api_key to a
        // user-configured base_url. See `providers::upload_client`.
        let client = super::upload_http_client();

        // Create multipart form with file data
        let file_part = reqwest::multipart::Part::bytes(upload.file_data)
            .file_name(upload.filename.clone())
            .mime_str(&upload.mime_type)
            .map_err(|e| ProviderError::InvalidRequest(format!("Invalid MIME type: {}", e)))?;

        let form = reqwest::multipart::Form::new().part("file", file_part);

        // Upload to Anthropic Files API
        let response = client
            .post(format!("{}/files", base_url))
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("anthropic-beta", "files-api-2025-04-14")
            .multipart(form)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e))?;

        // Check status
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ProviderError::from_status_code(status.as_u16(), error_text));
        }

        // Parse response
        #[derive(Deserialize)]
        struct AnthropicFileUploadResponse {
            id: String,
            #[serde(rename = "type")]
            file_type: String,
            filename: String,
            size_bytes: u64,
        }

        let upload_response: AnthropicFileUploadResponse = response.json().await
            .map_err(|e| ProviderError::file_upload(format!("Failed to parse upload response: {}", e)))?;

        Ok(Some(FileUploadResponse {
            provider_file_id: upload_response.id,
            expires_at: None,  // Anthropic files don't expire
            metadata: Some(json!({
                "filename": upload_response.filename,
                "file_type": upload_response.file_type,
                "size_bytes": upload_response.size_bytes,
                "mime_type": upload.mime_type,
            })),
        }))
    }

    fn supports_file_api(&self) -> bool {
        true
    }

    fn file_expiration(&self) -> Option<chrono::Duration> {
        None  // Anthropic files don't expire
    }

    async fn delete_file(
        &self,
        api_key: &str,
        base_url: &str,
        provider_file_id: &str,
    ) -> Result<(), ProviderError> {
        // Dedicated SSRF-guarded client (connect-time DNS-rebind guard +
        // no_proxy): like upload_file, this request carries the provider
        // api_key to a user-configured base_url. See `providers::upload_client`.
        let client = super::upload_http_client();

        let response = client
            .delete(format!("{}/files/{}", base_url, provider_file_id))
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("anthropic-beta", "files-api-2025-04-14")
            .send()
            .await
            .map_err(|e| ProviderError::Network(e))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ProviderError::from_status_code(status.as_u16(), error_text));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    /// The SSE-event debug log truncates at 200 bytes via `char_indices().nth(200)`
    /// so the slice always ends on a UTF-8 char boundary. Slicing with `&data[..200]`
    /// would panic when the 200-byte cut falls inside a multi-byte character.
    #[test]
    fn test_sse_log_truncation_respects_utf8_char_boundary() {
        // Build a string where byte 200 lands inside a multi-byte UTF-8 char.
        // 198 ASCII bytes + a 4-byte emoji "😀" (U+1F600) → byte 198..202 is the emoji.
        let data: String = "a".repeat(198) + "\u{1F600}" + "rest";

        // The exact expression used in chat_stream
        let truncated = data
            .char_indices()
            .nth(200)
            .map(|(i, _)| &data[..i])
            .unwrap_or(&data);

        // Must not panic and must be valid UTF-8 (free, since it's &str)
        assert!(truncated.is_char_boundary(truncated.len()));
        // And it must include at least the 198 'a's (the cut happens at char 200, which
        // includes the emoji at char index 198).
        assert!(truncated.len() >= 198);
    }

    /// Sanity: the previous incorrect approach `&data[..200]` would panic on the
    /// same input. We don't run it (would actually panic), but we document the
    /// motivation here.
    #[test]
    fn test_sse_log_truncation_handles_pure_ascii() {
        let data = "x".repeat(500);
        let truncated = data
            .char_indices()
            .nth(200)
            .map(|(i, _)| &data[..i])
            .unwrap_or(&data);
        assert_eq!(truncated.len(), 200);
    }

    /// Strings shorter than 200 chars must return the whole string (no truncation).
    #[test]
    fn test_sse_log_truncation_short_string_unchanged() {
        let data = "short";
        let truncated = data
            .char_indices()
            .nth(200)
            .map(|(i, _)| &data[..i])
            .unwrap_or(data);
        assert_eq!(truncated, "short");
    }

    /// Tier-1 wire-shape tests for the pure `build_request_body`.
    mod build_body {
        use super::super::AnthropicProvider;
        use crate::models::{
            ChatMessage, ChatRequest, ContentBlock, ImageSource, Role, ThinkingConfig,
            ThinkingEffort,
        };

        fn req(model: &str) -> ChatRequest {
            ChatRequest {
                model: model.to_string(),
                messages: vec![ChatMessage::user("hi")],
                max_tokens: Some(8192),
                ..Default::default()
            }
        }

        #[test]
        fn adaptive_thinking_shape_and_effort() {
            let mut r = req("claude-opus-4-7");
            r.thinking = Some(ThinkingConfig::adaptive_with_effort(ThinkingEffort::High));
            let body = AnthropicProvider::build_request_body(&r, true);
            assert_eq!(body["thinking"]["type"], "adaptive");
            assert_eq!(body["thinking"]["display"], "summarized");
            assert_eq!(body["output_config"]["effort"], "high");
            // must-fix: never the removed {type:enabled,budget_tokens} shape.
            assert!(body["thinking"].get("budget_tokens").is_none());
        }

        #[test]
        fn enabled_thinking_uses_budget() {
            let mut r = req("claude-3-5-sonnet");
            r.thinking = Some(ThinkingConfig::with_budget(4096));
            let body = AnthropicProvider::build_request_body(&r, true);
            assert_eq!(body["thinking"]["type"], "enabled");
            assert_eq!(body["thinking"]["budget_tokens"], 4096);
        }

        #[test]
        fn disabled_thinking_omits() {
            let mut r = req("claude-opus-4-7");
            r.thinking = Some(ThinkingConfig::disabled());
            let body = AnthropicProvider::build_request_body(&r, true);
            assert!(body.get("thinking").is_none());
        }

        #[test]
        fn opus_47_omits_sampling_params() {
            // must-fix: Opus 4.7 rejects temperature/top_p/top_k (registry-gated).
            let mut r = req("claude-opus-4-7");
            r.temperature = Some(0.7);
            r.top_p = Some(0.9);
            r.top_k = Some(40);
            let body = AnthropicProvider::build_request_body(&r, true);
            assert!(body.get("temperature").is_none());
            assert!(body.get("top_p").is_none());
            assert!(body.get("top_k").is_none());
        }

        #[test]
        fn allowed_model_sends_temperature_not_both() {
            let mut r = req("claude-3-5-sonnet"); // unknown to registry -> allowed
            r.temperature = Some(0.5);
            r.top_p = Some(0.9);
            let body = AnthropicProvider::build_request_body(&r, true);
            assert_eq!(body["temperature"], 0.5);
            assert!(body.get("top_p").is_none(), "never both temperature and top_p");
        }

        #[test]
        fn stop_sequences_and_user_metadata() {
            let mut r = req("claude-opus-4-7");
            r.stop = Some(vec!["STOP".to_string()]);
            r.user = Some("u1".to_string());
            let body = AnthropicProvider::build_request_body(&r, true);
            assert_eq!(body["stop_sequences"][0], "STOP");
            assert_eq!(body["metadata"]["user_id"], "u1");
        }

        #[test]
        fn cache_breakpoints_on_system_and_last_message() {
            let mut r = req("claude-opus-4-7");
            r.messages = vec![ChatMessage::system("persona"), ChatMessage::user("q")];
            let body = AnthropicProvider::build_request_body(&r, true);
            assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
            let last = body["messages"].as_array().unwrap().last().unwrap();
            assert_eq!(last["content"][0]["cache_control"]["type"], "ephemeral");
        }

        #[test]
        fn disable_prompt_cache_omits_breakpoints() {
            let mut r = req("claude-opus-4-7");
            r.messages = vec![ChatMessage::system("persona"), ChatMessage::user("q")];
            r.disable_prompt_cache = true;
            let body = AnthropicProvider::build_request_body(&r, true);
            assert!(body["system"].as_str().is_some());
            let last = body["messages"].as_array().unwrap().last().unwrap();
            assert!(last["content"].as_str().is_some());
        }

        #[test]
        fn thinking_block_with_signature_real_block_and_unsigned_omitted() {
            let mut r = req("claude-opus-4-7");
            r.messages = vec![ChatMessage::with_blocks(
                Role::Assistant,
                vec![
                    ContentBlock::Thinking {
                        thinking: "reasoned".into(),
                        signature: Some("sig123".into()),
                    },
                    ContentBlock::Text { text: "answer".into() },
                ],
            )];
            let body = AnthropicProvider::build_request_body(&r, true);
            let msg = &body["messages"][0];
            assert_eq!(msg["content"][0]["type"], "thinking");
            assert_eq!(msg["content"][0]["signature"], "sig123");

            // signature-less thinking is omitted (only the text remains).
            let mut r2 = req("claude-opus-4-7");
            r2.messages = vec![ChatMessage::with_blocks(
                Role::Assistant,
                vec![
                    ContentBlock::Thinking { thinking: "x".into(), signature: None },
                    ContentBlock::Text { text: "answer".into() },
                ],
            )];
            let body2 = AnthropicProvider::build_request_body(&r2, true);
            let content2 = &body2["messages"][0]["content"];
            let only_text = content2.as_str() == Some("answer")
                || (content2.is_array()
                    && content2.as_array().unwrap().len() == 1
                    && content2[0]["type"] == "text");
            assert!(only_text, "unsigned thinking must be omitted");
        }

        #[test]
        fn tool_result_with_image_renders_array() {
            let mut r = req("claude-opus-4-7");
            r.messages = vec![ChatMessage::with_blocks(
                Role::Tool,
                vec![ContentBlock::ToolResult {
                    tool_use_id: "t1".into(),
                    name: Some("screenshot".into()),
                    content: vec![
                        ContentBlock::Text { text: "see image".into() },
                        ContentBlock::Image {
                            source: ImageSource::Base64 {
                                media_type: "image/png".into(),
                                data: "abc".into(),
                            },
                        },
                    ],
                    is_error: None,
                }],
            )];
            let body = AnthropicProvider::build_request_body(&r, true);
            let tr = &body["messages"][0]["content"][0];
            assert_eq!(tr["type"], "tool_result");
            assert_eq!(tr["content"][0]["type"], "text");
            assert_eq!(tr["content"][1]["type"], "image");
        }
    }
}
