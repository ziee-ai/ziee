//! Gemini provider implementation (custom HTTP implementation for full control)
//!
//! This provider uses direct HTTP calls to the Gemini API instead of gemini-rust library.

use crate::{
    error::ProviderError,
    models::{ChatRequest, EmbeddingsRequest, EmbeddingsResponse, StreamChatChunk, FileUpload, FileUploadResponse},
    traits::AIProvider,
};
use async_stream::stream;
use async_trait::async_trait;
use futures_core::Stream;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

/// Gemini provider (zero-sized, stateless)
pub struct GeminiProvider;

/// Default Gemini API base URL
const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Gemini API request structure
#[derive(Serialize, Debug, Clone)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "systemInstruction")]
    system_instruction: Option<GeminiSystemInstruction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "generationConfig")]
    generation_config: Option<GeminiGenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GeminiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "toolConfig")]
    tool_config: Option<GeminiToolConfig>,
}

/// Gemini content structure
#[derive(Serialize, Deserialize, Debug, Clone)]
struct GeminiContent {
    #[serde(default)]
    role: String,
    #[serde(default)]
    parts: Vec<GeminiPart>,
}

/// Gemini part (text, inline_data, or file_data)
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
enum GeminiPart {
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        thought: Option<bool>,
    },
    InlineData {
        inline_data: GeminiInlineData,
    },
    FileData {
        file_data: GeminiFileData,
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: GeminiFunctionCall,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: GeminiFunctionResponse,
    },
}

/// Gemini inline data (for images)
#[derive(Serialize, Deserialize, Debug, Clone)]
struct GeminiInlineData {
    mime_type: String,
    data: String,
}

/// Gemini file data (for uploaded files via File API)
#[derive(Serialize, Deserialize, Debug, Clone)]
struct GeminiFileData {
    mime_type: String,
    file_uri: String,
}

/// Gemini function call
#[derive(Serialize, Deserialize, Debug, Clone)]
struct GeminiFunctionCall {
    name: String,
    args: serde_json::Value,
}

/// Gemini function response
#[derive(Serialize, Deserialize, Debug, Clone)]
struct GeminiFunctionResponse {
    name: String,
    response: serde_json::Value,
}

/// Gemini system instruction
#[derive(Serialize, Debug, Clone)]
struct GeminiSystemInstruction {
    parts: Vec<GeminiPart>,
}

/// Gemini generation config
#[derive(Serialize, Debug, Clone)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "topP")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "topK")]
    top_k: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "thinkingConfig")]
    thinking_config: Option<GeminiThinkingConfig>,
}

/// Gemini thinking config
#[derive(Serialize, Debug, Clone)]
struct GeminiThinkingConfig {
    #[serde(rename = "thinkingBudget")]
    thinking_budget: i32,
}

/// Gemini tool definition
#[derive(Serialize, Debug, Clone)]
struct GeminiTool {
    #[serde(rename = "functionDeclarations")]
    function_declarations: Vec<GeminiFunctionDeclaration>,
}

/// Gemini function declaration
#[derive(Serialize, Debug, Clone)]
struct GeminiFunctionDeclaration {
    name: String,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<serde_json::Value>,
}

/// Gemini tool config
#[derive(Serialize, Debug, Clone)]
struct GeminiToolConfig {
    #[serde(rename = "functionCallingConfig")]
    function_calling_config: GeminiFunctionCallingConfig,
}

/// Gemini function calling config
#[derive(Serialize, Debug, Clone)]
struct GeminiFunctionCallingConfig {
    mode: String, // "AUTO", "ANY", "NONE"
}

/// Gemini API response
#[derive(Deserialize, Debug)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<GeminiUsageMetadata>,
    #[serde(rename = "promptFeedback", default)]
    prompt_feedback: Option<GeminiPromptFeedback>,
}

/// Gemini candidate
#[derive(Deserialize, Debug)]
struct GeminiCandidate {
    #[serde(default)]
    content: Option<GeminiContent>,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
    #[serde(rename = "safetyRatings", default)]
    safety_ratings: Option<Vec<GeminiSafetyRating>>,
}

