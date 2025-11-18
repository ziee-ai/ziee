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
    models::{ChatRequest, EmbeddingsRequest, EmbeddingsResponse, StreamChatChunk},
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
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct OpenAIImageUrl {
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>, // "auto", "low", "high"
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

                // Text content
                if let Some(ref text) = message.content {
                    if !text.is_empty() {
                        content_deltas.push(crate::models::ContentBlockDelta::TextDelta {
                            index: 0,
                            delta: text.clone(),
                        });
                    }
                }

                // Tool calls
                if let Some(ref tool_calls) = message.tool_calls {
                    for (idx, tc) in tool_calls.iter().enumerate() {
                        content_deltas.push(crate::models::ContentBlockDelta::ToolUseDelta {
                            index: idx,
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
                    }),
                    refusal: None,
                    safety_ratings: Vec::new(),
                    safety_blocked: false,
                });
            }
        };

        Ok(Box::pin(output_stream))
    }

    /// Converts our messages to OpenAI format
    fn convert_messages(msgs: &[crate::models::ChatMessage]) -> Vec<OpenAIMessage> {
        use crate::models::{ContentBlock, ImageSource};

        msgs.iter()
            .map(|m| {
                let role = match m.role {
                    crate::models::Role::System => "system",
                    crate::models::Role::User => "user",
                    crate::models::Role::Assistant => "assistant",
                    crate::models::Role::Tool => "tool",
                }
                .to_string();

                // Convert content blocks to OpenAI format
                let mut openai_parts = Vec::new();
                let mut tool_calls = Vec::new();
                let mut tool_call_id = None;

                for block in &m.content {
                    match block {
                        ContentBlock::Text { text } => {
                            openai_parts.push(OpenAIContentPart::Text {
                                text: text.clone(),
                            });
                        }
                        ContentBlock::Image { source } => {
                            match source {
                                ImageSource::Base64 { media_type, data } => {
                                    let url = format!("data:{};base64,{}", media_type, data);
                                    openai_parts.push(OpenAIContentPart::ImageUrl {
                                        image_url: OpenAIImageUrl { url, detail: None },
                                    });
                                }
                                ImageSource::Url { url, detail } => {
                                    openai_parts.push(OpenAIContentPart::ImageUrl {
                                        image_url: OpenAIImageUrl { url: url.clone(), detail: detail.clone() },
                                    });
                                }
                                ImageSource::File { file_id } => {
                                    // OpenAI doesn't support file references for images
                                    eprintln!("Warning: OpenAI doesn't support file references for images, file_id: {}", file_id);
                                    // Skip this image - caller should use base64 instead
                                }
                            }
                        }
                        ContentBlock::Thinking { .. } => {
                            // OpenAI doesn't support thinking in requests - skip
                        }
                        ContentBlock::ToolUse { id, name, input } => {
                            // OpenAI uses separate tool_calls array
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
                            content,
                            ..
                        } => {
                            // OpenAI uses tool_call_id at message level
                            tool_call_id = Some(tool_use_id.clone());
                            // Flatten tool result content blocks to text
                            for sub_block in content {
                                if let ContentBlock::Text { text } = sub_block {
                                    openai_parts.push(OpenAIContentPart::Text {
                                        text: text.clone(),
                                    });
                                }
                            }
                        }
                        ContentBlock::Document { .. } => {
                            // OpenAI chat doesn't support document uploads via vision API
                            // Documents are only supported via Assistants API
                            eprintln!("Warning: OpenAI chat doesn't support document uploads (use Assistants API instead)");
                        }
                    }
                }

                // Build content (string or multimodal array)
                let content = if openai_parts.is_empty() {
                    None
                } else if openai_parts.len() == 1
                    && matches!(openai_parts[0], OpenAIContentPart::Text { .. })
                {
                    // Single text part - use string format
                    if let Some(OpenAIContentPart::Text { text }) = openai_parts.into_iter().next()
                    {
                        Some(OpenAIContent::Text(text))
                    } else {
                        unreachable!()
                    }
                } else {
                    // Multiple parts or non-text single part - use array format
                    Some(OpenAIContent::Multimodal(openai_parts))
                };

                OpenAIMessage {
                    role,
                    content,
                    name: None,
                    tool_call_id,
                    tool_calls: if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    },
                }
            })
            .collect()
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
        let client = Client::new();

        // Convert messages
        let messages = Self::convert_messages(&request.messages);

        // Check if model requires non-streaming workaround (org verification requirement)
        let requires_non_streaming = MODELS_REQUIRING_NON_STREAMING
            .contains(&request.model.as_str());

        // Build request body
        let mut body = json!({
            "model": request.model,
            "messages": messages,
            "stream": !requires_non_streaming, // Use non-streaming for gpt-5, gpt-5-mini
        });

        // Handle thinking/reasoning configuration
        let is_reasoning_model = if let Some(ref thinking) = request.thinking {
            if thinking.enabled {
                // Add reasoning_effort parameter
                if let Some(ref effort) = thinking.effort {
                    let effort_str = match effort {
                        crate::models::ThinkingEffort::Minimal => "minimal",
                        crate::models::ThinkingEffort::Low => "low",
                        crate::models::ThinkingEffort::Medium => "medium",
                        crate::models::ThinkingEffort::High => "high",
                        crate::models::ThinkingEffort::Dynamic => "medium",
                    };
                    body["reasoning_effort"] = json!(effort_str);
                }

                // Use max_completion_tokens for reasoning models
                if let Some(max_tokens) = request.max_tokens {
                    body["max_completion_tokens"] = json!(max_tokens);
                }
                true
            } else {
                false
            }
        } else {
            false
        };

        // Add optional parameters (only for non-reasoning models)
        if !is_reasoning_model {
            if let Some(temp) = request.temperature {
                body["temperature"] = json!(temp);
            }
            if let Some(max_tokens) = request.max_tokens {
                body["max_tokens"] = json!(max_tokens);
            }
            if let Some(top_p) = request.top_p {
                body["top_p"] = json!(top_p);
            }
        }

        // Add tools if provided
        if !request.tools.is_empty() {
            let tools = Self::convert_tools(&request.tools);
            body["tools"] = json!(tools);
        }

        // Add tool choice if provided
        if let Some(ref tool_choice) = request.tool_choice {
            body["tool_choice"] = json!(Self::convert_tool_choice(tool_choice));
        }

        // If model requires non-streaming, use the workaround
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
            let mut byte_stream = Box::pin(byte_stream);

            while let Some(chunk_result) = byte_stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        // Convert bytes to string
                        let chunk_str = match std::str::from_utf8(&chunk) {
                            Ok(s) => s,
                            Err(e) => {
                                yield Err(ProviderError::streaming(format!("Invalid UTF-8: {}", e)));
                                break;
                            }
                        };

                        buffer.push_str(chunk_str);

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

                                        // Text content delta
                                        if let Some(ref text) = delta.content {
                                            if !text.is_empty() {
                                                content_deltas.push(crate::models::ContentBlockDelta::TextDelta {
                                                    index: 0,
                                                    delta: text.clone(),
                                                });
                                            }
                                        }

                                        // Tool call deltas
                                        if let Some(ref tool_calls) = delta.tool_calls {
                                            for tc in tool_calls {
                                                content_deltas.push(crate::models::ContentBlockDelta::ToolUseDelta {
                                                    index: tc.index as usize,
                                                    id: tc.id.clone(),
                                                    name: tc.function.as_ref().and_then(|f| f.name.clone()),
                                                    input_delta: tc.function.as_ref().and_then(|f| f.arguments.clone()),
                                                });
                                            }
                                        }

                                        // Yield if there's any content or refusal
                                        if !content_deltas.is_empty() || delta.refusal.is_some() {
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
        let client = Client::new();

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
            }),
        })
    }
}
