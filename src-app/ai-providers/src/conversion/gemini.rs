//! Conversion functions between our types and Gemini library types

use crate::models::{
    ChatMessage, ChatResponse, Choice, EmbeddingsResponse, FunctionCall, Message, Role,
    StreamChatChunk, Tool, ToolCall, ToolChoice, Usage,
};
use gemini_rust::{
    Blob, Content, FunctionCallingMode, FunctionDeclaration, GenerationResponse, Part,
    Role as GeminiRole, Tool as GeminiTool,
};

/// Converts our messages to Gemini Content format
pub fn convert_messages_to_contents(
    msgs: &[ChatMessage],
    attachments: &[crate::models::FileAttachment],
) -> Vec<Content> {
    use base64::Engine;
    let base64_engine = base64::engine::general_purpose::STANDARD;

    let mut contents = Vec::new();

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
                // Gemini handles system messages separately - skip here
            }
            Role::User => {
                // Build parts (with attachments if this is the last user message)
                let mut parts = Vec::new();

                // Add text part if there is content
                if let Some(ref text) = msg.content {
                    if !text.is_empty() {
                        parts.push(Part::Text {
                            text: text.clone(),
                            thought: None,
                            thought_signature: None,
                        });
                    }
                }

                // Add image parts if this is the last user message
                if Some(idx) == last_user_idx {
                    for attachment in attachments {
                        // Check if it's an image
                        if attachment.mime_type.starts_with("image/") {
                            let base64_data = base64_engine.encode(&attachment.content);
                            parts.push(Part::InlineData {
                                inline_data: Blob::new(
                                    attachment.mime_type.clone(),
                                    base64_data,
                                ),
                            });
                        }
                    }
                }

                contents.push(Content {
                    parts: Some(parts),
                    role: Some(GeminiRole::User),
                });
            }
            Role::Assistant => {
                contents.push(Content {
                    parts: Some(vec![Part::Text {
                        text: msg.content.clone().unwrap_or_default(),
                        thought: None,
                        thought_signature: None,
                    }]),
                    role: Some(GeminiRole::Model),
                });
            }
            Role::Tool => {
                // Skip tool messages for now - Gemini handles them differently
            }
        }
    }

    contents
}

/// Extracts system instruction from messages
pub fn extract_system_instruction(msgs: &[ChatMessage]) -> Option<String> {
    msgs.iter()
        .find(|m| m.role == Role::System)
        .and_then(|m| m.content.clone())
}

/// Converts Gemini generation response to our format
pub fn convert_generation_response(resp: GenerationResponse, model: String) -> ChatResponse {
    let id = format!("gemini-{}", uuid::Uuid::new_v4()); // Generate ID since Gemini doesn't provide one

    let choices = resp
        .candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let mut content = String::new();
            let mut thinking = String::new();
            let mut tool_calls = Vec::new();

            if let Some(parts) = &candidate.content.parts {
                for part in parts {
                    match part {
                        Part::Text { text, thought, .. } => {
                            // If this is a thought part, add to thinking; otherwise add to content
                            if let Some(is_thought) = thought {
                                if *is_thought {
                                    if !thinking.is_empty() {
                                        thinking.push_str("\n");
                                    }
                                    thinking.push_str(text);
                                } else {
                                    content.push_str(text);
                                }
                            } else {
                                // No thought marker, treat as regular content
                                content.push_str(text);
                            }
                        }
                        Part::FunctionCall { function_call, .. } => {
                            tool_calls.push(ToolCall {
                                id: uuid::Uuid::new_v4(),
                                tool_type: "function".to_string(),
                                function: FunctionCall {
                                    name: function_call.name.clone(),
                                    arguments: serde_json::to_string(&function_call.args)
                                        .unwrap_or_default(),
                                },
                            });
                        }
                        _ => {}
                    }
                }
            }

            Choice {
                message: Message {
                    role: Role::Assistant,
                    content,
                    tool_calls,
                    thinking: if thinking.is_empty() {
                        None
                    } else {
                        Some(thinking)
                    },
                },
                finish_reason: format!("{:?}", candidate.finish_reason),
                index: index as u32,
            }
        })
        .collect();

    let usage = resp.usage_metadata.map(|u| Usage {
        prompt_tokens: u.prompt_token_count.unwrap_or(0) as u32,
        completion_tokens: u.candidates_token_count.unwrap_or(0) as u32,
        total_tokens: u.total_token_count.unwrap_or(0) as u32,
        reasoning_tokens: None, // TODO: Extract thoughts_token_count from metadata
    });

    ChatResponse {
        id,
        model,
        choices,
        usage,
    }
}