/// Gemini usage metadata
#[derive(Deserialize, Debug)]
struct GeminiUsageMetadata {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: Option<i32>,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: Option<i32>,
    #[serde(rename = "totalTokenCount")]
    total_token_count: Option<i32>,
    #[serde(rename = "thoughtsTokenCount")]
    thoughts_token_count: Option<i32>,
}

/// Gemini safety rating
#[derive(Deserialize, Debug, Clone)]
struct GeminiSafetyRating {
    category: String,
    probability: String,
    #[serde(default)]
    blocked: Option<bool>,
}

/// Gemini prompt feedback
#[derive(Deserialize, Debug)]
struct GeminiPromptFeedback {
    #[serde(rename = "blockReason")]
    block_reason: Option<String>,
}

/// Gemini embeddings request
#[derive(Serialize, Debug)]
struct GeminiEmbeddingsRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    content: GeminiEmbeddingContent,
}

/// Gemini batch embeddings request
#[derive(Serialize, Debug)]
struct GeminiBatchEmbeddingsRequest {
    requests: Vec<GeminiEmbeddingsRequest>,
}

/// Gemini embedding content
#[derive(Serialize, Debug)]
struct GeminiEmbeddingContent {
    parts: Vec<GeminiPart>,
}

/// Gemini embeddings response
#[derive(Deserialize, Debug)]
struct GeminiEmbeddingsResponse {
    embedding: GeminiEmbedding,
}

/// Gemini batch embeddings response
#[derive(Deserialize, Debug)]
struct GeminiBatchEmbeddingsResponse {
    embeddings: Vec<GeminiEmbedding>,
}

/// Gemini embedding
#[derive(Deserialize, Debug)]
struct GeminiEmbedding {
    values: Vec<f32>,
}

impl GeminiProvider {
    /// Converts our messages to Gemini Content format
    fn convert_messages(msgs: &[crate::models::ChatMessage]) -> Vec<GeminiContent> {
        use crate::models::{ContentBlock, ImageSource};

        let mut contents = Vec::new();

        for msg in msgs.iter() {
            match msg.role {
                crate::models::Role::System => {
                    // Gemini handles system messages separately - skip here
                }
                crate::models::Role::User | crate::models::Role::Tool => {
                    let mut parts = Vec::new();

                    for block in &msg.content {
                        match block {
                            ContentBlock::Text { text } => {
                                parts.push(GeminiPart::Text {
                                    text: text.clone(),
                                    thought: None,
                                });
                            }
                            ContentBlock::Thinking { thinking } => {
                                // Gemini uses thought flag
                                parts.push(GeminiPart::Text {
                                    text: thinking.clone(),
                                    thought: Some(true),
                                });
                            }
                            ContentBlock::Image { source } => {
                                match source {
                                    ImageSource::Base64 { media_type, data } => {
                                        parts.push(GeminiPart::InlineData {
                                            inline_data: GeminiInlineData {
                                                mime_type: media_type.clone(),
                                                data: data.clone(),
                                            },
                                        });
                                    }
                                    ImageSource::Url { url, .. } => {
                                        // Gemini doesn't support URL images directly
                                        // Would need to fetch and encode - skip for now
                                        eprintln!("Warning: Gemini doesn't support image URLs directly: {}", url);
                                    }
                                    ImageSource::File { file_id } => {
                                        parts.push(GeminiPart::FileData {
                                            file_data: GeminiFileData {
                                                mime_type: "image/jpeg".to_string(), // Default, actual type in metadata
                                                file_uri: file_id.clone(),
                                            },
                                        });
                                    }
                                }
                            }
                            ContentBlock::ToolResult {
                                tool_use_id: _,
                                name,
                                content,
                                is_error,
                            } => {
                                // Convert tool result to functionResponse
                                // Extract the response value from content
                                let response_value = if let Some(ContentBlock::Text { text }) = content.first() {
                                    // Try to parse as JSON, fallback to wrapping in result object
                                    serde_json::from_str(text).unwrap_or_else(|_| {
                                        serde_json::json!({ "result": text })
                                    })
                                } else {
                                    serde_json::json!({ "result": "Empty response" })
                                };

                                // If error, wrap in error structure
                                let final_response = if is_error.unwrap_or(false) {
                                    serde_json::json!({
                                        "error": response_value,
                                        "is_error": true
                                    })
                                } else {
                                    response_value
                                };

                                // Use function name from ToolResult if available, otherwise use placeholder
                                let function_name = name.clone().unwrap_or_else(|| {
                                    tracing::warn!("ToolResult missing function name - using placeholder");
                                    "unknown_function".to_string()
                                });

                                parts.push(GeminiPart::FunctionResponse {
                                    function_response: GeminiFunctionResponse {
                                        name: function_name,
                                        response: final_response,
                                    },
                                });
                            }
                            ContentBlock::ToolUse { .. } => {
                                // Tool use should only appear in assistant messages
                            }
                            ContentBlock::Document { source } => {
                                match source {
                                    crate::models::DocumentSource::Base64 { media_type, data } => {
                                        parts.push(GeminiPart::InlineData {
                                            inline_data: GeminiInlineData {
                                                mime_type: media_type.clone(),
                                                data: data.clone(),
                                            },
                                        });
                                    }
                                    crate::models::DocumentSource::File { file_id } => {
                                        // file_id is the Gemini file URI
                                        parts.push(GeminiPart::FileData {
                                            file_data: GeminiFileData {
                                                mime_type: "application/pdf".to_string(), // Default
                                                file_uri: file_id.clone(),
                                            },
                                        });
                                    }
                                    crate::models::DocumentSource::Url { url } => {
                                        // Gemini doesn't support document URLs directly
                                        eprintln!("Warning: Gemini doesn't support document URLs directly: {}", url);
                                    }
                                }
                            }
                        }
                    }

                    if !parts.is_empty() {
                        contents.push(GeminiContent {
                            role: "user".to_string(),
                            parts,
                        });
                    }
                }
                crate::models::Role::Assistant => {
                    let mut parts = Vec::new();

                    for block in &msg.content {
                        match block {
                            ContentBlock::Text { text } => {
                                parts.push(GeminiPart::Text {
                                    text: text.clone(),
                                    thought: None,
                                });
                            }
                            ContentBlock::Thinking { thinking } => {
                                parts.push(GeminiPart::Text {
                                    text: thinking.clone(),
                                    thought: Some(true),
                                });
                            }
                            ContentBlock::ToolUse { id: _, name, input } => {
                                parts.push(GeminiPart::FunctionCall {
                                    function_call: GeminiFunctionCall {
                                        name: name.clone(),
                                        args: input.clone(),
                                    },
                                });
                            }
                            _ => {} // Skip other types in assistant messages
                        }
                    }

                    if !parts.is_empty() {
                        contents.push(GeminiContent {
                            role: "model".to_string(),
                            parts,
                        });
                    }
                }
            }
        }

        contents
    }

