//! OpenAI provider implementation (custom implementation for full control)
//!
//! This provider is used for all OpenAI-API-compatible providers including:
//! - OpenAI (https://api.openai.com/v1)
//! - Groq (https://api.groq.com/openai/v1)
//! - DeepSeek (https://api.deepseek.com/v1)
//! - Mistral (https://api.mistral.ai/v1)
//! - HuggingFace (various endpoints)
//! - Local (http://localhost:8000/v1)
//! - Custom (any OpenAI-compatible endpoint)

use crate::{
    error::ProviderError,
    models::{
        ChatRequest, EmbeddingsRequest, EmbeddingsResponse, FileUpload, FileUploadResponse,
        StreamChatChunk,
    },
    traits::AIProvider,
};
use async_stream::stream;
use async_trait::async_trait;
use futures_core::Stream;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::pin::Pin;

/// OpenAI provider (zero-sized, stateless)
pub struct OpenAIProvider;

/// Models that require non-streaming due to organization verification requirements
/// These models require org verification for streaming, so we use non-streaming internally
const MODELS_REQUIRING_NON_STREAMING: &[&str] = &["gpt-5", "gpt-5-mini"];

/// OpenAI API message format
#[derive(Serialize, Deserialize, Debug, Clone)]
struct OpenAIMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<OpenAIContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAIToolCall>>,
}

/// OpenAI content format (can be string or array for multimodal)
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
enum OpenAIContent {
    Text(String),
    Multimodal(Vec<OpenAIContentPart>),
}

/// OpenAI content part for multimodal messages
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OpenAIContentPart {
    Text {
        text: String,
    },
    ImageUrl {
        image_url: OpenAIImageUrl,
    },
    /// Document/PDF input — `{"type":"file","file":{file_id | filename+file_data}}`.
    File {
        file: OpenAIFileRef,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct OpenAIImageUrl {
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>, // "auto", "low", "high"
}

/// OpenAI `file` content reference (uploaded `file_id`, or inline base64 via
/// `filename` + `file_data` data URL).
#[derive(Serialize, Deserialize, Debug, Clone)]
struct OpenAIFileRef {
    #[serde(skip_serializing_if = "Option::is_none")]
    file_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_data: Option<String>,
}

/// OpenAI tool call format
#[derive(Serialize, Deserialize, Debug, Clone)]
struct OpenAIToolCall {
    id: String,
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIFunctionCall,
}

/// OpenAI function call format
#[derive(Serialize, Deserialize, Debug, Clone)]
struct OpenAIFunctionCall {
    name: String,
    arguments: String,
}

/// OpenAI tool definition format
#[derive(Serialize, Debug, Clone)]
struct OpenAITool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIFunctionDef,
}

/// OpenAI function definition format
#[derive(Serialize, Debug, Clone)]
struct OpenAIFunctionDef {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<serde_json::Value>,
}

/// OpenAI tool choice format
#[derive(Serialize, Debug, Clone)]
#[serde(untagged)]
enum OpenAIToolChoice {
    String(String), // "auto", "required", "none"
    Specific {
        #[serde(rename = "type")]
        tool_type: String,
        function: OpenAIToolChoiceFunction,
    },
}

#[derive(Serialize, Debug, Clone)]
struct OpenAIToolChoiceFunction {
    name: String,
}

#[derive(Deserialize, Debug)]
struct OpenAIUsage {
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,  // Optional, embeddings API doesn't include this
    total_tokens: u32,
    #[serde(default)]
    completion_tokens_details: Option<OpenAICompletionTokensDetails>,
    #[serde(default)]
    prompt_tokens_details: Option<OpenAIPromptTokensDetails>,
}

/// OpenAI prompt-token breakdown (automatic prompt caching reports cached_tokens).
#[derive(Deserialize, Debug)]
struct OpenAIPromptTokensDetails {
    #[serde(default)]
    cached_tokens: Option<u32>,
    #[serde(default)]
    #[allow(dead_code)]
    audio_tokens: Option<u32>,
}

/// OpenAI completion tokens breakdown (for reasoning models)
#[derive(Deserialize, Debug)]
struct OpenAICompletionTokensDetails {
    #[serde(default)]
    reasoning_tokens: Option<u32>,
    #[serde(default)]
    #[allow(dead_code)]
    audio_tokens: Option<u32>,
    #[serde(default)]
    #[allow(dead_code)]
    accepted_prediction_tokens: Option<u32>,
    #[serde(default)]
    #[allow(dead_code)]
    rejected_prediction_tokens: Option<u32>,
}

/// OpenAI streaming chunk
#[derive(Deserialize, Debug)]
struct OpenAIStreamChunk {
    #[allow(dead_code)]
    id: String,
    choices: Vec<OpenAIStreamChoice>,
    #[serde(default)]
    usage: Option<OpenAIUsage>,
}

#[derive(Deserialize, Debug)]
struct OpenAIStreamChoice {
    #[allow(dead_code)]
    index: u32,
    delta: OpenAIStreamDelta,
    finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
struct OpenAIStreamDelta {
    #[serde(default)]
    #[allow(dead_code)]
    role: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    refusal: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAIStreamToolCall>>,
    /// Reasoning text from reasoning models (DeepSeek-R1 `reasoning_content`;
    /// some OpenAI-compatible servers use `reasoning`).
    #[serde(default, alias = "reasoning")]
    reasoning_content: Option<String>,
}

/// Tool call delta in streaming responses
#[derive(Deserialize, Debug)]
struct OpenAIStreamToolCall {
    index: u32,
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "type", default)]
    #[allow(dead_code)]
    tool_type: Option<String>,
    #[serde(default)]
    function: Option<OpenAIStreamFunction>,
}

