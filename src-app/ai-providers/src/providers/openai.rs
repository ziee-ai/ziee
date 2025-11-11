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
    models::{ChatRequest, ChatResponse, EmbeddingsRequest, EmbeddingsResponse, StreamChatChunk},
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

/// OpenAI API response
#[derive(Deserialize, Debug)]
struct OpenAIResponse {
    id: String,
    model: String,
    choices: Vec<OpenAIChoice>,
    usage: Option<OpenAIUsage>,
}

#[derive(Deserialize, Debug)]
struct OpenAIChoice {
    index: u32,
    message: OpenAIResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
struct OpenAIResponseMessage {
    #[allow(dead_code)]
    role: String,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAIToolCall>>,
}

#[derive(Deserialize, Debug)]
struct OpenAIUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
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
    /// Converts our messages to OpenAI format
    fn convert_messages(
        msgs: &[crate::models::ChatMessage],
        attachments: &[crate::models::FileAttachment],
    ) -> Vec<OpenAIMessage> {
        use base64::Engine;
        let base64_engine = base64::engine::general_purpose::STANDARD;

        // Find the index of the last user message to attach files to
        let last_user_idx = msgs
            .iter()
            .enumerate()
            .rev()
            .find(|(_, m)| m.role == crate::models::Role::User)
            .map(|(i, _)| i);

        msgs.iter()
            .enumerate()
            .map(|(idx, m)| {
                let role = match m.role {
                    crate::models::Role::System => "system",
                    crate::models::Role::User => "user",
                    crate::models::Role::Assistant => "assistant",
                    crate::models::Role::Tool => "tool",
                }
                .to_string();

                let tool_calls = if !m.tool_calls.is_empty() {
                    Some(
                        m.tool_calls
                            .iter()
                            .map(|tc| OpenAIToolCall {
                                id: tc.id.clone(),
                                tool_type: tc.tool_type.clone(),
                                function: OpenAIFunctionCall {
                                    name: tc.function.name.clone(),
                                    arguments: tc.function.arguments.clone(),
                                },
                            })
                            .collect(),
                    )
                } else {
                    None
                };

                // Build content (with attachments if this is the last user message)
                let content = if Some(idx) == last_user_idx && !attachments.is_empty() {
                    // Multimodal content with text and images
                    let mut parts = Vec::new();

                    // Add text part if there is content
                    if let Some(ref text) = m.content {
                        if !text.is_empty() {
                            parts.push(OpenAIContentPart::Text { text: text.clone() });
                        }
                    }

                    // Add image parts
                    for attachment in attachments {
                        // Check if it's an image
                        if attachment.mime_type.starts_with("image/") {
                            let base64_data = base64_engine.encode(&attachment.content);
                            let data_url =
                                format!("data:{};base64,{}", attachment.mime_type, base64_data);
                            parts.push(OpenAIContentPart::ImageUrl {
                                image_url: OpenAIImageUrl {
                                    url: data_url,
                                    detail: None, // Let the API decide
                                },
                            });
                        }
                    }

                    if parts.len() > 1 {
                        Some(OpenAIContent::Multimodal(parts))
                    } else if parts.len() == 1 {
                        // Single text part, use string format
                        if let Some(OpenAIContentPart::Text { text }) = parts.into_iter().next() {
                            Some(OpenAIContent::Text(text))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    // Regular text content
                    m.content.clone().map(OpenAIContent::Text)
                };

                OpenAIMessage {
                    role,
                    content,
                    name: None,
                    tool_call_id: m.tool_call_id.clone(),
                    tool_calls,
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

    /// Converts OpenAI response to our format
    fn convert_response(resp: OpenAIResponse) -> ChatResponse {
        use crate::models::{Choice, Message, Role, Usage};

        ChatResponse {
            id: resp.id,
            model: resp.model,
            choices: resp
                .choices
                .into_iter()
                .map(|c| {
                    let tool_calls = c
                        .message
                        .tool_calls
                        .unwrap_or_default()
                        .into_iter()
                        .map(|tc| crate::models::ToolCall {
                            id: tc.id,
                            tool_type: tc.tool_type,
                            function: crate::models::FunctionCall {
                                name: tc.function.name,
                                arguments: tc.function.arguments,
                            },
                        })
                        .collect();

                    Choice {
                        message: Message {
                            role: Role::Assistant,
                            content: c.message.content.unwrap_or_default(),
                            tool_calls,
                            thinking: None, // TODO: Extract thinking for reasoning models
                        },
                        finish_reason: c.finish_reason.unwrap_or_else(|| "stop".to_string()),
                        index: c.index,
                    }
                })
                .collect(),
            usage: resp.usage.map(|u| Usage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
                reasoning_tokens: u
                    .completion_tokens_details
                    .and_then(|d| d.reasoning_tokens),
            }),
        }
    }
}

#[async_trait]
impl AIProvider for OpenAIProvider {
    fn name(&self) -> &str {
        "OpenAI"
    }

    fn provider_type(&self) -> &str {
        "openai"
    }

    async fn chat(
        &self,
        api_key: &str,
        base_url: &str,
        request: ChatRequest,
    ) -> Result<ChatResponse, ProviderError> {
        let client = Client::new();

        // Convert messages
        let messages = Self::convert_messages(&request.messages, &request.attachments);

        // Build request body
        let mut body = json!({
            "model": request.model,
            "messages": messages,
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
                        crate::models::ThinkingEffort::Dynamic => "medium", // Default for OpenAI
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

        // Make request
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
        let openai_resp: OpenAIResponse = response.json().await?;

        // Convert to our format
        Ok(Self::convert_response(openai_resp))
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
        let messages = Self::convert_messages(&request.messages, &request.attachments);

        // Build request body with stream: true
        let mut body = json!({
            "model": request.model,
            "messages": messages,
            "stream": true,
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
                                    if let Some(choice) = chunk_data.choices.first() {
                                        if let Some(content) = &choice.delta.content {
                                            yield Ok(StreamChatChunk {
                                                content: content.clone(),
                                                finish_reason: choice.finish_reason.clone(),
                                                thinking: None, // TODO: Extract thinking for reasoning models
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