    /// Extracts system instruction from messages
    fn extract_system_instruction(msgs: &[crate::models::ChatMessage]) -> Option<GeminiSystemInstruction> {
        use crate::models::ContentBlock;

        msgs.iter()
            .find(|m| m.role == crate::models::Role::System)
            .and_then(|m| {
                let text = m
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                if text.is_empty() {
                    None
                } else {
                    Some(GeminiSystemInstruction {
                        parts: vec![GeminiPart::Text {
                            text,
                            thought: None,
                        }],
                    })
                }
            })
    }

    /// Converts our tools to Gemini format
    /// Sanitize JSON schema to remove fields unsupported by Gemini API
    /// Gemini doesn't support: exclusiveMinimum, exclusiveMaximum, and other advanced JSON Schema keywords
    fn sanitize_schema_for_gemini(schema: &mut serde_json::Value) {
        if let Some(obj) = schema.as_object_mut() {
            // Remove unsupported keywords
            let had_exclusive_min = obj.remove("exclusiveMinimum").is_some();
            let had_exclusive_max = obj.remove("exclusiveMaximum").is_some();
            let had_title = obj.remove("title").is_some();

            if had_exclusive_min || had_exclusive_max || had_title {
                eprintln!("!!!! SANITIZED: min={}, max={}, title={}", had_exclusive_min, had_exclusive_max, had_title);
            }

            // Recursively sanitize nested objects
            for value in obj.values_mut() {
                Self::sanitize_schema_for_gemini(value);
            }
        } else if let Some(arr) = schema.as_array_mut() {
            for item in arr {
                Self::sanitize_schema_for_gemini(item);
            }
        }
    }

