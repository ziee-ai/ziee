//! Anthropic provider implementation (custom implementation based on anthropic-sdk reference)

use crate::{
    error::ProviderError,
    models::{
        ChatMessage, ChatRequest, ChatResponse, Choice, EmbeddingsRequest, EmbeddingsResponse,
        Message, Role, StreamChatChunk, Tool, ToolChoice, Usage,
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
    Image {
        source: AnthropicImageSource,
    },
}

#[derive(Serialize)]
struct AnthropicImageSource {
    #[serde(rename = "type")]
    source_type: String, // "base64"
    media_type: String,  // "image/jpeg", "image/png", etc.
    data: String,        // base64 encoded image data
}

/// Anthropic API response
#[derive(Deserialize)]
struct AnthropicResponse {
    id: String,
    model: String,
    content: Vec<AnthropicContent>,
    usage: AnthropicUsage,
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    thinking: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    signature: Option<String>, // Encrypted verification string
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

/// Anthropic streaming chunk
#[derive(Deserialize)]
struct AnthropicStreamChunk {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<AnthropicDelta>,
}

#[derive(Deserialize)]
struct AnthropicDelta {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    delta_type: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    thinking: Option<String>,
}

impl AnthropicProvider {
    /// Converts our messages to Anthropic format
    fn convert_messages(
        msgs: &[ChatMessage],
        attachments: &[crate::models::FileAttachment],
    ) -> (Option<String>, Vec<AnthropicMessage>) {
        use base64::Engine;
        let base64_engine = base64::engine::general_purpose::STANDARD;

        let mut system_message = None;
        let mut messages = Vec::new();

        // Find the index of the last user message to attach files to
        let last_user_idx = msgs
            .iter()
            .enumerate()
            .rev()
            .find(|(_, m)| m.role == Role::User)
            .map(|(i, _)| i);

        for (idx, msg) in msgs.iter().enumerate() {
            match msg.role {
                Role::System => {
                    system_message = msg.content.clone();
                }
                Role::User => {
                    // Build content (with attachments if this is the last user message)
                    let content = if Some(idx) == last_user_idx && !attachments.is_empty() {
                        // Multimodal content with text and images
                        let mut blocks = Vec::new();

                        // Add text block if there is content
                        if let Some(ref text) = msg.content {
                            if !text.is_empty() {
                                blocks.push(AnthropicContentBlock::Text { text: text.clone() });
                            }
                        }

                        // Add image blocks
                        for attachment in attachments {
                            // Check if it's an image
                            if attachment.mime_type.starts_with("image/") {
                                let base64_data = base64_engine.encode(&attachment.content);
                                blocks.push(AnthropicContentBlock::Image {
                                    source: AnthropicImageSource {
                                        source_type: "base64".to_string(),
                                        media_type: attachment.mime_type.clone(),
                                        data: base64_data,
                                    },
                                });
                            }
                        }

                        if blocks.len() > 1 {
                            AnthropicMessageContent::Multimodal(blocks)
                        } else if blocks.len() == 1 {
                            // Single text block, use string format
                            if let Some(AnthropicContentBlock::Text { text }) =
                                blocks.into_iter().next()
                            {
                                AnthropicMessageContent::Text(text)
                            } else {
                                AnthropicMessageContent::Text(String::new())
                            }
                        } else {
                            AnthropicMessageContent::Text(String::new())
                        }
                    } else {
                        // Regular text content
                        AnthropicMessageContent::Text(msg.content.clone().unwrap_or_default())
                    };

                    messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content,
                    });
                }
                Role::Assistant => {
                    messages.push(AnthropicMessage {
                        role: "assistant".to_string(),
                        content: AnthropicMessageContent::Text(
                            msg.content.clone().unwrap_or_default(),
                        ),
                    });
                }
                Role::Tool => {
                    // Anthropic handles tool results as user messages with special format
                    // For now, we'll treat them as user messages
                    messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: AnthropicMessageContent::Text(
                            msg.content.clone().unwrap_or_default(),
                        ),
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

    /// Converts Anthropic response to our format
    fn convert_response(resp: AnthropicResponse) -> ChatResponse {
        // Extract text content
        let content = resp
            .content
            .iter()
            .filter(|c| c.content_type == "text")
            .filter_map(|c| c.text.as_ref())
            .cloned()
            .collect::<Vec<_>>()
            .join("");

        // Extract thinking content
        let thinking = resp
            .content
            .iter()
            .filter(|c| c.content_type == "thinking")
            .filter_map(|c| c.thinking.as_ref())
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");

        ChatResponse {
            id: resp.id,
            model: resp.model,
            choices: vec![Choice {
                message: Message {
                    role: Role::Assistant,
                    content,
                    tool_calls: Vec::new(), // TODO: Extract tool calls from Anthropic response
                    thinking: if thinking.is_empty() {
                        None
                    } else {
                        Some(thinking)
                    },
                },
                finish_reason: resp.stop_reason.unwrap_or_else(|| "stop".to_string()),
                index: 0,
            }],
            usage: Some(Usage {
                prompt_tokens: resp.usage.input_tokens,
                completion_tokens: resp.usage.output_tokens,
                total_tokens: resp.usage.input_tokens + resp.usage.output_tokens,
                reasoning_tokens: None, // Anthropic bills for full thinking tokens, not separate
            }),
        }
    }
}

#[async_trait]
impl AIProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "Anthropic"
    }

    fn provider_type(&self) -> &str {
        "anthropic"
    }

    async fn chat(
        &self,
        api_key: &str,
        base_url: &str,
        request: ChatRequest,
    ) -> Result<ChatResponse, ProviderError> {
        let client = Client::new();

        // Convert messages
        let (system, messages) = Self::convert_messages(&request.messages, &request.attachments);

        // Build request body
        let mut body = json!({
            "model": request.model,
            "max_tokens": request.max_tokens.unwrap_or(1024),
            "messages": messages,
        });

        if let Some(system_msg) = system {
            body["system"] = json!(system_msg);
        }

        if let Some(temp) = request.temperature {
            body["temperature"] = json!(temp);
        }

        if let Some(top_p) = request.top_p {
            body["top_p"] = json!(top_p);
        }

        // Add tools if provided
        if !request.tools.is_empty() {
            let tools = Self::convert_tools(&request.tools);
            body["tools"] = json!(tools);
        }

        // Add tool choice if provided
        if let Some(ref tool_choice) = request.tool_choice {
            body["tool_choice"] = Self::convert_tool_choice(tool_choice);
        }

        // Add thinking configuration if provided
        if let Some(ref thinking) = request.thinking {
            if thinking.enabled {
                let budget = thinking.budget_tokens.unwrap_or(10000).max(1024); // Minimum 1024
                body["thinking"] = json!({
                    "type": "enabled",
                    "budget_tokens": budget
                });
            }
        }

        // Make request
        let response = client
            .post(format!("{}/messages", base_url))
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
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

        // Parse response
        let anthropic_resp: AnthropicResponse = response.json().await?;

        // Convert to our format
        Ok(Self::convert_response(anthropic_resp))
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
        let (system, messages) = Self::convert_messages(&request.messages, &request.attachments);

        // Build request body with stream: true
        let mut body = json!({
            "model": request.model,
            "max_tokens": request.max_tokens.unwrap_or(1024),
            "messages": messages,
            "stream": true,
        });

        if let Some(system_msg) = system {
            body["system"] = json!(system_msg);
        }

        if let Some(temp) = request.temperature {
            body["temperature"] = json!(temp);
        }

        if let Some(top_p) = request.top_p {
            body["top_p"] = json!(top_p);
        }

        // Add tools if provided
        if !request.tools.is_empty() {
            let tools = Self::convert_tools(&request.tools);
            body["tools"] = json!(tools);
        }

        // Add tool choice if provided
        if let Some(ref tool_choice) = request.tool_choice {
            body["tool_choice"] = Self::convert_tool_choice(tool_choice);
        }

        // Add thinking configuration if provided
        if let Some(ref thinking) = request.thinking {
            if thinking.enabled {
                let budget = thinking.budget_tokens.unwrap_or(10000).max(1024);
                body["thinking"] = json!({
                    "type": "enabled",
                    "budget_tokens": budget
                });
            }
        }

        // Make streaming request
        let response = client
            .post(format!("{}/messages", base_url))
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
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
                                if let Ok(chunk_data) = serde_json::from_str::<AnthropicStreamChunk>(data) {
                                    if chunk_data.event_type == "content_block_delta" {
                                        if let Some(delta) = chunk_data.delta {
                                            // Yield text delta if present
                                            if let Some(text) = delta.text {
                                                yield Ok(StreamChatChunk {
                                                    content: text,
                                                    finish_reason: None,
                                                    thinking: None,
                                                });
                                            }
                                            // Yield thinking delta if present
                                            if let Some(thinking) = delta.thinking {
                                                yield Ok(StreamChatChunk {
                                                    content: String::new(),
                                                    finish_reason: None,
                                                    thinking: Some(thinking),
                                                });
                                            }
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
        _api_key: &str,
        _base_url: &str,
        _request: EmbeddingsRequest,
    ) -> Result<EmbeddingsResponse, ProviderError> {
        // Anthropic doesn't support embeddings API
        Err(ProviderError::not_supported(
            "Anthropic does not support embeddings",
        ))
    }
}
