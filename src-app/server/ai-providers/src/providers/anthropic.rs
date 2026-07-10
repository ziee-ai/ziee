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
    /// — the Opus 4.7/4.8 + Claude 5 (e.g. Sonnet 5) families, per the model
    /// registry. Unknown models default to allowed (back-compat); a brand-new
    /// restricted model 400s until registered (then self-heals — see
    /// `is_unsupported_sampling_error`).
    fn sampling_restricted(model: &str) -> bool {
        crate::model_registry::lookup("anthropic", model)
            .and_then(|c| c.supports_sampling_params)
            .map(|ok| !ok)
            .unwrap_or(false)
    }

    /// Parse an Anthropic error envelope `{"error":{"type","message"}}` into its
    /// `(type, message)` pair using untyped `Value` access; `None` for a body that
    /// isn't the expected JSON shape.
    fn parse_error_envelope(body: &str) -> Option<(String, String)> {
        let v: serde_json::Value = serde_json::from_str(body).ok()?;
        let err = v.get("error")?;
        let ty = err.get("type")?.as_str()?.to_string();
        let msg = err.get("message")?.as_str()?.to_string();
        Some((ty, msg))
    }

    /// True when a 400 error `message` says a sampling param (`temperature`/
    /// `top_p`/`top_k`) is **unsupported for this model** — the only case the
    /// self-heal may repair by stripping the param and retrying.
    ///
    /// Deliberately fail-closed: an invalid-**value** error (e.g. "temperature:
    /// Input should be less than or equal to 1") names the param but is NOT
    /// repairable — stripping there would silently swallow the operator's
    /// misconfiguration and answer at the provider default. So we require an
    /// explicit "unsupported / not-permitted" indicator, not just the param name.
    fn is_unsupported_sampling_error(message: &str) -> bool {
        let m = message.to_lowercase();
        let names_param =
            m.contains("temperature") || m.contains("top_p") || m.contains("top_k");
        if !names_param {
            return false;
        }
        // Phrases Anthropic uses when a param is not accepted at all for the model
        // (restricted model, or thinking-active temperature). Kept specific so a
        // value-validation 400 that merely mentions the param isn't misread as
        // unsupported (e.g. bare "unexpected"/"not allowed" would over-match).
        const UNSUPPORTED_HINTS: [&str; 8] = [
            "not permitted",       // pydantic: "Extra inputs are not permitted"
            "extra input",         // pydantic: "Extra inputs ..."
            "not supported",       // "temperature is not supported ..."
            "unsupported",         // "unsupported_parameter" / "unsupported ..."
            "only be set to 1",    // thinking-active temperature
            "unexpected keyword",  // "unexpected keyword argument ..."
            "unexpected parameter",
            "cannot be used with", // "temperature cannot be used with thinking"
        ];
        UNSUPPORTED_HINTS.iter().any(|h| m.contains(h))
    }

    /// Remove the sampling keys from an already-built request body. Returns `true`
    /// when at least one was present (i.e. a retry could change the outcome). All
    /// three are stripped together: Anthropic's sampling support is all-or-nothing
    /// per model / thinking-state, so an "unsupported" 400 rejects the whole set.
    fn strip_sampling_params(body: &mut serde_json::Value) -> bool {
        let Some(obj) = body.as_object_mut() else {
            return false;
        };
        let mut removed = false;
        for key in ["temperature", "top_p", "top_k"] {
            removed |= obj.remove(key).is_some();
        }
        removed
    }

    /// Build a clean `ProviderError` from an Anthropic HTTP error. For a **400**
    /// with a parseable envelope, prefer the human-readable `type`+`message` (via
    /// `from_anthropic_error`, which sanitizes both) over the raw JSON blob. Other
    /// statuses keep the existing status-driven `from_status_code` mapping (also
    /// sanitized) so a non-400 isn't silently reclassified by its error `type`.
    fn clean_http_error(
        status: u16,
        parsed: Option<(String, String)>,
        raw: &str,
    ) -> ProviderError {
        match parsed {
            Some((ty, msg)) if status == 400 => {
                ProviderError::from_anthropic_error(&ty, &msg)
            }
            _ => ProviderError::from_status_code(status, raw.to_string()),
        }
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

        // Sampling params (temperature/top_p/top_k) are gated two ways:
        //  - restricted models (Opus 4.7/4.8, Sonnet 5, …) reject them outright; and
        //  - whenever thinking is active, Anthropic requires temperature == 1 and
        //    400s on any other value — it defaults to 1 when the sampling block is
        //    absent, so we omit the block entirely rather than emit a value the
        //    operator did not configure.
        let thinking_active = matches!(
            request.thinking.as_ref().map(|t| t.mode),
            Some(ThinkingMode::Adaptive | ThinkingMode::Enabled)
        );
        let sampling_ok = !Self::sampling_restricted(&request.model) && !thinking_active;
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
        let mut body = Self::build_request_body(&request, true);

        // Make streaming request. Self-heal: if the provider 400s because a
        // sampling param (temperature/top_p/top_k) is unsupported for this model
        // — e.g. a model whose requirements changed or was added manually and
        // isn't in the static registry — strip the offending params and retry
        // once. On a non-repairable failure, surface a clean, human-readable
        // provider error (parsed Anthropic type+message) instead of the raw JSON.
        let mut attempted_repair = false;
        let response = loop {
            let resp = client
                .post(format!("{}/messages", base_url))
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .header("anthropic-beta", "files-api-2025-04-14")  // Enable Files API beta
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await?;

            let status = resp.status();
            if status.is_success() {
                break resp;
            }

            let error_text = resp.text().await.unwrap_or_default();
            // Parse the error envelope once; reuse it for both the repair decision
            // and the clean surfaced error (no double parse).
            let parsed = Self::parse_error_envelope(&error_text);
            let message = parsed
                .as_ref()
                .map(|(_, m)| m.as_str())
                .unwrap_or(error_text.as_str());
            if status.as_u16() == 400
                && !attempted_repair
                && Self::is_unsupported_sampling_error(message)
                && Self::strip_sampling_params(&mut body)
            {
                attempted_repair = true;
                // `request.model` is untrusted (operator-supplied) — sanitize it
                // before logging to prevent CR/LF log-forging.
                tracing::warn!(
                    "Anthropic: 400 on unsupported sampling params for model {}; stripping temperature/top_p/top_k and retrying once",
                    crate::error::sanitize_error_body(&request.model)
                );
                continue;
            }

            return Err(Self::clean_http_error(status.as_u16(), parsed, &error_text));
        };

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
            .header("anthropic-version", "2023-06-01")
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
            .header("anthropic-version", "2023-06-01")
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

        #[test]
        fn adaptive_thinking_omits_sampling_on_allowed_model() {
            // Failure #2: with thinking active, temperature/top_p/top_k must be
            // omitted even on a sampling-ALLOWED model (Anthropic requires
            // temperature == 1 with thinking and 400s otherwise).
            let mut r = req("claude-3-5-sonnet"); // unknown to registry -> allowed
            r.thinking = Some(ThinkingConfig::adaptive_with_effort(ThinkingEffort::High));
            r.temperature = Some(0.7);
            r.top_p = Some(0.9);
            r.top_k = Some(40);
            let body = AnthropicProvider::build_request_body(&r, true);
            assert!(body.get("temperature").is_none());
            assert!(body.get("top_p").is_none());
            assert!(body.get("top_k").is_none());
            assert_eq!(body["thinking"]["type"], "adaptive");
        }

        #[test]
        fn enabled_thinking_omits_sampling_keeps_budget() {
            let mut r = req("claude-3-5-sonnet");
            r.thinking = Some(ThinkingConfig::with_budget(4096));
            r.temperature = Some(0.7);
            let body = AnthropicProvider::build_request_body(&r, true);
            assert!(body.get("temperature").is_none());
            assert_eq!(body["thinking"]["type"], "enabled");
            assert_eq!(body["thinking"]["budget_tokens"], 4096);
        }

        #[test]
        fn no_thinking_allowed_model_keeps_temperature() {
            // thinking = None on an allowed model -> temperature still sent.
            let mut r = req("claude-3-5-sonnet");
            r.temperature = Some(0.5);
            let body = AnthropicProvider::build_request_body(&r, true);
            assert_eq!(body["temperature"], 0.5);

            // explicitly-disabled thinking likewise keeps sampling.
            let mut r2 = req("claude-3-5-sonnet");
            r2.thinking = Some(ThinkingConfig::disabled());
            r2.temperature = Some(0.5);
            let body2 = AnthropicProvider::build_request_body(&r2, true);
            assert_eq!(body2["temperature"], 0.5);
        }

        #[test]
        fn sonnet_5_omits_sampling_via_registry() {
            // Failure #1: claude-sonnet-5 is registry-restricted, so sampling
            // params are dropped even without thinking.
            let mut r = req("claude-sonnet-5");
            r.temperature = Some(0.7);
            r.top_p = Some(0.9);
            r.top_k = Some(40);
            let body = AnthropicProvider::build_request_body(&r, true);
            assert!(body.get("temperature").is_none());
            assert!(body.get("top_p").is_none());
            assert!(body.get("top_k").is_none());
        }
    }

    mod self_heal {
        use super::super::AnthropicProvider;
        use crate::error::ProviderError;
        use serde_json::json;

        fn err_body(message: &str) -> String {
            format!(
                r#"{{"type":"error","error":{{"type":"invalid_request_error","message":"{message}"}}}}"#
            )
        }

        #[test]
        fn is_unsupported_sampling_error_matches_only_unsupported() {
            // Param unsupported for the model -> repairable.
            assert!(AnthropicProvider::is_unsupported_sampling_error(
                "temperature may only be set to 1 when thinking is enabled or in adaptive mode."
            ));
            assert!(AnthropicProvider::is_unsupported_sampling_error("top_p is not supported"));
            assert!(AnthropicProvider::is_unsupported_sampling_error(
                "temperature: Extra inputs are not permitted"
            ));
            // Invalid-VALUE errors name the param but must NOT be repaired —
            // stripping there would silently swallow the misconfiguration.
            assert!(!AnthropicProvider::is_unsupported_sampling_error(
                "temperature: Input should be less than or equal to 1"
            ));
            assert!(!AnthropicProvider::is_unsupported_sampling_error("temperature must be <= 1"));
            // Broad words that merely appear must NOT over-match (hints are specific).
            assert!(!AnthropicProvider::is_unsupported_sampling_error(
                "temperature triggered an unexpected internal error"
            ));
            assert!(!AnthropicProvider::is_unsupported_sampling_error(
                "temperature: value not allowed for this range"
            ));
            // Unrelated 400 (no sampling param named) -> not a repair.
            assert!(!AnthropicProvider::is_unsupported_sampling_error(
                "max_tokens: must be greater than 0"
            ));
        }

        #[test]
        fn strip_sampling_params_removes_only_sampling_keys() {
            let mut body = json!({
                "model": "claude-sonnet-5",
                "messages": [],
                "thinking": {"type": "adaptive"},
                "temperature": 0.7,
                "top_p": 0.9,
                "top_k": 40,
            });
            assert!(AnthropicProvider::strip_sampling_params(&mut body));
            assert!(body.get("temperature").is_none());
            assert!(body.get("top_p").is_none());
            assert!(body.get("top_k").is_none());
            assert_eq!(body["model"], "claude-sonnet-5");
            assert_eq!(body["thinking"]["type"], "adaptive");
            // nothing left to strip -> false (no pointless retry).
            assert!(!AnthropicProvider::strip_sampling_params(&mut body));
        }

        #[test]
        fn clean_http_error_prefers_and_sanitizes_message() {
            // Prefers the parsed human-readable message over the raw JSON blob.
            let raw = err_body("temperature must be 1");
            let parsed = AnthropicProvider::parse_error_envelope(&raw);
            match AnthropicProvider::clean_http_error(400, parsed, &raw) {
                ProviderError::InvalidRequest(m) => {
                    assert_eq!(m, "temperature must be 1");
                    assert!(!m.contains('{'), "clean message, not the raw JSON blob");
                }
                other => panic!("expected InvalidRequest, got {other:?}"),
            }

            // Untrusted message is sanitized: CR/LF collapsed + length bounded
            // (same control the raw-body from_status_code path applies).
            let long = "x".repeat(5000);
            let raw2 = err_body(&format!("bad temperature line1\\nline2 {long}"));
            let parsed2 = AnthropicProvider::parse_error_envelope(&raw2);
            match AnthropicProvider::clean_http_error(400, parsed2, &raw2) {
                ProviderError::InvalidRequest(m) => {
                    assert!(!m.contains('\n') && !m.contains('\r'), "newlines collapsed");
                    assert!(m.contains("[truncated]"), "oversized message bounded");
                }
                other => panic!("expected InvalidRequest, got {other:?}"),
            }

            // Non-JSON body -> falls back to the sanitized from_status_code error.
            match AnthropicProvider::clean_http_error(400, None, "not json blob") {
                ProviderError::InvalidRequest(m) => assert!(m.contains("not json blob")),
                other => panic!("expected InvalidRequest, got {other:?}"),
            }
        }

        // Minimal HTTP/1.1 reader: parse headers + Content-Length body.
        async fn read_http_request(sock: &mut tokio::net::TcpStream) -> String {
            use tokio::io::AsyncReadExt;
            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            loop {
                let n = sock.read(&mut tmp).await.unwrap();
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..pos]).to_lowercase();
                    let cl = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    let body_start = pos + 4;
                    while buf.len() < body_start + cl {
                        let n = sock.read(&mut tmp).await.unwrap();
                        if n == 0 {
                            break;
                        }
                        buf.extend_from_slice(&tmp[..n]);
                    }
                    break;
                }
            }
            String::from_utf8_lossy(&buf).to_string()
        }

        fn http_400(json: &str) -> String {
            format!(
                "HTTP/1.1 400 Bad Request\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                json.len(),
                json
            )
        }

        fn http_200_sse() -> String {
            let sse = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":1}}}\n\n";
            format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n{sse}"
            )
        }

        // Loopback mock: serves `responses` (each a full HTTP/1.1 message) over
        // `responses.len()` sequential connections, capturing each request body.
        // `connection: close` on every response forces reqwest to open a fresh
        // TCP connection per request (no keep-alive reuse).
        async fn spawn_mock(
            responses: Vec<String>,
        ) -> (
            String,
            tokio::task::JoinHandle<()>,
            std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        ) {
            use std::sync::{Arc, Mutex};
            use tokio::io::AsyncWriteExt;
            use tokio::net::TcpListener;

            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let bodies = Arc::new(Mutex::new(Vec::<String>::new()));
            let bodies_srv = bodies.clone();
            let handle = tokio::spawn(async move {
                for resp in responses {
                    let (mut sock, _) = listener.accept().await.unwrap();
                    let req = read_http_request(&mut sock).await;
                    bodies_srv.lock().unwrap().push(req);
                    sock.write_all(resp.as_bytes()).await.unwrap();
                    let _ = sock.flush().await;
                    let _ = sock.shutdown().await;
                }
            });
            (format!("http://{addr}"), handle, bodies)
        }

        fn chat_request(model: &str, temperature: f32) -> crate::models::ChatRequest {
            use crate::models::{ChatMessage, ChatRequest};
            let mut request = ChatRequest {
                model: model.to_string(),
                messages: vec![ChatMessage::user("hi")],
                max_tokens: Some(64),
                ..Default::default()
            };
            request.temperature = Some(temperature);
            request
        }

        #[tokio::test]
        async fn stream_chat_self_heals_unsupported_sampling_400_and_retries_once() {
            // Wrapped in a timeout so a retry/connection regression FAILS FAST
            // instead of hanging the mock's accept loop indefinitely.
            tokio::time::timeout(std::time::Duration::from_secs(15), async {
                use crate::traits::AIProvider;
                use futures_util::StreamExt;

                let (base_url, server, bodies) = spawn_mock(vec![
                    http_400(&err_body(
                        "temperature may only be set to 1 when thinking is enabled or in adaptive mode.",
                    )),
                    http_200_sse(),
                ])
                .await;

                // Allowed model, no thinking -> first body carries temperature.
                let request = chat_request("claude-3-5-sonnet", 0.5);
                let mut stream = AnthropicProvider
                    .stream_chat("test-key", &base_url, request)
                    .await
                    .expect("self-heal should retry and yield an Ok stream");
                // Drain the tiny stream so the mock's second connection completes.
                while stream.next().await.is_some() {}
                server.await.unwrap();

                let captured = bodies.lock().unwrap();
                assert_eq!(captured.len(), 2, "initial request + exactly one retry");
                assert!(
                    captured[0].contains("\"temperature\""),
                    "first request carries temperature"
                );
                assert!(
                    !captured[1].contains("\"temperature\""),
                    "retry strips temperature"
                );
            })
            .await
            .expect("test timed out — likely a retry/connection regression");
        }

        #[tokio::test]
        async fn stream_chat_surfaces_invalid_value_400_without_retry() {
            tokio::time::timeout(std::time::Duration::from_secs(15), async {
                use crate::traits::AIProvider;

                // An invalid-VALUE 400 (param accepted, value bad) must NOT be
                // self-healed: no retry, and the real error surfaces cleanly.
                let (base_url, server, bodies) = spawn_mock(vec![http_400(&err_body(
                    "temperature: Input should be less than or equal to 1",
                ))])
                .await;

                let request = chat_request("claude-3-5-sonnet", 5.0);
                // The Ok variant (boxed stream) isn't Debug, so match rather than expect_err.
                let err = match AnthropicProvider
                    .stream_chat("test-key", &base_url, request)
                    .await
                {
                    Ok(_) => panic!("invalid-value 400 must not be self-healed (got Ok stream)"),
                    Err(e) => e,
                };
                server.await.unwrap();

                match err {
                    ProviderError::InvalidRequest(m) => {
                        assert!(
                            m.contains("less than or equal to 1"),
                            "surfaces the real value error, got: {m}"
                        );
                        assert!(!m.contains('{'), "clean message, not the raw JSON blob");
                    }
                    other => panic!("expected InvalidRequest, got {other:?}"),
                }

                let captured = bodies.lock().unwrap();
                assert_eq!(captured.len(), 1, "no retry on an invalid-value 400");
            })
            .await
            .expect("test timed out");
        }

        #[tokio::test]
        async fn stream_chat_retries_at_most_once_on_persistent_sampling_400() {
            // The `attempted_repair` guard: if the stripped retry ALSO 400s, we
            // must NOT loop — exactly two requests, then a clean surfaced error.
            tokio::time::timeout(std::time::Duration::from_secs(15), async {
                use crate::traits::AIProvider;

                let unsupported = err_body("temperature: Extra inputs are not permitted");
                let (base_url, server, bodies) =
                    spawn_mock(vec![http_400(&unsupported), http_400(&unsupported)]).await;

                let request = chat_request("claude-3-5-sonnet", 0.5);
                let err = match AnthropicProvider
                    .stream_chat("test-key", &base_url, request)
                    .await
                {
                    Ok(_) => panic!("a persistently-400ing retry must surface an error"),
                    Err(e) => e,
                };
                server.await.unwrap();

                assert!(matches!(err, ProviderError::InvalidRequest(_)));
                let captured = bodies.lock().unwrap();
                assert_eq!(captured.len(), 2, "retry at most once, then give up");
                assert!(captured[0].contains("\"temperature\""), "first request carries temperature");
                assert!(!captured[1].contains("\"temperature\""), "retry stripped temperature");
            })
            .await
            .expect("test timed out — the attempted_repair guard may have regressed");
        }
    }
}