    fn convert_tools(tools: &[crate::models::Tool]) -> Vec<GeminiTool> {
        eprintln!("!!!!! CONVERT_TOOLS CALLED WITH {} TOOLS !!!!!", tools.len());

        if tools.is_empty() {
            return vec![];
        }

        let function_declarations: Vec<GeminiFunctionDeclaration> = tools
            .iter()
            .map(|t| {
                // Clone and sanitize the parameters schema
                let mut parameters = t.function.parameters.clone();

                eprintln!("BEFORE: {}", serde_json::to_string(&parameters).unwrap_or_default());

                Self::sanitize_schema_for_gemini(&mut parameters);

                eprintln!("AFTER: {}", serde_json::to_string(&parameters).unwrap_or_default());

                GeminiFunctionDeclaration {
                    name: t.function.name.clone(),
                    description: t.function.description.clone().unwrap_or_default(),
                    parameters: Some(parameters),
                }
            })
            .collect();

        eprintln!("!!!!! CONVERT_TOOLS RETURNING {} FUNCTION DECLARATIONS !!!!!", function_declarations.len());

        vec![GeminiTool {
            function_declarations,
        }]
    }

    /// Converts our tool choice to Gemini function calling mode
    fn convert_tool_config(choice: &crate::models::ToolChoice) -> GeminiToolConfig {
        let mode = match choice {
            crate::models::ToolChoice::Auto => "AUTO",
            crate::models::ToolChoice::Required => "ANY",
            // Gemini doesn't support forcing a specific tool, so we use ANY
            crate::models::ToolChoice::Specific { .. } => "ANY",
        };

        GeminiToolConfig {
            function_calling_config: GeminiFunctionCallingConfig {
                mode: mode.to_string(),
            },
        }
    }

    /// Converts Gemini stream chunk to our format
    fn convert_stream_chunk(candidate: &GeminiCandidate) -> Option<StreamChatChunk> {
        let mut content_deltas = Vec::new();

        let parts = candidate.content.as_ref().map(|c| c.parts.as_slice()).unwrap_or_default();
        for (idx, part) in parts.iter().enumerate() {
            match part {
                GeminiPart::Text { text, thought } => {
                    // If this is a thought part, use ThinkingDelta; otherwise use TextDelta
                    if let Some(is_thought) = thought {
                        if *is_thought {
                            content_deltas.push(crate::models::ContentBlockDelta::ThinkingDelta {
                                index: idx,
                                delta: text.clone(),
                            });
                        } else {
                            content_deltas.push(crate::models::ContentBlockDelta::TextDelta {
                                index: idx,
                                delta: text.clone(),
                            });
                        }
                    } else {
                        content_deltas.push(crate::models::ContentBlockDelta::TextDelta {
                            index: idx,
                            delta: text.clone(),
                        });
                    }
                }
                GeminiPart::FunctionCall { function_call } => {
                    // Generate a unique ID for Gemini function calls
                    // (Gemini API doesn't provide IDs like Anthropic/OpenAI do)
                    let tool_use_id = format!("gemini_{}", uuid::Uuid::new_v4());

                    content_deltas.push(crate::models::ContentBlockDelta::ToolUseDelta {
                        index: idx,
                        id: Some(tool_use_id),
                        name: Some(function_call.name.clone()),
                        input_delta: Some(function_call.args.to_string()),
                    });
                }
                _ => {}
            }
        }

        if content_deltas.is_empty() && candidate.finish_reason.is_none() {
            return None;
        }

        // Convert safety ratings
        let safety_ratings: Vec<crate::models::SafetyRating> = candidate.safety_ratings.as_ref()
            .map(|ratings| ratings.iter().map(|sr| {
                crate::models::SafetyRating {
                    category: sr.category.clone(),
                    probability: sr.probability.clone(),
                    blocked: sr.blocked.unwrap_or(false),
                }
            }).collect())
            .unwrap_or_default();

        // Check if safety blocked
        let safety_blocked = candidate.finish_reason.as_deref() == Some("SAFETY")
            || safety_ratings.iter().any(|r| r.blocked);

        Some(StreamChatChunk {
            content: content_deltas,
            finish_reason: candidate.finish_reason.clone(),
            usage: None,
            refusal: None,
            safety_ratings,
            safety_blocked,
        })
    }