/// Function delta in streaming tool calls
#[derive(Deserialize, Debug)]
struct OpenAIStreamFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

/// OpenAI non-streaming response
#[derive(Deserialize, Debug)]
struct OpenAINonStreamResponse {
    #[allow(dead_code)]
    id: String,
    choices: Vec<OpenAINonStreamChoice>,
    usage: OpenAIUsage,
}

/// OpenAI non-streaming choice
#[derive(Deserialize, Debug)]
struct OpenAINonStreamChoice {
    #[allow(dead_code)]
    index: u32,
    message: OpenAINonStreamMessage,
    finish_reason: Option<String>,
}

/// OpenAI non-streaming message
#[derive(Deserialize, Debug)]
struct OpenAINonStreamMessage {
    #[serde(default)]
    #[allow(dead_code)]
    role: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    refusal: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(default, alias = "reasoning")]
    reasoning_content: Option<String>,
}

/// OpenAI embeddings request
#[derive(Serialize, Debug)]
struct OpenAIEmbeddingsRequest {
    model: String,
    input: Vec<String>,
}

/// OpenAI embeddings response
#[derive(Deserialize, Debug)]
struct OpenAIEmbeddingsResponse {
    data: Vec<OpenAIEmbedding>,
    usage: OpenAIUsage,
}

#[derive(Deserialize, Debug)]
struct OpenAIEmbedding {
    embedding: Vec<f32>,
}

