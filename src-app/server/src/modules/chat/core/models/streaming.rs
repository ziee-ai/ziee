// Streaming models

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

/// A chunk of streamed chat content (extends ai-providers StreamChatChunk)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

    /// Auto-generated title (when available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Error information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<StreamError>,
}

/// Content block delta - Base types (extensions can add more variants)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockDelta {
    /// Text content delta
    TextDelta {
        index: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        content_id: Option<Uuid>,
        delta: String,
    },

    /// Thinking content delta
    ThinkingDelta {
        index: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        content_id: Option<Uuid>,
        delta: String,
    },

    // Tool-related deltas (ToolUseDelta, etc.) will be added by MCP extension
}

impl ContentBlockDelta {
    /// Get the index of this content block
    pub fn index(&self) -> usize {
        match self {
            Self::TextDelta { index, .. } => *index,
            Self::ThinkingDelta { index, .. } => *index,
        }
    }

    /// Get the content_id if present
    pub fn content_id(&self) -> Option<Uuid> {
        match self {
            Self::TextDelta { content_id, .. } => *content_id,
            Self::ThinkingDelta { content_id, .. } => *content_id,
        }
    }

    /// Set the content_id
    pub fn set_content_id(&mut self, id: Uuid) {
        match self {
            Self::TextDelta { content_id, .. } => *content_id = Some(id),
            Self::ThinkingDelta { content_id, .. } => *content_id = Some(id),
        }
    }

    /// Convert from ai-providers ContentBlockDelta
    pub fn from_ai_providers_delta(delta: &ai_providers::ContentBlockDelta) -> Option<Self> {
        match delta {
            ai_providers::ContentBlockDelta::TextDelta { index, delta } => {
                Some(Self::TextDelta {
                    index: *index,
                    content_id: None,
                    delta: delta.clone(),
                })
            }
            ai_providers::ContentBlockDelta::ThinkingDelta { index, delta } => {
                Some(Self::ThinkingDelta {
                    index: *index,
                    content_id: None,
                    delta: delta.clone(),
                })
            }
            // Tool-related deltas handled by MCP extension
            _ => None,
        }
    }
}

/// Stream context - Carries metadata through the streaming pipeline
#[derive(Clone)]
pub struct StreamContext {
    pub conversation_id: Uuid,
    pub branch_id: Uuid,
    pub message_id: Uuid,
    pub user_id: Uuid,
    pub model_id: Uuid,
    pub pool: PgPool,
    pub metadata: HashMap<String, Value>,
}

/// Usage metadata from AI provider
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}