    /// Normalizes model name to include "models/" prefix
    fn normalize_model(model: &str) -> String {
        if model.starts_with("models/") {
            model.to_string()
        } else {
            format!("models/{}", model)
        }
    }

    /// Gets the effective base URL (use default if empty)
    fn get_base_url(base_url: &str) -> &str {
        if base_url.is_empty() {
            DEFAULT_BASE_URL
        } else {
            base_url
        }
    }
}

#[async_trait]
impl AIProvider for GeminiProvider {
    fn name(&self) -> &str {
        "Gemini"
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
        eprintln!("!!!!! GEMINI STREAM_CHAT CALLED, model={} !!!!!", request.model);
        let client = Client::new();
        let base_url = Self::get_base_url(base_url);
        let model = Self::normalize_model(&request.model);
        eprintln!("!!!!! GEMINI: client created, normalized model={} !!!!!", model);

        // Build request body
        let mut generation_config = GeminiGenerationConfig {
            temperature: request.temperature,
            top_p: request.top_p,
            top_k: Some(40), // Default value
            max_output_tokens: request.max_tokens.map(|t| t as i32),
            thinking_config: None,
        };

        // Add thinking configuration if provided
        if let Some(ref thinking) = request.thinking {
            if thinking.enabled {
                // Determine thinking budget
                let budget = if let Some(ref effort) = thinking.effort {
                    match effort {
                        crate::models::ThinkingEffort::Dynamic => -1,
                        _ => thinking.budget_tokens.unwrap_or(-1),
                    }
                } else {
                    thinking.budget_tokens.unwrap_or(-1)
                };

                generation_config.thinking_config = Some(GeminiThinkingConfig {
                    thinking_budget: budget,
                });
            }
        }

        let gemini_request = GeminiRequest {
            contents: Self::convert_messages(&request.messages),
            system_instruction: Self::extract_system_instruction(&request.messages),
            generation_config: Some(generation_config),
            tools: if request.tools.is_empty() {
                None
            } else {
                Some(Self::convert_tools(&request.tools))
            },
            tool_config: request
                .tool_choice
                .as_ref()
                .map(|tc| Self::convert_tool_config(tc)),
        };

        // Log request details
        if !request.tools.is_empty() {
            tracing::info!("Gemini: Sending {} tools to API", request.tools.len());
            if let Some(ref tc) = request.tool_choice {
                let mode = match tc {
                    crate::models::ToolChoice::Auto => "AUTO",
                    crate::models::ToolChoice::Required => "ANY",
                    crate::models::ToolChoice::Specific { .. } => "ANY (specific)",
                };
                tracing::info!("Gemini: Tool choice mode: {}", mode);
            } else {
                tracing::info!("Gemini: No tool_choice set (defaults to AUTO)");
            }
        }

        // Log the full request body for debugging
        if let Ok(request_json) = serde_json::to_string_pretty(&gemini_request) {
            tracing::info!("Gemini API Request Body: {}", request_json);
        }

        // Make streaming request
        let url = format!(
            "{}/{}:streamGenerateContent?alt=sse&key={}",
            base_url, model, api_key
        );

        eprintln!("!!!!! GEMINI: About to send HTTP POST to: {} !!!!!", url);
        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&gemini_request)
            .send()
            .await?;
        eprintln!("!!!!! GEMINI: HTTP POST completed, status={} !!!!!", response.status());