impl OpenAIProvider {
    /// Makes a non-streaming request and converts to a stream (workaround for org verification)
    async fn non_streaming_to_stream(
        client: &Client,
        api_key: &str,
        base_url: &str,
        body: serde_json::Value,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<StreamChatChunk, ProviderError>> + Send>>,
        ProviderError,
    > {
        // Make non-streaming request
        let response = client
            .post(format!("{}/chat/completions", base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        // Check status
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ProviderError::from_status_code(status.as_u16(), error_text));
        }

        // Parse response
        let resp: OpenAINonStreamResponse = response.json().await?;

        // Convert to stream that yields chunks
        let output_stream = stream! {
            if let Some(choice) = resp.choices.first() {
                let message = &choice.message;

                // Build content deltas
                let mut content_deltas = Vec::new();

                // Reasoning content (DeepSeek-R1 style) -> thinking
                if let Some(ref reasoning) = message.reasoning_content {
                    if !reasoning.is_empty() {
                        content_deltas.push(crate::models::ContentBlockDelta::ThinkingDelta {
                            index: 0,
                            delta: reasoning.clone(),
                        });
                    }
                }

                // Offset text/tools past the thinking block at index 0 when reasoning
                // is present, so the answer isn't merged into the thinking block.
                let has_reasoning = message
                    .reasoning_content
                    .as_ref()
                    .map(|r| !r.is_empty())
                    .unwrap_or(false);
                let text_index = if has_reasoning { 1 } else { 0 };
                let tool_offset = if has_reasoning { 2 } else { 0 };

                // Text content
                if let Some(ref text) = message.content {
                    if !text.is_empty() {
                        content_deltas.push(crate::models::ContentBlockDelta::TextDelta {
                            index: text_index,
                            delta: text.clone(),
                        });
                    }
                }

                // Tool calls
                if let Some(ref tool_calls) = message.tool_calls {
                    for (idx, tc) in tool_calls.iter().enumerate() {
                        content_deltas.push(crate::models::ContentBlockDelta::ToolUseDelta {
                            index: idx + tool_offset,
                            id: Some(tc.id.clone()),
                            name: Some(tc.function.name.clone()),
                            input_delta: Some(tc.function.arguments.clone()),
                        });
                    }
                }

                // Yield content chunk (if there's content or refusal)
                if !content_deltas.is_empty() || message.refusal.is_some() {
                    yield Ok(StreamChatChunk {
                        content: content_deltas,
                        finish_reason: choice.finish_reason.clone(),
                        usage: None,
                        refusal: message.refusal.clone(),
                        safety_ratings: Vec::new(),
                        safety_blocked: false,
                    });
                }

                // Yield usage chunk
                yield Ok(StreamChatChunk {
                    content: Vec::new(),
                    finish_reason: None,
                    usage: Some(crate::models::StreamUsage {
                        prompt_tokens: resp.usage.prompt_tokens,
                        completion_tokens: resp.usage.completion_tokens,
                        total_tokens: resp.usage.total_tokens,
                        reasoning_tokens: resp.usage.completion_tokens_details
                            .and_then(|d| d.reasoning_tokens),
                        cache_read_input_tokens: resp.usage.prompt_tokens_details
                            .and_then(|d| d.cached_tokens),
                        cache_creation_input_tokens: None,
                    }),
                    refusal: None,
                    safety_ratings: Vec::new(),
                    safety_blocked: false,
                });
            }
        };

        Ok(Box::pin(output_stream))
    }

    /// Map a unified image source to an OpenAI `image_url` part. Returns `None`
    /// for a provider `file_id` (Chat Completions can't reference an image by id).
    fn image_part(source: &crate::models::ImageSource) -> Option<OpenAIContentPart> {
        use crate::models::ImageSource;
        match source {
            ImageSource::Base64 { media_type, data } => Some(OpenAIContentPart::ImageUrl {
                image_url: OpenAIImageUrl {
                    url: format!("data:{};base64,{}", media_type, data),
                    detail: None,
                },
            }),
            ImageSource::Url { url, detail } => Some(OpenAIContentPart::ImageUrl {
                image_url: OpenAIImageUrl {
                    url: url.clone(),
                    detail: detail.clone(),
                },
            }),
            ImageSource::File { file_id, .. } => {
                tracing::warn!("OpenAI: image file_id unsupported on Chat Completions: {}", file_id);
                None
            }
        }
    }

    /// Synthesize a filename for an inline base64 document (OpenAI requires one).
    fn synth_filename(media_type: &str) -> String {
        let ext = match media_type {
            "application/pdf" => "pdf",
            "text/plain" => "txt",
            "text/markdown" => "md",
            "application/json" => "json",
            _ => "bin",
        };
        format!("document.{ext}")
    }

    /// Collapse parts into the string-or-array content form.
    fn build_content(openai_parts: Vec<OpenAIContentPart>) -> Option<OpenAIContent> {
        if openai_parts.is_empty() {
            None
        } else if openai_parts.len() == 1
            && matches!(openai_parts[0], OpenAIContentPart::Text { .. })
        {
            match openai_parts.into_iter().next() {
                Some(OpenAIContentPart::Text { text }) => Some(OpenAIContent::Text(text)),
                _ => None,
            }
        } else {
            Some(OpenAIContent::Multimodal(openai_parts))
        }
    }

    /// Converts our messages to OpenAI format. Each tool result becomes its OWN
    /// `role:tool` message (so parallel tool calls keep distinct `tool_call_id`s),
    /// and a tool result carrying images additionally emits a following `role:user`
    /// message holding them (Chat Completions tool messages are text-only).
    fn convert_messages(msgs: &[crate::models::ChatMessage]) -> Vec<OpenAIMessage> {
        use crate::models::{ContentBlock, DocumentSource};

        // One pending tool result → one role:tool message (+ optional image spill).
        struct ToolResultMsg {
            id: String,
            name: Option<String>,
            text: String,
            images: Vec<OpenAIContentPart>,
        }

        let mut out: Vec<OpenAIMessage> = Vec::new();

        for m in msgs.iter() {
            let role = match m.role {
                crate::models::Role::System => "system",
                crate::models::Role::User => "user",
                crate::models::Role::Assistant => "assistant",
                crate::models::Role::Tool => "tool",
            }
            .to_string();

            let mut openai_parts = Vec::new();
            let mut tool_calls = Vec::new();
            let mut tool_results: Vec<ToolResultMsg> = Vec::new();

            for block in &m.content {
                match block {
                    ContentBlock::Text { text } => {
                        openai_parts.push(OpenAIContentPart::Text { text: text.clone() });
                    }
                    ContentBlock::Image { source } => {
                        if let Some(part) = Self::image_part(source) {
                            openai_parts.push(part);
                        }
                    }
                    // OpenAI doesn't accept thinking back in requests — skip.
                    ContentBlock::Thinking { .. } | ContentBlock::RedactedThinking { .. } => {}
                    ContentBlock::ToolUse { id, name, input } => {
                        tool_calls.push(OpenAIToolCall {
                            id: id.clone(),
                            tool_type: "function".to_string(),
                            function: OpenAIFunctionCall {
                                name: name.clone(),
                                arguments: input.to_string(),
                            },
                        });
                    }
                    ContentBlock::ToolResult {
                        tool_use_id,
                        name,
                        content,
                        ..
                    } => {
                        let mut texts: Vec<String> = Vec::new();
                        let mut images: Vec<OpenAIContentPart> = Vec::new();
                        for sub_block in content {
                            match sub_block {
                                ContentBlock::Text { text } => texts.push(text.clone()),
                                ContentBlock::Image { source } => {
                                    if let Some(part) = Self::image_part(source) {
                                        images.push(part);
                                    }
                                }
                                _ => {}
                            }
                        }
                        tool_results.push(ToolResultMsg {
                            id: tool_use_id.clone(),
                            name: name.clone(),
                            text: texts.join("\n"),
                            images,
                        });
                    }
                    ContentBlock::Document { source } => match source {
                        DocumentSource::Base64 { media_type, data } => {
                            openai_parts.push(OpenAIContentPart::File {
                                file: OpenAIFileRef {
                                    file_id: None,
                                    filename: Some(Self::synth_filename(media_type)),
                                    file_data: Some(format!("data:{};base64,{}", media_type, data)),
                                },
                            });
                        }
                        DocumentSource::File { file_id, .. } => {
                            openai_parts.push(OpenAIContentPart::File {
                                file: OpenAIFileRef {
                                    file_id: Some(file_id.clone()),
                                    filename: None,
                                    file_data: None,
                                },
                            });
                        }
                        DocumentSource::Url { url } => {
                            tracing::warn!("OpenAI: document URL unsupported on Chat Completions: {}", url);
                        }
                    },
                }
            }

            // Emit the non-tool-result message (text/image/document + tool_calls).
            let main_content = Self::build_content(openai_parts);
            if main_content.is_some() || !tool_calls.is_empty() {
                out.push(OpenAIMessage {
                    role,
                    content: main_content,
                    name: None,
                    tool_call_id: None,
                    tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
                });
            }

            // Emit one role:tool message per tool result; spill its images after it.
            for tr in tool_results {
                let content_text = if !tr.text.is_empty() {
                    tr.text
                } else if tr.images.is_empty() {
                    "[no output]".to_string()
                } else {
                    "[tool returned image(s); see following message]".to_string()
                };
                out.push(OpenAIMessage {
                    role: "tool".to_string(),
                    content: Some(OpenAIContent::Text(content_text)),
                    name: None,
                    tool_call_id: Some(tr.id),
                    tool_calls: None,
                });
                if !tr.images.is_empty() {
                    let label = format!(
                        "[tool {} returned image(s)]",
                        tr.name.unwrap_or_else(|| "result".to_string())
                    );
                    let mut parts = vec![OpenAIContentPart::Text { text: label }];
                    parts.extend(tr.images);
                    out.push(OpenAIMessage {
                        role: "user".to_string(),
                        content: Some(OpenAIContent::Multimodal(parts)),
                        name: None,
                        tool_call_id: None,
                        tool_calls: None,
                    });
                }
            }
        }

        out
    }

    /// Converts our tools to OpenAI format
    fn convert_tools(tools: &[crate::models::Tool]) -> Vec<OpenAITool> {
        tools
            .iter()
            .map(|t| OpenAITool {
                tool_type: t.tool_type.clone(),
                function: OpenAIFunctionDef {
                    name: t.function.name.clone(),
                    description: t.function.description.clone(),
                    parameters: Some(t.function.parameters.clone()),
                },
            })
            .collect()
    }

    /// Converts our tool choice to OpenAI format
    fn convert_tool_choice(choice: &crate::models::ToolChoice) -> OpenAIToolChoice {
        match choice {
            crate::models::ToolChoice::Auto => OpenAIToolChoice::String("auto".to_string()),
            crate::models::ToolChoice::Required => {
                OpenAIToolChoice::String("required".to_string())
            }
            crate::models::ToolChoice::Specific { function, .. } => OpenAIToolChoice::Specific {
                tool_type: "function".to_string(),
                function: OpenAIToolChoiceFunction {
                    name: function.name.clone(),
                },
            },
        }
    }

    /// Pure request-body assembly (no HTTP), so the wire shape is unit-testable.
    /// `requires_non_streaming` is the gpt-5 org-verification workaround flag.
    fn build_request_body(request: &ChatRequest, requires_non_streaming: bool) -> serde_json::Value {
        use crate::models::{ThinkingEffort, ThinkingMode};

        let messages = Self::convert_messages(&request.messages);

        let mut body = json!({
            "model": request.model,
            "messages": messages,
            "stream": !requires_non_streaming,
        });

        // When streaming, ask the provider to emit a final usage chunk
        // (`stream_options.include_usage`). Without this, OpenAI-compatible
        // backends omit `usage` from streamed responses, so token accounting
        // (chat cost, workflow run `total_tokens`) silently reads 0. The
        // streaming parser already consumes the usage chunk when present.
        if !requires_non_streaming {
            body["stream_options"] = json!({ "include_usage": true });
        }

        // Thinking → reasoning_effort. A reasoning model rejects temperature/top_p
        // and the penalties, so gate those below.
        let is_reasoning_model = match &request.thinking {
            Some(thinking) if thinking.mode != ThinkingMode::Disabled => {
                if let Some(effort) = thinking.effort {
                    let effort_str = match effort {
                        ThinkingEffort::Minimal => "minimal",
                        ThinkingEffort::Low => "low",
                        ThinkingEffort::Medium => "medium",
                        // OpenAI reasoning_effort tops out at "high".
                        ThinkingEffort::High | ThinkingEffort::XHigh | ThinkingEffort::Max => "high",
                        ThinkingEffort::Dynamic => "medium",
                    };
                    body["reasoning_effort"] = json!(effort_str);
                }
                if let Some(max_tokens) = request.max_tokens {
                    body["max_completion_tokens"] = json!(max_tokens);
                }
                true
            }
            _ => false,
        };

        // temperature / top_p / penalties — non-reasoning models only, and not on
        // gpt-5 (which only accepts default sampling).
        if !is_reasoning_model {
            if !requires_non_streaming {
                if let Some(temp) = request.temperature {
                    body["temperature"] = json!(temp);
                }
                if let Some(top_p) = request.top_p {
                    body["top_p"] = json!(top_p);
                }
                if let Some(fp) = request.frequency_penalty {
                    body["frequency_penalty"] = json!(fp);
                }
                if let Some(pp) = request.presence_penalty {
                    body["presence_penalty"] = json!(pp);
                }
            }
            if let Some(max_tokens) = request.max_tokens {
                if requires_non_streaming {
                    body["max_completion_tokens"] = json!(max_tokens);
                } else {
                    body["max_tokens"] = json!(max_tokens);
                }
            }
        }

        // seed / stop: the gpt-5 org-verification family rejects these (only
        // default sampling + max_completion_tokens), so gate them like temp/top_p.
        // (OpenAI Chat Completions has no top_k — it is intentionally omitted.)
        if !requires_non_streaming {
            if let Some(seed) = request.seed {
                body["seed"] = json!(seed);
            }
            if let Some(stop) = &request.stop {
                if !stop.is_empty() {
                    body["stop"] = json!(stop);
                }
            }
        }
        // user / prompt_cache_key are metadata-only and accepted on all models.
        if let Some(user) = &request.user {
            body["user"] = json!(user);
        }
        if let Some(key) = &request.prompt_cache_key {
            body["prompt_cache_key"] = json!(key);
        }

        if !request.tools.is_empty() {
            body["tools"] = json!(Self::convert_tools(&request.tools));
        }
        if let Some(tool_choice) = &request.tool_choice {
            body["tool_choice"] = json!(Self::convert_tool_choice(tool_choice));
        }

        body
    }

}

#[async_trait]
impl AIProvider for OpenAIProvider {
    fn name(&self) -> &str {
        "OpenAI"
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

        // gpt-5 / gpt-5-mini require the non-streaming org-verification workaround.
        let requires_non_streaming =
            MODELS_REQUIRING_NON_STREAMING.contains(&request.model.as_str());

        // Build the request body (pure, unit-testable).
        let body = Self::build_request_body(&request, requires_non_streaming);

        // If model requires non-streaming, use the workaround.
        if requires_non_streaming {
            return Self::non_streaming_to_stream(&client, api_key, base_url, body).await;
        }

        // Make streaming request
        let response = client
            .post(format!("{}/chat/completions", base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
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

            // OpenAI emits reasoning_content and the answer in the same delta object
            // with no per-stream index. Once reasoning appears, shift text to index 1
            // and tools to +2 so the answer isn't merged into the thinking block.
            // (Reasoning models emit reasoning before content, so this latches early.)
            let mut reasoning_seen = false;
            // Freeze (text_index, tool_offset) the first time real content is placed,
            // so a tool call's deltas keep ONE index across chunks even in the
            // pathological case where tool deltas arrive before reasoning (otherwise
            // the offset would flip mid-call and split one tool call into two).
            let mut frozen_offsets: Option<(usize, usize)> = None;

            while let Some(chunk_result) = byte_stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        // Decode incrementally so a multi-byte UTF-8 char split
                        // across chunk boundaries doesn't abort the stream.
                        buffer.push_str(&decoder.decode(&chunk));

                        // Process complete SSE events
                        while let Some(index) = buffer.find("\n\n") {
                            let event = buffer[..index].to_string();
                            buffer.drain(..=index + 1);

                            // Parse event
                            if event.starts_with("data: ") {
                                let data = &event[6..]; // Skip "data: "

                                if data == "[DONE]" {
                                    break;
                                }

                                // Try to parse as JSON
                                if let Ok(chunk_data) = serde_json::from_str::<OpenAIStreamChunk>(data) {
                                    // Check for usage metadata (final chunk)
                                    if let Some(usage) = chunk_data.usage {
                                        yield Ok(StreamChatChunk {
                                            content: Vec::new(),
                                            finish_reason: None,
                                            usage: Some(crate::models::StreamUsage {
                                                prompt_tokens: usage.prompt_tokens,
                                                completion_tokens: usage.completion_tokens,
                                                total_tokens: usage.total_tokens,
                                                reasoning_tokens: usage.completion_tokens_details
                                                    .and_then(|d| d.reasoning_tokens),
                                                cache_read_input_tokens: usage.prompt_tokens_details
                                                    .and_then(|d| d.cached_tokens),
                                                cache_creation_input_tokens: None,
                                            }),
                                            refusal: None,
                                            safety_ratings: Vec::new(),
                                            safety_blocked: false,
                                        });
                                    }

                                    if let Some(choice) = chunk_data.choices.first() {
                                        let delta = &choice.delta;

                                        // Build content block deltas
                                        let mut content_deltas = Vec::new();

                                        // Reasoning content delta (DeepSeek-R1 style) -> thinking
                                        if let Some(ref reasoning) = delta.reasoning_content {
                                            if !reasoning.is_empty() {
                                                reasoning_seen = true;
                                                content_deltas.push(crate::models::ContentBlockDelta::ThinkingDelta {
                                                    index: 0,
                                                    delta: reasoning.clone(),
                                                });
                                            }
                                        }

                                        // Index offset so text/tools never collide with the
                                        // thinking block at index 0. Freeze on first real
                                        // content so the offset can't change mid-stream.
                                        let has_content =
                                            delta.content.as_ref().map(|t| !t.is_empty()).unwrap_or(false);
                                        let has_tools =
                                            delta.tool_calls.as_ref().map(|t| !t.is_empty()).unwrap_or(false);
                                        let (text_index, tool_offset) = if has_content || has_tools {
                                            *frozen_offsets
                                                .get_or_insert(if reasoning_seen { (1, 2) } else { (0, 0) })
                                        } else {
                                            (0, 0) // unused this chunk
                                        };

                                        // Text content delta
                                        if let Some(ref text) = delta.content {
                                            if !text.is_empty() {
                                                content_deltas.push(crate::models::ContentBlockDelta::TextDelta {
                                                    index: text_index,
                                                    delta: text.clone(),
                                                });
                                            }
                                        }

                                        // Tool call deltas
                                        if let Some(ref tool_calls) = delta.tool_calls {
                                            for tc in tool_calls {
                                                content_deltas.push(crate::models::ContentBlockDelta::ToolUseDelta {
                                                    index: tc.index as usize + tool_offset,
                                                    id: tc.id.clone(),
                                                    name: tc.function.as_ref().and_then(|f| f.name.clone()),
                                                    input_delta: tc.function.as_ref().and_then(|f| f.arguments.clone()),
                                                });
                                            }
                                        }

                                        // Yield if there's any content, refusal, or finish_reason
                                        // (finish_reason can arrive on an empty delta, e.g. "tool_calls")
                                        if !content_deltas.is_empty() || delta.refusal.is_some() || choice.finish_reason.is_some() {
                                            yield Ok(StreamChatChunk {
                                                content: content_deltas,
                                                finish_reason: choice.finish_reason.clone(),
                                                usage: None,
                                                refusal: delta.refusal.clone(),
                                                safety_ratings: Vec::new(),
                                                safety_blocked: false,
                                            });
                                        }
                                    }
                                }
                            }
                        }

                        // Guard against an upstream that never emits a delimiter
                        // (would otherwise grow `buffer` until OOM).
                        if buffer.len() > super::MAX_SSE_BUFFER_BYTES {
                            yield Err(ProviderError::streaming(
                                "OpenAI: SSE buffer exceeded maximum size",
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
        api_key: &str,
        base_url: &str,
        request: EmbeddingsRequest,
    ) -> Result<EmbeddingsResponse, ProviderError> {
        let client = super::http_client();

        // Build request
        let body = OpenAIEmbeddingsRequest {
            model: request.model,
            input: request.input,
        };

        // Make request
        let response = client
            .post(format!("{}/embeddings", base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        // Check status
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ProviderError::from_status_code(status.as_u16(), error_text));
        }

        // Parse response
        let openai_resp: OpenAIEmbeddingsResponse = response.json().await?;

        // Convert to our format
        Ok(EmbeddingsResponse {
            embeddings: openai_resp
                .data
                .into_iter()
                .map(|e| e.embedding)
                .collect(),
            usage: Some(crate::models::Usage {
                prompt_tokens: openai_resp.usage.prompt_tokens,
                completion_tokens: 0, // Embeddings don't have completion tokens
                total_tokens: openai_resp.usage.total_tokens,
                reasoning_tokens: None, // Embeddings don't use reasoning
                cache_read_input_tokens: None,
                cache_creation_input_tokens: None,
            }),
        })
    }

    fn supports_file_api(&self) -> bool {
        // Documents/PDFs only — the server router (provider_routing.rs) keeps
        // images base64 for OpenAI (image file_id is Responses-API-only).
        // NOTE: OpenAIProvider is shared by groq/deepseek/etc.; those are kept
        // off the upload path by the router's provider_type gate.
        true
    }

    fn file_expiration(&self) -> Option<chrono::Duration> {
        None // OpenAI files persist until deleted.
    }

    async fn upload_file(
        &self,
        api_key: &str,
        base_url: &str,
        upload: FileUpload,
    ) -> Result<Option<FileUploadResponse>, ProviderError> {
        let client = super::http_client();

        let file_part = reqwest::multipart::Part::bytes(upload.file_data)
            .file_name(upload.filename.clone())
            .mime_str(&upload.mime_type)
            .map_err(|e| ProviderError::InvalidRequest(format!("Invalid MIME type: {}", e)))?;

        let form = reqwest::multipart::Form::new()
            .text("purpose", "user_data")
            .part("file", file_part);

        let response = client
            .post(format!("{}/files", base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .multipart(form)
            .send()
            .await
            .map_err(ProviderError::Network)?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ProviderError::from_status_code(status.as_u16(), error_text));
        }

        #[derive(Deserialize)]
        struct OpenAIFileUploadResponse {
            id: String,
            #[serde(default)]
            bytes: Option<u64>,
            #[serde(default)]
            filename: Option<String>,
        }

        let upload_response: OpenAIFileUploadResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::file_upload(format!("Failed to parse upload response: {}", e)))?;

        Ok(Some(FileUploadResponse {
            provider_file_id: upload_response.id,
            expires_at: None, // OpenAI files don't expire.
            metadata: Some(serde_json::json!({
                "filename": upload_response.filename.unwrap_or(upload.filename),
                "purpose": "user_data",
                "bytes": upload_response.bytes,
                "mime_type": upload.mime_type,
            })),
        }))
    }

    async fn delete_file(
        &self,
        api_key: &str,
        base_url: &str,
        provider_file_id: &str,
    ) -> Result<(), ProviderError> {
        let client = super::http_client();

        let response = client
            .delete(format!("{}/files/{}", base_url, provider_file_id))
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
            .map_err(ProviderError::Network)?;

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
    use super::*;

    /// gpt-5 and gpt-5-mini require the org-verification "non-streaming" workaround
    /// AND they reject the older `max_tokens` / non-default `temperature` / `top_p`
    /// parameters. The list drives the per-request body shape.
    #[test]
    fn test_models_requiring_non_streaming_includes_gpt5_family() {
        assert!(MODELS_REQUIRING_NON_STREAMING.contains(&"gpt-5"));
        assert!(MODELS_REQUIRING_NON_STREAMING.contains(&"gpt-5-mini"));
        // gpt-4 family should NOT be in this list (they support streaming +
        // max_tokens + temperature normally).
        assert!(!MODELS_REQUIRING_NON_STREAMING.contains(&"gpt-4o"));
        assert!(!MODELS_REQUIRING_NON_STREAMING.contains(&"gpt-4-turbo"));
    }

    /// OpenAI streams often emit a final chunk whose `delta` is empty and whose
    /// only meaningful field is `finish_reason: "tool_calls"` (or "stop"). The
    /// chat_stream loop yields on any of: content deltas, refusal, OR
    /// finish_reason present — so this chunk shape must deserialize cleanly so
    /// the guard sees it.
    #[test]
    fn test_stream_chunk_with_empty_delta_and_finish_reason_deserializes() {
        // Real-world OpenAI streaming chunk that carries only finish_reason
        let json = r#"{
            "id": "chatcmpl-1",
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "tool_calls"
            }]
        }"#;
        let chunk: OpenAIStreamChunk = serde_json::from_str(json)
            .expect("empty-delta + finish_reason chunk must deserialize");
        let choice = &chunk.choices[0];
        assert_eq!(choice.finish_reason.as_deref(), Some("tool_calls"));
        assert!(choice.delta.content.is_none());
        assert!(choice.delta.refusal.is_none());
        assert!(choice.delta.tool_calls.is_none());

        // The yield guard in chat_stream evaluates exactly this disjunction:
        let content_deltas: Vec<()> = vec![]; // (empty — no content deltas in this chunk)
        let should_yield = !content_deltas.is_empty()
            || choice.delta.refusal.is_some()
            || choice.finish_reason.is_some();
        assert!(
            should_yield,
            "guard must yield when only finish_reason is present"
        );
    }

    /// And the regression direction: a chunk with neither content nor refusal
    /// nor finish_reason must NOT yield (would emit spurious empty chunks).
    #[test]
    fn test_stream_chunk_with_nothing_does_not_yield() {
        let json = r#"{
            "id": "chatcmpl-1",
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": null
            }]
        }"#;
        let chunk: OpenAIStreamChunk = serde_json::from_str(json).unwrap();
        let choice = &chunk.choices[0];
        let content_deltas: Vec<()> = vec![];
        let should_yield = !content_deltas.is_empty()
            || choice.delta.refusal.is_some()
            || choice.finish_reason.is_some();
        assert!(
            !should_yield,
            "guard must NOT yield when nothing is present"
        );
    }