/// Converts Gemini stream chunk to our format
pub fn convert_stream_chunk_from_response(resp: &GenerationResponse) -> Option<StreamChatChunk> {
    let candidate = resp.candidates.first()?;
    let parts = candidate.content.parts.as_ref()?;

    let mut content = String::new();
    let mut thinking = String::new();

    for part in parts {
        match part {
            Part::Text { text, thought, .. } => {
                // If this is a thought part, add to thinking; otherwise add to content
                if let Some(is_thought) = thought {
                    if *is_thought {
                        if !thinking.is_empty() {
                            thinking.push_str("\n");
                        }
                        thinking.push_str(text);
                    } else {
                        content.push_str(text);
                    }
                } else {
                    content.push_str(text);
                }
            }
            _ => {}
        }
    }

    if content.is_empty() && thinking.is_empty() {
        return None;
    }

    Some(StreamChatChunk {
        content,
        finish_reason: Some(format!("{:?}", candidate.finish_reason)),
        thinking: if thinking.is_empty() {
            None
        } else {
            Some(thinking)
        },
    })
}

/// Converts Gemini embeddings response to our format
pub fn convert_embeddings_response(
    resp: gemini_rust::ContentEmbeddingResponse,
) -> EmbeddingsResponse {
    EmbeddingsResponse {
        embeddings: vec![resp.embedding.values],
        usage: None, // Gemini doesn't provide token usage for embeddings
    }
}

/// Converts batch embeddings response to our format
pub fn convert_batch_embeddings_response(
    resp: gemini_rust::BatchContentEmbeddingResponse,
) -> EmbeddingsResponse {
    EmbeddingsResponse {
        embeddings: resp
            .embeddings
            .into_iter()
            .map(|e| e.values)
            .collect(),
        usage: None,
    }
}

/// Converts our tools to Gemini tool format
pub fn convert_tools(tools: &[Tool]) -> Vec<GeminiTool> {
    // Gemini expects a single Tool::Function with multiple FunctionDeclarations
    if tools.is_empty() {
        return vec![];
    }

    let function_declarations: Vec<FunctionDeclaration> = tools
        .iter()
        .map(|t| {
            // Create a FunctionDeclaration using the builder API
            // Note: Gemini uses a special schema generator, so we pass the parameters as raw JSON
            let decl = FunctionDeclaration::new(
                t.function.name.clone(),
                t.function.description.clone().unwrap_or_default(),
                None, // behavior
            );
            // Store the parameters as a JSON value by reconstructing with serde
            // This is a workaround since parameters is private
            let json = serde_json::json!({
                "name": t.function.name,
                "description": t.function.description,
                "parameters": t.function.parameters,
            });
            // Use deserialization to set the private fields
            serde_json::from_value(json).unwrap_or(decl)
        })
        .collect();

    vec![GeminiTool::Function {
        function_declarations,
    }]
}

/// Converts our tool choice to Gemini function calling mode
pub fn convert_tool_choice(choice: &ToolChoice) -> FunctionCallingMode {
    match choice {
        ToolChoice::Auto => FunctionCallingMode::Auto,
        ToolChoice::Required => FunctionCallingMode::Any,
        // Gemini doesn't support forcing a specific tool, so we use Any
        ToolChoice::Specific { .. } => FunctionCallingMode::Any,
    }
}

/// Extracts tool calls from Gemini response
pub fn extract_tool_calls(response: &GenerationResponse) -> Vec<ToolCall> {
    let mut tool_calls = Vec::new();

    for candidate in &response.candidates {
        if let Some(parts) = &candidate.content.parts {
            for part in parts {
                if let Part::FunctionCall { function_call, .. } = part {
                    tool_calls.push(ToolCall {
                        id: uuid::Uuid::new_v4(),
                        tool_type: "function".to_string(),
                        function: FunctionCall {
                            name: function_call.name.clone(),
                            arguments: serde_json::to_string(&function_call.args)
                                .unwrap_or_default(),
                        },
                    });
                }
            }
        }
    }

    tool_calls
}

// Simple UUID generation using current time
mod uuid {
    pub struct Uuid;
    impl Uuid {
        pub fn new_v4() -> String {
            use std::time::{SystemTime, UNIX_EPOCH};
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            format!("{:x}", now)
        }
    }
}