        // Check status
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ProviderError::from_status_code(
                status.as_u16(),
                error_text,
            ));
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
                        tracing::info!("Gemini: Received chunk ({} bytes)", chunk.len());

                        // Convert bytes to string
                        let chunk_str = match std::str::from_utf8(&chunk) {
                            Ok(s) => s,
                            Err(e) => {
                                yield Err(ProviderError::streaming(format!("Invalid UTF-8: {}", e)));
                                break;
                            }
                        };

                        buffer.push_str(chunk_str);

                        // Process complete SSE events (Gemini uses \r\n\r\n)
                        while let Some(index) = buffer.find("\r\n\r\n").or_else(|| buffer.find("\n\n")) {
                            let event = buffer[..index].to_string();
                            // Check if we found \r\n\r\n (4 chars) or \n\n (2 chars)
                            let drain_len = if buffer[index..].starts_with("\r\n\r\n") { index + 4 } else { index + 2 };
                            buffer.drain(..drain_len);

                            tracing::info!("Gemini: Found complete SSE event ({} bytes)", event.len());

                            // Parse event
                            if event.starts_with("data: ") {
                                let data = &event[6..]; // Skip "data: "
                                tracing::info!("Gemini: Parsing SSE data: {}", if data.len() > 200 { &data[..200] } else { data });

                                // Try to parse as JSON
                                match serde_json::from_str::<GeminiResponse>(data) {
                                    Ok(response) => {
                                    // Log the raw response for debugging
                                    tracing::info!("Gemini API Response: {}", data);

                                    // Check for prompt feedback (prompt blocking)
                                    if let Some(prompt_feedback) = response.prompt_feedback {
                                        if let Some(block_reason) = prompt_feedback.block_reason {
                                            yield Err(ProviderError::provider(format!(
                                                "Prompt blocked: {}",
                                                block_reason
                                            )));
                                            break;
                                        }
                                    }

                                    // Extract usage metadata (typically in final chunk)
                                    if let Some(usage) = response.usage_metadata {
                                        yield Ok(StreamChatChunk {
                                            content: Vec::new(),
                                            finish_reason: None,
                                            usage: Some(crate::models::StreamUsage {
                                                prompt_tokens: usage.prompt_token_count.unwrap_or(0) as u32,
                                                completion_tokens: usage.candidates_token_count.unwrap_or(0) as u32,
                                                total_tokens: usage.total_token_count.unwrap_or(0) as u32,
                                                reasoning_tokens: usage.thoughts_token_count.map(|t| t as u32),
                                            }),
                                            refusal: None,
                                            safety_ratings: Vec::new(),
                                            safety_blocked: false,
                                        });
                                    }

                                    // Extract content from candidate
                                    if let Some(candidate) = response.candidates.first() {
                                        // Log candidate parts for debugging
                                        let parts = candidate.content.as_ref().map(|c| c.parts.as_slice()).unwrap_or_default();
                                        tracing::info!("Gemini candidate has {} parts, finish_reason={:?}", parts.len(), candidate.finish_reason);
                                        for (i, part) in parts.iter().enumerate() {
                                            match part {
                                                GeminiPart::Text { text, .. } => {
                                                    tracing::info!("  Part {}: Text ({}chars)", i, text.len());
                                                }
                                                GeminiPart::FunctionCall { function_call } => {
                                                    tracing::info!("  Part {}: FunctionCall name={}", i, function_call.name);
                                                }
                                                GeminiPart::FunctionResponse { function_response } => {
                                                    tracing::info!("  Part {}: FunctionResponse name={}", i, function_response.name);
                                                }
                                                _ => {
                                                    tracing::info!("  Part {}: Other", i);
                                                }
                                            }
                                        }

                                        if let Some(chunk) = Self::convert_stream_chunk(candidate) {
                                            yield Ok(chunk);
                                        }
                                    }
                                    }
                                    Err(e) => {
                                        yield Err(ProviderError::provider(format!(
                                            "Failed to parse Gemini response: {}",
                                            e
                                        )));
                                        break;
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
        let base_url = Self::get_base_url(base_url);
        let model = Self::normalize_model(&request.model);

        if request.input.len() == 1 {
            // Single embedding
            let gemini_request = GeminiEmbeddingsRequest {
                model: None, // Not needed for single embeddings (in URL)
                content: GeminiEmbeddingContent {
                    parts: vec![GeminiPart::Text {
                        text: request.input[0].clone(),
                        thought: None,
                    }],
                },
            };

            let url = format!("{}/{}:embedContent?key={}", base_url, model, api_key);

            let response = client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&gemini_request)
                .send()
                .await?;

            // Check status
            let status = response.status();
            if !status.is_success() {
                let error_text = response.text().await.unwrap_or_default();
                return Err(ProviderError::from_status_code(
                    status.as_u16(),
                    error_text,
                ));
            }

            let gemini_response: GeminiEmbeddingsResponse = response.json().await?;

            Ok(EmbeddingsResponse {
                embeddings: vec![gemini_response.embedding.values],
                usage: None, // Gemini doesn't provide token usage for embeddings
            })
        } else {
            // Batch embeddings - each request needs the model specified
            let requests: Vec<GeminiEmbeddingsRequest> = request
                .input
                .iter()
                .map(|text| GeminiEmbeddingsRequest {
                    model: Some(model.clone()), // Required for batch embeddings
                    content: GeminiEmbeddingContent {
                        parts: vec![GeminiPart::Text {
                            text: text.clone(),
                            thought: None,
                        }],
                    },
                })
                .collect();

            let gemini_request = GeminiBatchEmbeddingsRequest { requests };

            let url = format!(
                "{}/{}:batchEmbedContents?key={}",
                base_url, model, api_key
            );

            let response = client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&gemini_request)
                .send()
                .await?;

            // Check status
            let status = response.status();
            if !status.is_success() {
                let error_text = response.text().await.unwrap_or_default();
                return Err(ProviderError::from_status_code(
                    status.as_u16(),
                    error_text,
                ));
            }

            let gemini_response: GeminiBatchEmbeddingsResponse = response.json().await?;

            Ok(EmbeddingsResponse {
                embeddings: gemini_response
                    .embeddings
                    .into_iter()
                    .map(|e| e.values)
                    .collect(),
                usage: None,
            })
        }
    }

    async fn upload_file(
        &self,
        api_key: &str,
        base_url: &str,
        upload: FileUpload,
    ) -> Result<Option<FileUploadResponse>, ProviderError> {
        let client = Client::new();

        // Gemini uses resumable upload protocol (2-step process)
        // Step 1: Initiate resumable upload and get upload URL

        // Build base URL (remove /v1beta if present, we'll add /upload/v1beta/files)
        let upload_base = base_url.trim_end_matches("/v1beta").trim_end_matches('/');
        let init_url = format!("{}/upload/v1beta/files", upload_base);

        let file_size = upload.file_data.len();
        let metadata = serde_json::json!({
            "file": {
                "display_name": upload.filename
            }
        });

        // Step 1: Initiate resumable upload
        let init_response = client
            .post(&init_url)
            .header("x-goog-api-key", api_key)
            .header("X-Goog-Upload-Protocol", "resumable")
            .header("X-Goog-Upload-Command", "start")
            .header("X-Goog-Upload-Header-Content-Length", file_size.to_string())
            .header("X-Goog-Upload-Header-Content-Type", &upload.mime_type)
            .header("Content-Type", "application/json")
            .json(&metadata)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e))?;

        // Check status
        let status = init_response.status();
        if !status.is_success() {
            let error_text = init_response.text().await.unwrap_or_default();
            return Err(ProviderError::from_status_code(status.as_u16(), error_text));
        }

        // Get upload URL from response headers
        let upload_url = init_response
            .headers()
            .get("x-goog-upload-url")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ProviderError::file_upload("No upload URL in response"))?
            .to_string();

        // Step 2: Upload file bytes
        let upload_response = client
            .post(&upload_url)
            .header("Content-Length", file_size.to_string())
            .header("X-Goog-Upload-Offset", "0")
            .header("X-Goog-Upload-Command", "upload, finalize")
            .body(upload.file_data.clone())
            .send()
            .await
            .map_err(|e| ProviderError::Network(e))?;

        // Check status
        let status = upload_response.status();
        if !status.is_success() {
            let error_text = upload_response.text().await.unwrap_or_default();
            return Err(ProviderError::from_status_code(status.as_u16(), error_text));
        }

        // Parse response
        #[derive(Deserialize)]
        struct GeminiFileUploadResponse {
            file: GeminiFileMetadata,
        }

        #[derive(Deserialize)]
        struct GeminiFileMetadata {
            name: String,
            uri: String,
            #[serde(rename = "mimeType")]
            mime_type: String,
            #[serde(rename = "sizeBytes")]
            size_bytes: String,
            state: String,
        }

        let upload_response: GeminiFileUploadResponse = upload_response.json().await
            .map_err(|e| ProviderError::file_upload(format!("Failed to parse upload response: {}", e)))?;

        // Gemini files expire after 48 hours
        let expires_at = chrono::Utc::now() + chrono::Duration::hours(48);

        Ok(Some(FileUploadResponse {
            provider_file_id: upload_response.file.uri,
            expires_at: Some(expires_at),
            metadata: Some(serde_json::json!({
                "filename": upload.filename,
                "mime_type": upload_response.file.mime_type,
                "size_bytes": upload_response.file.size_bytes,
                "state": upload_response.file.state,
                "name": upload_response.file.name,
            })),
        }))
    }

    fn supports_file_api(&self) -> bool {
        true
    }

    fn file_expiration(&self) -> Option<chrono::Duration> {
        Some(chrono::Duration::hours(48))  // Gemini files expire after 48 hours
    }

    async fn delete_file(
        &self,
        api_key: &str,
        base_url: &str,
        provider_file_id: &str,
    ) -> Result<(), ProviderError> {
        let client = Client::new();

        // provider_file_id could be:
        // - Full URL: "https://generativelanguage.googleapis.com/v1beta/files/abc123"
        // - Relative path: "files/abc123"
        // Extract the file name part (after "/files/")
        let file_name = if provider_file_id.starts_with("http") {
            // Extract from URL
            provider_file_id
                .split("/files/")
                .nth(1)
                .unwrap_or(provider_file_id)
        } else {
            // Already a path like "files/abc123"
            provider_file_id.strip_prefix("files/").unwrap_or(provider_file_id)
        };

        // Use x-goog-api-key header instead of query parameter
        let delete_url = format!("{}/files/{}", base_url, file_name);

        let response = client
            .delete(&delete_url)
            .header("x-goog-api-key", api_key)
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
    use super::*;

    /// Gemini sometimes returns a candidate with `content: null` — e.g. when the
    /// response is safety-blocked or when a streaming chunk only carries
    /// `finishReason`. With the old `content: GeminiContent` (non-optional) field
    /// these payloads failed serde deserialization and broke the stream.
    #[test]
    fn test_gemini_candidate_deserializes_with_null_content() {
        let json = r#"{
            "content": null,
            "finishReason": "STOP"
        }"#;
        let candidate: GeminiCandidate =
            serde_json::from_str(json).expect("should deserialize with null content");
        assert!(candidate.content.is_none());
        assert_eq!(candidate.finish_reason.as_deref(), Some("STOP"));
    }

    /// Same as above but `content` is missing entirely (older Gemini SDKs).
    #[test]
    fn test_gemini_candidate_deserializes_with_missing_content() {
        let json = r#"{
            "finishReason": "STOP"
        }"#;
        let candidate: GeminiCandidate =
            serde_json::from_str(json).expect("should deserialize with missing content");
        assert!(candidate.content.is_none());
    }

    /// `GeminiContent.role` and `.parts` are `#[serde(default)]` so a partial
    /// content (only one field) deserializes cleanly.
    #[test]
    fn test_gemini_content_deserializes_with_missing_fields() {
        let json_no_role = r#"{ "parts": [] }"#;
        let content: GeminiContent =
            serde_json::from_str(json_no_role).expect("should deserialize without role");
        assert_eq!(content.role, "");

        let json_no_parts = r#"{ "role": "model" }"#;
        let content: GeminiContent =
            serde_json::from_str(json_no_parts).expect("should deserialize without parts");
        assert_eq!(content.role, "model");
        assert!(content.parts.is_empty());
    }

    /// A streaming chunk with only `finishReason` (no parts) must yield a
    /// `StreamChatChunk` so the caller's loop can terminate. Without this the
    /// stream silently truncates and the assistant message never finalizes.
    #[test]
    fn test_convert_stream_chunk_yields_on_finish_reason_only() {
        let candidate = GeminiCandidate {
            content: None,
            finish_reason: Some("STOP".to_string()),
            safety_ratings: None,
        };
        let chunk = GeminiProvider::convert_stream_chunk(&candidate);
        assert!(
            chunk.is_some(),
            "finish_reason-only chunk must produce a StreamChatChunk"
        );
        let chunk = chunk.unwrap();
        assert!(chunk.content.is_empty());
        assert_eq!(chunk.finish_reason.as_deref(), Some("STOP"));
    }

    /// A chunk with neither content nor finish_reason returns `None` (nothing
    /// to do).
    #[test]
    fn test_convert_stream_chunk_returns_none_when_empty() {
        let candidate = GeminiCandidate {
            content: None,
            finish_reason: None,
            safety_ratings: None,
        };
        assert!(GeminiProvider::convert_stream_chunk(&candidate).is_none());
    }
}