    /// Tier-1 wire-shape tests for the pure `build_request_body` + conversions.
    mod build_body {
        use super::super::OpenAIProvider;
        use crate::models::{
            ChatMessage, ChatRequest, ContentBlock, DocumentSource, ImageSource, Role,
            ThinkingConfig, ThinkingEffort,
        };

        fn req() -> ChatRequest {
            ChatRequest {
                model: "gpt-4o".to_string(),
                messages: vec![ChatMessage::user("hi")],
                max_tokens: Some(1024),
                ..Default::default()
            }
        }

        #[test]
        fn reasoning_effort_caps_at_high_and_no_top_k() {
            let mut r = req();
            r.thinking = Some(ThinkingConfig::adaptive_with_effort(ThinkingEffort::Max));
            r.top_k = Some(40);
            let body = OpenAIProvider::build_request_body(&r, false);
            assert_eq!(body["reasoning_effort"], "high"); // XHigh/Max -> high
            assert!(body.get("top_k").is_none(), "OpenAI has no top_k");
        }

        #[test]
        fn sampling_params_seed_stop_user_cache_key() {
            let mut r = req();
            r.frequency_penalty = Some(0.5);
            r.presence_penalty = Some(0.25);
            r.seed = Some(7);
            r.stop = Some(vec!["END".into()]);
            r.user = Some("u9".into());
            r.prompt_cache_key = Some("conv-1".into());
            let body = OpenAIProvider::build_request_body(&r, false);
            assert_eq!(body["frequency_penalty"], 0.5);
            assert_eq!(body["presence_penalty"], 0.25);
            assert_eq!(body["seed"], 7);
            assert_eq!(body["stop"][0], "END");
            assert_eq!(body["user"], "u9");
            assert_eq!(body["prompt_cache_key"], "conv-1");
        }

