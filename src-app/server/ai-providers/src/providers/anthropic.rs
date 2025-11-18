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
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
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
}

impl AnthropicProvider {
    /// Converts our messages to Anthropic format
    fn convert_messages(msgs: &[ChatMessage]) -> (Option<String>, Vec<AnthropicMessage>) {
        use crate::models::{ContentBlock, ImageSource};

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
                        system_message = Some(text);
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
                                let anthropic_source = match source {
                                    ImageSource::Base64 { media_type, data } => {
                                        AnthropicImageSource {
                                            source_type: "base64".to_string(),
                                            media_type: Some(media_type.clone()),
                                            data: Some(data.clone()),
                                            file_id: None,
                                        }
                                    }
                                    ImageSource::Url { url, .. } => AnthropicImageSource {
                                        source_type: "url".to_string(),
                                        media_type: None,
                                        data: Some(url.clone()),
                                        file_id: None,
                                    },
                                    ImageSource::File { file_id } => AnthropicImageSource {
                                        source_type: "file".to_string(),
                                        media_type: None,
                                        data: None,
                                        file_id: Some(file_id.clone()),
                                    },
                                };
                                anthropic_blocks.push(AnthropicContentBlock::Image {
                                    source: anthropic_source,
                                });
                            }
                            ContentBlock::Thinking { thinking } => {
                                // Anthropic supports thinking blocks natively
                                anthropic_blocks.push(AnthropicContentBlock::Text {
                                    text: thinking.clone(),
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
                                content,
                                is_error,
                            } => {
                                // Convert nested content blocks
                                let result_content = content
                                    .iter()
                                    .filter_map(|block| match block {
                                        ContentBlock::Text { text } => Some(text.clone()),
                                        _ => None,
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n");

                                anthropic_blocks.push(AnthropicContentBlock::ToolResult {
                                    tool_use_id: tool_use_id.clone(),
                                    content: result_content,
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
                                    DocumentSource::File { file_id } => {
                                        AnthropicDocumentSource {
                                            source_type: "file".to_string(),
                                            media_type: None,
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

                    // Build message content
                    let content = if anthropic_blocks.is_empty() {
                        AnthropicMessageContent::Text(String::new())
                    } else if anthropic_blocks.len() == 1
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
                            ContentBlock::Thinking { thinking } => {
                                anthropic_blocks.push(AnthropicContentBlock::Text {
                                    text: thinking.clone(),
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

                    let content = if anthropic_blocks.is_empty() {
                        AnthropicMessageContent::Text(String::new())
                    } else if anthropic_blocks.len() == 1
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
        let client = Client::new();

        // Convert messages
        let (system, messages) = Self::convert_messages(&request.messages);

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

                                    // Try to parse as JSON
                                    if let Ok(chunk_data) = serde_json::from_str::<AnthropicStreamChunk>(data) {
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

                                        // Handle message_delta for usage and finish reason
                                        if chunk_data.event_type == "message_delta" {
                                            let mut finish_reason = None;
                                            let mut usage = None;

                                            if let Some(message_delta) = chunk_data.message {
                                                finish_reason = message_delta.stop_reason;
                                            }

                                            if let Some(stream_usage) = chunk_data.usage {
                                                let input = stream_usage.input_tokens.unwrap_or(0);
                                                let output = stream_usage.output_tokens.unwrap_or(0);
                                                usage = Some(crate::models::StreamUsage {
                                                    prompt_tokens: input,
                                                    completion_tokens: output,
                                                    total_tokens: input + output,
                                                    reasoning_tokens: None,
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

                                        // Handle content_block_delta
                                        if chunk_data.event_type == "content_block_delta" {
                                            if let Some(delta) = chunk_data.delta {
                                                let content_delta = match delta.delta_type.as_str() {
                                                    "text_delta" => {
                                                        delta.text.map(|text| {
                                                            crate::models::ContentBlockDelta::TextDelta {
                                                                index: 0, // TODO: track block index
                                                                delta: text,
                                                            }
                                                        })
                                                    }
                                                    "thinking_delta" => {
                                                        delta.thinking.map(|thinking| {
                                                            crate::models::ContentBlockDelta::ThinkingDelta {
                                                                index: 0, // TODO: track block index
                                                                delta: thinking,
                                                            }
                                                        })
                                                    }
                                                    "input_json_delta" => {
                                                        delta.partial_json.map(|partial_json| {
                                                            crate::models::ContentBlockDelta::ToolUseDelta {
                                                                index: 0, // TODO: track block index
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
        let client = Client::new();

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
        let client = Client::new();

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
