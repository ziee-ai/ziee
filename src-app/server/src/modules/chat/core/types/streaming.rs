// Chat streaming types
#![allow(dead_code)]

// Streaming API types

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A chunk of streamed chat content (core streaming response from LLM)
///
/// IMPORTANT: Extensions should NOT add fields to this struct.
/// Instead, extensions should:
/// - Send their own SSE events via SSEChatStreamEvent variants
/// - Add new ContentBlockDelta variants if needed
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
pub struct ChatStreamChunk {
    /// Content block deltas
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<ContentBlockDelta>,

    /// Message ID (sent in first chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<Uuid>,

    /// Conversation ID (sent in first chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<Uuid>,

    /// Branch ID (sent in first chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_id: Option<Uuid>,

    /// Finish reason (when stream completes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,

    /// Usage metadata (when stream completes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,

    /// Error information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<StreamError>,
}

/// Content block delta - Base types (extensions CAN add more variants)
///
/// EXTENSIONS MAY extend this enum with new variants using the
/// compose_content_block_delta_variants macro. For example, the MCP extension
/// adds ToolUseDelta and ToolResultDelta variants.
///
/// Extension variants are automatically added by the compose_content_block_delta_variants macro.
#[macros::compose_content_block_delta_variants]
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockDelta {
    /// Text content delta
    TextDelta {
        index: usize,
        delta: String,
    },

    /// Thinking content delta
    ThinkingDelta {
        index: usize,
        delta: String,
    },
}

impl ContentBlockDelta {
    /// Get the index of this content block
    pub fn index(&self) -> usize {
        match self {
            Self::TextDelta { index, .. } => *index,
            Self::ThinkingDelta { index, .. } => *index,
            Self::ToolUseDelta { index, .. } => *index,
        }
    }

    /// Convert from ai-providers ContentBlockDelta
    pub fn from_ai_providers_delta(delta: &ai_providers::ContentBlockDelta) -> Option<Self> {
        match delta {
            ai_providers::ContentBlockDelta::TextDelta { index, delta } => Some(Self::TextDelta {
                index: *index,
                delta: delta.clone(),
            }),
            ai_providers::ContentBlockDelta::ThinkingDelta { index, delta } => {
                Some(Self::ThinkingDelta {
                    index: *index,
                    delta: delta.clone(),
                })
            }
            // Tool-related deltas handled by MCP extension
            _ => None,
        }
    }
}

/// Usage metadata from AI provider
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Usage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u32>,
}

impl From<ai_providers::StreamUsage> for Usage {
    fn from(usage: ai_providers::StreamUsage) -> Self {
        Self {
            input_tokens: Some(usage.prompt_tokens),
            output_tokens: Some(usage.completion_tokens),
        }
    }
}

/// Stream error information
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StreamError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

// ===================================================================
// Server-Sent Event Types
// ===================================================================

/// Data for the Started SSE event
/// Sent before content streaming begins to communicate conversation context
/// Client learns assistant_message_id from content chunks (message_id field)
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct SSEChatStreamStartedData {
    /// User message ID (None if resuming with tool approvals or regenerating)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_message_id: Option<Uuid>,

    /// Conversation ID
    pub conversation_id: Uuid,

    /// Branch ID
    pub branch_id: Uuid,
}

/// Data for the Complete SSE event
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct SSEChatStreamCompleteData {
    /// Finish reason
    pub finish_reason: String,

    /// Usage metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

/// Data for the Error SSE event
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct SSEChatStreamErrorData {
    /// Error message
    pub message: String,

    /// Error code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// SSE event enum for chat streaming
///
/// This enum represents all possible Server-Sent Events that can be streamed
/// during a chat message request.
///
/// # Extension Architecture
///
/// **EXTENSIONS SHOULD send their own SSE events** instead of adding fields to ChatStreamChunk.
/// Extensions add new event variants through the SSEChatStreamEventVariants enum using
/// the compose_chat_stream_events macro.
///
/// Example: The title extension sends a separate `TitleUpdated` event instead of
/// adding a title field to ChatStreamChunk.
///
/// Events are sent with proper `event:` names (e.g., "started", "content", "complete", "error", "titleUpdated")
/// for type-safe client-side handling.
#[macros::compose_chat_stream_events]
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum SSEChatStreamEvent {
    /// Streaming started event (sent before content with message IDs)
    Started(SSEChatStreamStartedData),

    /// Content chunk event (streamed content deltas)
    Content(ChatStreamChunk),

    /// Stream completion event
    Complete(SSEChatStreamCompleteData),

    /// Error event
    Error(SSEChatStreamErrorData),
}

// Generic implementation that works for all variants (including extension-added ones)
impl SSEChatStreamEvent {
    /// Get the event name for this SSE event
    /// Uses serde's tag to extract the variant name in camelCase format
    pub fn event_name(&self) -> &'static str {
        // For core variants, return static strings (avoids allocation/conversion overhead)
        // Extension variants are handled dynamically
        // Extract variant name from Debug representation
        let debug_str = format!("{:?}", self);
        let variant_name = debug_str.split('(').next().unwrap_or("unknown");

        // Return static strings for known core variants only
        match variant_name {
            "Started" => "started",
            "Content" => "content",
            "Complete" => "complete",
            "Error" => "error",
            // Extension variants: convert PascalCase to camelCase dynamically
            _ => {
                // Convert first character to lowercase for camelCase
                // Note: This leaks a small amount of memory for each unique extension variant
                // but is only called once per variant type
                Box::leak(
                    variant_name
                        .chars()
                        .enumerate()
                        .map(|(i, c)| if i == 0 { c.to_lowercase().to_string() } else { c.to_string() })
                        .collect::<String>()
                        .into_boxed_str()
                )
            }
        }
    }

    /// Serialize the inner event data to JSON
    pub fn data(&self) -> Result<String, serde_json::Error> {
        // Serialize the entire variant - serde will handle it correctly with the tag
        serde_json::to_string(self)
    }
}

impl Into<axum::response::sse::Event> for SSEChatStreamEvent {
    fn into(self) -> axum::response::sse::Event {
        axum::response::sse::Event::default()
            .event(self.event_name())
            .data(self.data().unwrap_or_default())
    }
}