        #[test]
        fn document_base64_becomes_file_part() {
            let mut r = req();
            r.messages = vec![ChatMessage::with_blocks(
                Role::User,
                vec![ContentBlock::Document {
                    source: DocumentSource::Base64 {
                        media_type: "application/pdf".into(),
                        data: "QUJD".into(),
                    },
                }],
            )];
            let body = OpenAIProvider::build_request_body(&r, false);
            let part = &body["messages"][0]["content"][0];
            assert_eq!(part["type"], "file");
            assert_eq!(part["file"]["filename"], "document.pdf");
            assert_eq!(part["file"]["file_data"], "data:application/pdf;base64,QUJD");
        }

        #[test]
        fn document_file_id_part() {
            let mut r = req();
            r.messages = vec![ChatMessage::with_blocks(
                Role::User,
                vec![ContentBlock::Document {
                    source: DocumentSource::File { file_id: "file-abc".into(), media_type: None },
                }],
            )];
            let body = OpenAIProvider::build_request_body(&r, false);
            assert_eq!(body["messages"][0]["content"][0]["file"]["file_id"], "file-abc");
        }

        #[test]
        fn tool_result_image_spills_to_following_user_message() {
            let mut r = req();
            r.messages = vec![ChatMessage::with_blocks(
                Role::Tool,
                vec![ContentBlock::ToolResult {
                    tool_use_id: "call_1".into(),
                    name: Some("snap".into()),
                    content: vec![
                        ContentBlock::Text { text: "ok".into() },
                        ContentBlock::Image {
                            source: ImageSource::Base64 {
                                media_type: "image/png".into(),
                                data: "QQ".into(),
                            },
                        },
                    ],
                    is_error: None,
                }],
            )];
            let body = OpenAIProvider::build_request_body(&r, false);
            let msgs = body["messages"].as_array().unwrap();
            // The tool turn expands to: role:tool (text) + role:user (image_url).
            assert_eq!(msgs.len(), 2);
            assert_eq!(msgs[0]["role"], "tool");
            assert_eq!(msgs[0]["tool_call_id"], "call_1");
            assert_eq!(msgs[1]["role"], "user");
            // image rides in the user message as image_url
            let user_parts = msgs[1]["content"].as_array().unwrap();
            assert!(user_parts.iter().any(|p| p["type"] == "image_url"));
        }
    }
}
